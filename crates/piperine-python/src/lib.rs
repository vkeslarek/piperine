use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use numpy::PyArray1;

use piperine_circuit::{HardwareRegistry, elaborate_circuit, Circuit as RustCircuit, SoaOp};
use piperine_interpreter::{SimulatorBackend, Plugin, AnalysisEvent, value::{AnalysisResult, VectorData}};
use piperine_ngspice::{NgspiceBackend, NgspicePlugin};
use piperine_coordinator::pool::{ProcessPool, PoolConfig};
use piperine_common::EventAction;

use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;

// ── NgspiceSession ────────────────────────────────────────────────────────────

/// A live ngspice simulation session.
///
/// Wrap a structural `.ppr` hardware module and an ngspice worker process.
/// The backend is held in an `Arc<Mutex<Option<...>>>` so async futures can
/// borrow it across threads and return it on completion.
#[pyclass]
pub struct NgspiceSession {
    backend: Arc<Mutex<Option<NgspiceBackend>>>,
    circuit: Arc<RustCircuit>,
}

impl NgspiceSession {
    fn with_backend<F, T>(&self, py: Python<'_>, f: F) -> PyResult<T>
    where
        F: FnOnce(&mut NgspiceBackend) -> Result<T, piperine_interpreter::InterpreterError> + Send,
        T: Send,
    {
        let arc = Arc::clone(&self.backend);
        py.allow_threads(move || {
            let mut guard = arc.lock().unwrap();
            let b = guard.as_mut().ok_or_else(|| {
                piperine_interpreter::InterpreterError::Other(
                    "session backend in use by a SimFuture — call .join() first".into()
                )
            })?;
            f(b)
        }).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }
}

#[pymethods]
impl NgspiceSession {
    /// Load a `.ppr` hardware file and instantiate the named module.
    ///
    /// ```python
    /// sess = NgspiceSession.from_file("hardware/lpf.ppr", module="lpf")
    /// ```
    #[staticmethod]
    #[pyo3(signature = (path, module=None))]
    pub fn from_file(py: Python<'_>, path: &str, module: Option<&str>) -> PyResult<NgspiceSession> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| PyRuntimeError::new_err(format!("cannot read {path}: {e}")))?;

        let include_dirs = vec![
            piperine_ngspice::ppr_dir(),
            piperine_parser::bundled_header_dir(),
        ];
        let doc = piperine_parser::parse_with_includes(&src, &include_dirs)
            .map_err(|e| PyRuntimeError::new_err(format!("parse error in {path}: {e}")))?;

        let mut registry = HardwareRegistry::new();
        NgspicePlugin::default().register_hardware(&mut registry);

        let circuit = Arc::new(elaborate_circuit(&doc, &registry, module)
            .map_err(|e| PyRuntimeError::new_err(format!("elaboration error: {e}")))?);

        let worker_handle = py.allow_threads(|| {
            ProcessPool::spawn(PoolConfig { size: 1, worker_binary: None })
                .map(|mut p| p.take_first())
        }).map_err(|e| PyRuntimeError::new_err(format!("failed to start worker: {e}")))?;

        let mut backend = NgspiceBackend::new(worker_handle.cmd, worker_handle.resp);
        let mut netlist = circuit.spice_lines.clone();
        netlist.push(".end".into());
        py.allow_threads(|| backend.load_circuit(&netlist))
            .map_err(|e| PyRuntimeError::new_err(format!("circuit load error: {e}")))?;

        Ok(NgspiceSession {
            backend: Arc::new(Mutex::new(Some(backend))),
            circuit,
        })
    }

    /// Run an operating-point analysis. Returns `{signal: float}`.
    pub fn op(&self, py: Python<'_>) -> PyResult<HashMap<String, f64>> {
        let result = self.with_backend(py, |b| b.run_analysis_simple("op"))?;
        Ok(result.vectors.into_iter()
            .filter_map(|(k, v)| {
                if let VectorData::Real(data) = v { data.into_iter().next().map(|x| (k, x)) }
                else { None }
            })
            .collect())
    }

    /// Run a transient analysis. Returns `{signal: np.ndarray}`.
    ///
    /// ```python
    /// r = sess.tran("1n", "1u")
    /// plt.plot(r["time"], r["v(out)"])
    /// ```
    pub fn tran<'py>(&self, py: Python<'py>, step: &str, stop: &str)
        -> PyResult<HashMap<String, Bound<'py, PyArray1<f64>>>>
    {
        let cmd = format!("tran {step} {stop}");
        let result = self.with_backend(py, move |b| b.run_analysis_simple(&cmd))?;
        vectors_to_py(py, result)
    }

    /// Run an AC sweep. Complex signals split into `.re` / `.im` arrays.
    /// `frequency` vector is real; all other vectors are complex.
    pub fn ac<'py>(&self, py: Python<'py>, scale: &str, points: u32, fstart: f64, fstop: f64)
        -> PyResult<HashMap<String, Bound<'py, PyArray1<f64>>>>
    {
        let cmd = format!("ac {scale} {points} {fstart} {fstop}");
        let arc = Arc::clone(&self.backend);
        let result: HashMap<String, VectorData> = py.allow_threads(move || {
            let mut guard = arc.lock().unwrap();
            let b = guard.as_mut().ok_or_else(|| {
                piperine_interpreter::InterpreterError::Other(
                    "session backend in use by a SimFuture".into()
                )
            })?;
            run_ac_analysis(b, &cmd)
        }).map_err(|e: piperine_interpreter::InterpreterError| PyRuntimeError::new_err(e.to_string()))?;

        let mut out = HashMap::new();
        for (name, data) in result {
            match data {
                VectorData::Real(v) => { out.insert(name, PyArray1::from_vec_bound(py, v)); }
                VectorData::Complex(pairs) => {
                    let re: Vec<f64> = pairs.iter().map(|(r, _)| *r).collect();
                    let im: Vec<f64> = pairs.iter().map(|(_, i)| *i).collect();
                    out.insert(format!("{name}.re"), PyArray1::from_vec_bound(py, re));
                    out.insert(format!("{name}.im"), PyArray1::from_vec_bound(py, im));
                    // Also expose magnitude and phase for convenience.
                    let mag: Vec<f64> = pairs.iter().map(|(r, i)| (r*r + i*i).sqrt()).collect();
                    let pha: Vec<f64> = pairs.iter().map(|(r, i)| i.atan2(*r)).collect();
                    out.insert(format!("{name}"), PyArray1::from_vec_bound(py, mag.clone()));
                    out.insert(format!("{name}.mag"), PyArray1::from_vec_bound(py, mag));
                    out.insert(format!("{name}.phase"), PyArray1::from_vec_bound(py, pha));
                }
            }
        }
        Ok(out)
    }

    /// Run a DC sweep.
    pub fn dc<'py>(&self, py: Python<'py>, source: &str, start: f64, stop: f64, step: f64)
        -> PyResult<HashMap<String, Bound<'py, PyArray1<f64>>>>
    {
        let cmd = format!("dc {source} {start} {stop} {step}");
        let result = self.with_backend(py, move |b| b.run_analysis_simple(&cmd))?;
        vectors_to_py(py, result)
    }

    /// Alter a device parameter before the next analysis.
    ///
    /// ```python
    /// sess.alter("R1", "resistance", 1050.0)
    /// ```
    pub fn alter(&self, py: Python<'_>, device: &str, param: &str, value: f64) -> PyResult<()> {
        let cmd = format!("alter @{device}[{param}] = {value}");
        self.with_backend(py, move |b| b.run_command(&cmd))
    }

    /// Return the SPICE netlist lines generated from the .ppr file.
    pub fn spice_lines(&self) -> Vec<String> {
        self.circuit.spice_lines.clone()
    }

    /// Get one vector by name from the last completed analysis.
    pub fn vector<'py>(&self, py: Python<'py>, name: &str)
        -> PyResult<Bound<'py, PyArray1<f64>>>
    {
        let name = name.to_string();
        let data = self.with_backend(py, move |b| b.get_vector(&name))?;
        Ok(PyArray1::from_vec_bound(py, data))
    }

    /// Start a transient analysis in the background. Returns a `SimFuture`.
    ///
    /// The session's backend is taken until the future is joined:
    /// ```python
    /// f1 = s1.tran_async("1n", "1u")
    /// f2 = s2.tran_async("1n", "1u")
    /// r1, r2 = join_all([f1, f2])   # both run in parallel
    /// ```
    pub fn tran_async(&self, step: &str, stop: &str) -> PyResult<SimFuture> {
        let backend = {
            let mut guard = self.backend.lock().unwrap();
            guard.take().ok_or_else(|| {
                PyRuntimeError::new_err("session already has a pending SimFuture")
            })?
        };
        let slot = Arc::clone(&self.backend);
        let cmd = format!("tran {step} {stop}");
        let handle = thread::spawn(move || {
            let mut b = backend;
            let r = b.run_analysis_simple(&cmd).map_err(|e| e.to_string());
            *slot.lock().unwrap() = Some(b);
            r
        });
        Ok(SimFuture { handle: Some(handle) })
    }

    /// Evaluate SOA checks compiled from `always @(step)` blocks.
    /// Raises `RuntimeError` on the first violation.
    pub fn check_soa(&self, py: Python<'_>) -> PyResult<()> {
        let checks = self.circuit.soa_checks.clone();
        for check in &checks {
            let name = check.meas_name.clone();
            let vals = self.with_backend(py, move |b| b.get_vector(&name))?;
            if let Some(&measured) = vals.first() {
                let violated = match check.op {
                    SoaOp::Gt => measured > check.threshold,
                    SoaOp::Ge => measured >= check.threshold,
                    SoaOp::Lt => measured < check.threshold,
                    SoaOp::Le => measured <= check.threshold,
                };
                if violated {
                    return Err(PyRuntimeError::new_err(format!(
                        "SOA violation: {} (measured {:.4e}, limit {:.4e})",
                        check.label, measured, check.threshold
                    )));
                }
            }
        }
        Ok(())
    }
}

// ── SimFuture ─────────────────────────────────────────────────────────────────

/// An in-flight ngspice analysis. Call `.join()` to block and collect results.
///
/// On completion the backend is automatically returned to the session, so the
/// session becomes usable again after join.
#[pyclass]
pub struct SimFuture {
    handle: Option<thread::JoinHandle<Result<AnalysisResult, String>>>,
}

#[pymethods]
impl SimFuture {
    /// Block until the analysis completes. Returns `{signal: np.ndarray}`.
    pub fn join<'py>(&mut self, py: Python<'py>)
        -> PyResult<HashMap<String, Bound<'py, PyArray1<f64>>>>
    {
        let handle = self.handle.take()
            .ok_or_else(|| PyRuntimeError::new_err("SimFuture already joined"))?;
        let result = py.allow_threads(|| handle.join().unwrap())
            .map_err(|e| PyRuntimeError::new_err(format!("async analysis failed: {e}")))?;
        vectors_to_py(py, result)
    }
}

// ── join_all ──────────────────────────────────────────────────────────────────

/// Wait for all futures; wall time ≈ slowest worker.
///
/// ```python
/// results = join_all([f1, f2, f3])
/// ```
#[pyfunction]
pub fn join_all<'py>(py: Python<'py>, futures: Vec<PyRefMut<'py, SimFuture>>)
    -> PyResult<Vec<HashMap<String, Bound<'py, PyArray1<f64>>>>>
{
    futures.into_iter().map(|mut f| f.join(py)).collect()
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Run an AC analysis and return vectors: frequency as Real, all others as Complex.
fn run_ac_analysis(
    b: &mut NgspiceBackend,
    cmd: &str,
) -> Result<HashMap<String, VectorData>, piperine_interpreter::InterpreterError> {
    b.start_analysis(cmd, false)?;
    let plot_name = loop {
        match b.poll_analysis()? {
            piperine_interpreter::AnalysisEvent::Done { plot_name, .. } => break plot_name,
            piperine_interpreter::AnalysisEvent::Event { .. } => {
                b.respond_to_analysis_event(piperine_common::EventAction::Continue)?;
            }
        }
    };
    let names = b.list_vectors(&plot_name)?;
    let mut vectors = HashMap::new();
    for name in names {
        // Try complex first; fall back to real. In AC analysis all vectors
        // (including "frequency") are stored as complex by ngspice.
        match b.get_complex_vector(&name) {
            Ok(pairs) => {
                if name == "frequency" {
                    // Extract real part only — imaginary part of frequency is 0.
                    let reals: Vec<f64> = pairs.iter().map(|(r, _)| *r).collect();
                    vectors.insert(name, VectorData::Real(reals));
                } else {
                    vectors.insert(name, VectorData::Complex(pairs));
                }
            }
            Err(_) => {
                if let Ok(data) = b.get_vector(&name) {
                    vectors.insert(name, VectorData::Real(data));
                }
            }
        }
    }
    Ok(vectors)
}

fn vectors_to_py<'py>(
    py: Python<'py>,
    result: AnalysisResult,
) -> PyResult<HashMap<String, Bound<'py, PyArray1<f64>>>> {
    let mut out = HashMap::new();
    for (name, data) in result.vectors {
        match data {
            VectorData::Real(v) => { out.insert(name, PyArray1::from_vec_bound(py, v)); }
            VectorData::Complex(pairs) => {
                let re: Vec<f64> = pairs.iter().map(|(r, _)| *r).collect();
                let im: Vec<f64> = pairs.iter().map(|(_, i)| *i).collect();
                out.insert(format!("{name}.re"), PyArray1::from_vec_bound(py, re));
                out.insert(format!("{name}.im"), PyArray1::from_vec_bound(py, im));
            }
        }
    }
    Ok(out)
}

// ── module ────────────────────────────────────────────────────────────────────

#[pymodule]
fn piperine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NgspiceSession>()?;
    m.add_class::<SimFuture>()?;
    m.add_function(wrap_pyfunction!(join_all, m)?)?;
    Ok(())
}
