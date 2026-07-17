//! `_LiveSession` — compile once, `set`, re-run (LIVE-10..13).
//!
//! Unlike [`crate::module::_Module`], which forks the design and rebuilds a
//! fresh `SimSession` per analysis, a live session elaborates + JITs **once**
//! (`_Module::compile`) and then runs every analysis on the held, compiled
//! [`CircuitInstance`]. Parameter writes route through the solver's live-set
//! path (`CircuitInstance::set_element_param`, MD-18: restamp, never re-JIT),
//! so optimization loops pay one compilation total.
//!
//! Addressing is the PHDL/POM scheme (LIVE-01): flat instance labels
//! (elaboration flattens the top module) with bundle fields flattened to
//! `{param}_{field}` — the same names `Design::set_param` accepts.

use std::collections::HashMap;
use std::rc::Rc;

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use piperine_bench::{OpResult, SimSession};
use piperine_codegen::device::{CircuitBuildInfo, CircuitCompiler};
use piperine_lang::{Design, Value};
use piperine_solver::abi::{AnalogVariable, InitialValue, Invalidation};
use piperine_solver::prelude::{CircuitInstance, NodeIdentifier};

use crate::instance::InstanceResolver;
use crate::results::{_AcTrace, _NoiseTrace, _OpResult, _Trace};

/// `_LiveSession` — a compiled circuit held live across analyses (LIVE-10).
///
/// Owns the applied [`Design`] (the POM the circuit was compiled from — the
/// oracle for structural rebuilds and the instance resolver's hierarchy
/// source), the compiled [`CircuitInstance`], and its [`CircuitBuildInfo`]
/// (net map + per-instance kernels for result readbacks).
///
/// `unsendable`: same single-interpreter contract as [`crate::_Design`].
#[pyclass(module = "piperine", unsendable)]
pub struct _LiveSession {
    design: Rc<Design>,
    module: String,
    circuit: CircuitInstance,
    info: CircuitBuildInfo,
    rebuilds: usize,
    /// Scheduled live writes `(t, label, param, value)` for the next
    /// transient (LIVE-06 from Python): drained into
    /// `TransientSolver::schedule_set` when `tran` runs, in scheduling order
    /// (last-write-wins per param, one breakpoint per entry).
    pending_sets: Vec<(f64, String, String, f64)>,
    /// The most recent solved node voltages by net name — the source for
    /// the state carry across an auto rebuild (LIVE-15).
    last_voltages: HashMap<String, f64>,
    /// One-shot warm-start guess after an auto rebuild (LIVE-15): carried
    /// node voltages by net name, merged under any user nodeset/ic on the
    /// next analysis and cleared once that analysis succeeds. Dropped nets
    /// were already filtered out at rebuild time; new nets start cold.
    carry: HashMap<String, f64>,
}

impl _LiveSession {
    /// Build the one-and-only compilation for a module (called by
    /// [`crate::module::_Module::compile`]): fork the parent design, replay
    /// the staged overrides, apply them, lower + JIT, and hold everything.
    /// The held design gets a fresh, empty staging area — future structural
    /// sets stage onto it (LIVE-14), never onto the user's `_Design`.
    pub(crate) fn from_design(
        parent: &Design,
        module: &str,
        staged: &HashMap<(String, String), Value>,
    ) -> PyResult<Self> {
        let forked = parent.fork();
        for ((label, param), value) in staged {
            forked.set_param(label, param, value.clone());
        }
        let applied = forked
            .with_overrides_applied(module)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?
            .fork();
        let (circuit, info) = Self::build(&applied, module)?;
        Ok(Self {
            design: Rc::new(applied),
            module: module.to_string(),
            circuit,
            info,
            rebuilds: 0,
            pending_sets: Vec::new(),
            last_voltages: HashMap::new(),
            carry: HashMap::new(),
        })
    }

    /// Lower + JIT `module` of `design` into a runnable circuit — the same
    /// recipe as the bench's `SimSession::build_circuit`, minus plugins.
    fn build(design: &Design, module: &str) -> PyResult<(CircuitInstance, CircuitBuildInfo)> {
        let bodies = piperine_codegen::ir::lower_bodies(design)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        let mut compiler = CircuitCompiler::new(design, &bodies);
        let (mut circuit, info) = compiler
            .build_circuit_mapped(module)
            .map_err(|e| PyValueError::new_err(format!("{e}")))?;
        circuit
            .init_digital()
            .map_err(|e| PyRuntimeError::new_err(format!("{e}")))?;
        circuit.rebuild_digital_topology();
        Ok((circuit, info))
    }

    /// Surface a solver live-set error as the right Python exception, message
    /// verbatim (LIVE-11: same errors as the Rust path): unknown element /
    /// unknown parameter read as `KeyError` (lookup failures), an
    /// out-of-bounds value as `ValueError`, everything else `RuntimeError`.
    fn set_err(e: piperine_solver::prelude::Error) -> PyErr {
        let msg = format!("{e}");
        if msg.contains("no element labeled") || msg.contains("unknown parameter") {
            PyKeyError::new_err(msg)
        } else if msg.contains("out of bounds") {
            PyValueError::new_err(msg)
        } else {
            PyRuntimeError::new_err(msg)
        }
    }

    /// Surface a solver analysis error (mirrors `_Module::analysis_err`'s
    /// string contract over the solver error type).
    fn analysis_err(e: piperine_solver::prelude::Error) -> PyErr {
        PyRuntimeError::new_err(format!("{e}"))
    }

    /// Resolve a net name against the built circuit (ground names map to the
    /// reference node) — the live twin of the bench's `NetLookup`.
    fn node(&self, name: &str) -> PyResult<NodeIdentifier> {
        if piperine_lang::pom::is_ground(name) {
            return Ok(NodeIdentifier::Gnd);
        }
        self.info
            .nets
            .get(name)
            .cloned()
            .ok_or_else(|| PyKeyError::new_err(format!("net `{name}` is not addressable")))
    }

    /// Solver initial-value hints from a `{net: volts}` dict
    /// (`OpConfig.nodeset` / `TranConfig.ic`) — the live twin of the bench's
    /// `build_ivs`, with the post-rebuild carry (LIVE-15) merged under the
    /// user's map (user entries win). Ground keys have no index and are
    /// skipped; user-named unknown nets stay loud.
    fn ivs(
        &self,
        map: Option<HashMap<String, f64>>,
    ) -> PyResult<Vec<InitialValue<piperine_solver::abi::AnalogReference, f64>>> {
        let mut merged: HashMap<String, f64> = self.carry.clone();
        for (name, value) in map.into_iter().flatten() {
            // Validate user entries loudly (carry entries were filtered
            // against the current net map at rebuild time).
            self.node(&name)?;
            merged.insert(name, value);
        }
        let mut ivs = Vec::new();
        for (name, value) in merged {
            let node = self.node(&name)?;
            if let Some(reference) =
                self.circuit.netlist().reference_for(&AnalogVariable::Node(node))
            {
                ivs.push(InitialValue { reference: reference.clone(), value });
            }
        }
        Ok(ivs)
    }

    /// The [`InstanceResolver`] handed to result objects (PY-13), rooted at
    /// the held applied design.
    fn instance_resolver(&self) -> InstanceResolver {
        InstanceResolver::new(Rc::clone(&self.design), self.module.clone())
    }

    /// Auto re-elaboration on a structural set (LIVE-14): stage the write on
    /// the held POM design, re-apply + re-elaborate + recompile, carry the
    /// last solved node voltages by net name as the next solve's initial
    /// guess (LIVE-15), and bump the `rebuilds` notice. Any failure leaves
    /// the previous compiled circuit fully usable (LIVE-17) — the swap
    /// happens only after a successful build.
    fn auto_rebuild(&mut self, label: &str, param: &str, value: f64) -> PyResult<()> {
        self.design.set_param(label, param, Value::Real(value));
        let rebuilt = self
            .design
            .with_overrides_applied(&self.module)
            .map_err(|e| PyValueError::new_err(format!("{e}")))
            .map(|applied| applied.fork())
            .and_then(|applied| Self::build(&applied, &self.module).map(|built| (applied, built)));
        match rebuilt {
            Ok((applied, (circuit, info))) => {
                // Carry by net name: nets that survived keep their solved
                // voltage as the warm-start guess; dropped nets are
                // discarded here, new nets start cold (LIVE-15).
                self.carry = self
                    .last_voltages
                    .iter()
                    .filter(|(name, _)| info.nets.contains_key(*name))
                    .map(|(name, &v)| (name.clone(), v))
                    .collect();
                self.design = Rc::new(applied);
                self.circuit = circuit;
                self.info = info;
                self.rebuilds += 1;
                Ok(())
            }
            Err(e) => {
                // Un-stage the failed write so a later structural set does
                // not replay it, and keep the old circuit active (LIVE-17).
                self.design.clear_overrides();
                Err(PyValueError::new_err(format!(
                    "structural set `{label}`.`{param}` failed to re-elaborate: {e}; \
                     previous circuit still active"
                )))
            }
        }
    }

    /// Record the solved node voltages by net name (the LIVE-15 carry
    /// source) and consume any one-shot post-rebuild warm-start.
    fn record_voltages(&mut self, read: impl Fn(&NodeIdentifier) -> f64) {
        self.last_voltages = self
            .info
            .nets
            .iter()
            .map(|(name, node)| {
                let v = if *node == NodeIdentifier::Gnd { 0.0 } else { read(node) };
                (name.clone(), v)
            })
            .collect();
        self.carry.clear();
    }
}

#[pymethods]
impl _LiveSession {
    /// How many automatic structural rebuilds this session has performed
    /// (LIVE-14 notice; `0` until a structural set lands).
    #[getter]
    fn rebuilds(&self) -> usize {
        self.rebuilds
    }

    /// Live parameter write on the compiled circuit (LIVE-11): PHDL
    /// addressing (flat instance label + flattened param name), routed
    /// through the solver's `set_element_param` — restamp, never re-JIT
    /// (MD-18). Unknown label/param raise `KeyError` with the solver's
    /// message (candidates listed); an out-of-bounds value raises
    /// `ValueError`; no partial apply.
    fn set(&mut self, label: &str, param: &str, value: f64) -> PyResult<()> {
        match self
            .circuit
            .set_element_param(label, param, piperine_solver::abi::Value::Real(value))
        {
            // Structural write (optional-param presence flip): the solver
            // returns the typed `Rebuild` outcome without applying — the
            // session re-elaborates automatically (LIVE-14).
            Ok(inv) if inv >= Invalidation::Rebuild => self.auto_rebuild(label, param, value),
            Ok(_) => {
                // Mirror the write into the build info so device-internal
                // current readbacks (`.i(a, b)` on force-less two-terminal
                // devices) see the new value (same mirror as the bench's
                // `run_op_sweep`).
                if let Some(inst) = self.info.instances.iter_mut().find(|i| i.label == label)
                    && let Some(pidx) =
                        inst.kernel.param_names().iter().position(|n| n == param)
                {
                    inst.params[pidx] = value;
                }
                Ok(())
            }
            Err(e) => {
                // A parameter the compiled kernel does not carry can still
                // be a structural POM write (the elaboration may specialize
                // on it) — try the rebuild path; if the POM rejects it too,
                // surface the original solver error with its candidate
                // list (fail loud, LIVE-17: old circuit stays usable).
                let msg = format!("{e}");
                if msg.contains("unknown parameter")
                    && let Ok(()) = self.auto_rebuild(label, param, value)
                {
                    return Ok(());
                }
                Err(Self::set_err(e))
            }
        }
    }

    /// Run a DC operating point on the held circuit (LIVE-10). Signature and
    /// result shape are identical to `_Module::op` (LIVE-13 / PY-17).
    #[pyo3(signature = (nodeset=None, solver=None))]
    fn op(
        &mut self,
        nodeset: Option<HashMap<String, f64>>,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_OpResult> {
        let config = crate::module::_Module::solver_config(solver)?;
        let ivs = self.ivs(nodeset)?;
        let mut dc = self.circuit.dc(config.to_context()).map_err(Self::analysis_err)?;
        dc.policy = config.to_policy();
        dc.apply_initial_conditions(ivs);
        let result = dc.solve().map_err(Self::analysis_err)?;
        drop(dc);
        self.record_voltages(|node| result.get_node(node).unwrap_or(0.0));
        let digital = SimSession::snapshot_digital(&self.info, &self.circuit);
        let op = OpResult::new(result, digital, Rc::new(self.info.clone()));
        Ok(_OpResult::new(op).with_resolver(self.instance_resolver()))
    }

    /// Schedule a live parameter write at simulation time `t` for the next
    /// `tran` run (LIVE-06 from Python): the integrator lands exactly on `t`
    /// (unified breakpoint table) and the write applies there with the
    /// discontinuity edge rules. Addressing and errors are the solver's —
    /// unknown names fail loud when the set lands, same as the Rust path.
    fn schedule_set(&mut self, t: f64, label: &str, param: &str, value: f64) {
        self.pending_sets.push((t, label.to_string(), param.to_string(), value));
    }

    /// Run a transient on the held circuit (LIVE-10); same signature and
    /// result shape as `_Module::tran` (LIVE-13 / PY-17).
    #[pyo3(signature = (stop, step=None, start=0.0, ic=None, solver=None))]
    fn tran(
        &mut self,
        stop: f64,
        step: Option<f64>,
        start: f64,
        ic: Option<HashMap<String, f64>>,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_Trace> {
        let config = crate::module::_Module::solver_config(solver)?;
        let ivs = self.ivs(ic)?;
        let opts = match step {
            Some(dt) if dt > 0.0 => {
                piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt)
            }
            _ => piperine_solver::prelude::TransientAnalysisOptions::new(stop, stop * 1e-3),
        }
        .with_record_from(start);
        let mut tran = self
            .circuit
            .transient(opts, config.to_context())
            .map_err(Self::analysis_err)?;
        tran.policy = config.to_policy();
        tran.apply_initial_conditions(ivs);
        let scheduled: Vec<(f64, String, String, f64)> = self.pending_sets.drain(..).collect();
        for (t, label, param, value) in &scheduled {
            tran.schedule_set(*t, label, param, piperine_solver::abi::Value::Real(*value));
        }
        let result = tran.solve().map_err(Self::analysis_err)?;
        drop(tran);
        if let Some(last) = result.iter().last() {
            self.record_voltages(|node| last.get_node(node).unwrap_or(0.0));
        }
        // The run ends with the scheduled values applied — mirror them into
        // the build info (scheduling order = last-write-wins) so post-run
        // `.i()` readbacks see the final parameters.
        for (_, label, param, value) in &scheduled {
            if let Some(inst) = self.info.instances.iter_mut().find(|i| &i.label == label)
                && let Some(pidx) = inst.kernel.param_names().iter().position(|n| n == param)
            {
                inst.params[pidx] = *value;
            }
        }
        let trace = piperine_bench::Trace::new(result, Rc::new(self.info.clone()));
        Ok(_Trace::new(trace).with_resolver(self.instance_resolver()))
    }

    /// Run an AC small-signal sweep on the held circuit (LIVE-10); same
    /// signature and result shape as `_Module::ac` (LIVE-13 / PY-17).
    #[pyo3(signature = (fstart, fstop, points=100, logarithmic=true, solver=None))]
    fn ac(
        &mut self,
        fstart: f64,
        fstop: f64,
        points: usize,
        logarithmic: bool,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_AcTrace> {
        let config = crate::module::_Module::solver_config(solver)?;
        let opts = piperine_solver::prelude::AcSweepAnalysisOptions {
            start_frequency: fstart,
            stop_frequency: fstop,
            steps: points,
            logarithmic,
        };
        let mut ac = self.circuit.ac(config.to_context()).map_err(Self::analysis_err)?;
        ac.policy = config.to_policy();
        let result = ac.solve_sweep(opts).map_err(Self::analysis_err)?;
        drop(ac);
        let trace = piperine_bench::AcTrace::new(result, Rc::new(self.info.clone()));
        Ok(_AcTrace::new(trace))
    }

    /// Run an output-referred noise analysis on the held circuit (LIVE-10);
    /// same signature and result shape as `_Module::noise` (LIVE-13 / PY-17).
    #[pyo3(signature = (out, fstart, fstop, points=100, reference="gnd", logarithmic=true, solver=None))]
    #[allow(clippy::too_many_arguments)]
    fn noise(
        &mut self,
        out: &str,
        fstart: f64,
        fstop: f64,
        points: usize,
        reference: &str,
        logarithmic: bool,
        solver: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<_NoiseTrace> {
        let config = crate::module::_Module::solver_config(solver)?;
        let out = self.node(out)?;
        let reference = self.node(reference)?;
        let opts = piperine_solver::prelude::NoiseAnalysisOptions {
            sweep_options: piperine_solver::prelude::AcSweepAnalysisOptions {
                start_frequency: fstart,
                stop_frequency: fstop,
                steps: points,
                logarithmic,
            },
            output_node: out,
            reference_node: reference,
            input_source_name: None,
        };
        let result = self
            .circuit
            .noise(opts, config.to_context())
            .map_err(Self::analysis_err)?
            .solve()
            .map_err(Self::analysis_err)?;
        let trace = piperine_bench::NoiseTrace::new(result);
        Ok(_NoiseTrace::new(trace))
    }
}

#[cfg(test)]
mod tests {
    use pyo3::prelude::*;
    use pyo3::types::PyModule;

    /// The runnable divider fixture (mirrors `lib.rs::ANALYSIS_PHDL`):
    /// `mid = 5·r_bot/(r_top+r_bot)` — 3 k/2 k → 2.0 V; `r_top = 2e3` → 2.5 V.
    const DIVIDER: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

    /// LIVE-10 (single build) + LIVE-11 (errors) + LIVE-13 (result shapes):
    /// `module.compile()` returns a live session whose `set` + `op` loop
    /// matches per-point fresh builds exactly; the four analyses return the
    /// same pyclass types as `_Module`'s; unknown label/param and
    /// out-of-scope addressing fail loud with the solver's message.
    /// (The compile-count proof is the isolated `tests/live_session.rs`
    /// binary — a lib test would share the process-global counter with
    /// concurrent tests.)
    #[test]
    fn live_session_set_op_matches_fresh_builds_and_keeps_result_shapes() -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_live_t6_test.phdl");
        std::fs::write(&path, DIVIDER)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let m = PyModule::new(py, "_piperine")?;
            crate::_piperine(&m)?;
            let design = m.getattr("load")?.call1((path_str,))?;
            let module = design.getattr("module")?.call1(("Divider",))?;
            let session = module.getattr("compile")?.call0()?;

            let class_name = |obj: &Bound<'_, PyAny>| -> PyResult<String> {
                obj.getattr("__class__")?.getattr("__name__")?.extract::<String>()
            };

            // LIVE-13: identical pyclass types to `_Module`'s analyses.
            let op = session.getattr("op")?.call0()?;
            assert_eq!(class_name(&op)?, "_OpResult");
            let tran = session.getattr("tran")?.call1((1e-4, 1e-6))?;
            assert_eq!(class_name(&tran)?, "_Trace");
            let ac = session.getattr("ac")?.call1((1.0, 1e6, 5))?;
            assert_eq!(class_name(&ac)?, "_AcTrace");
            let noise = session.getattr("noise")?.call1(("mid", 1.0, 1e6, 5))?;
            assert_eq!(class_name(&noise)?, "_NoiseTrace");

            // Baseline: 5·2/(3+2) = 2.0 V, same as `_Module::op`.
            let v = op.getattr("v")?.call1(("mid",))?.extract::<f64>()?;
            assert!((v - 2.0).abs() < 1e-9, "baseline mid = 2.0 V, got {v}");

            // set + op loop equals per-point fresh builds (staged path).
            for r in [1e3, 2e3, 4e3, 6e3] {
                session.getattr("set")?.call1(("r_top", "r", r))?;
                let live = session
                    .getattr("op")?
                    .call0()?
                    .getattr("v")?
                    .call1(("mid",))?
                    .extract::<f64>()?;
                module.getattr("stage")?.call1(("r_top", "r", r))?;
                let fresh = module
                    .getattr("op")?
                    .call0()?
                    .getattr("v")?
                    .call1(("mid",))?
                    .extract::<f64>()?;
                assert!(
                    (live - fresh).abs() < 1e-9,
                    "r_top = {r}: live {live} V vs fresh build {fresh} V"
                );
            }

            // The build info mirror keeps `.i()` readbacks on the new value:
            // r_top = 6 k, r_bot = 2 k → i = 5/8k = 0.625 mA through r_top.
            let i = session
                .getattr("op")?
                .call0()?
                .getattr("i")?
                .call1(("vin", "mid"))?
                .extract::<f64>()?;
            assert!((i - 0.625e-3).abs() < 1e-9, "i(r_top) = 0.625 mA, got {i}");

            // LIVE-11: unknown label → KeyError echoing the path.
            let err = session
                .getattr("set")?
                .call1(("nope", "r", 1.0))
                .expect_err("unknown label must raise");
            assert!(err.is_instance_of::<pyo3::exceptions::PyKeyError>(py), "{err}");
            assert!(format!("{err}").contains("nope"), "{err}");

            // LIVE-11: unknown param → KeyError listing the candidates.
            let err = session
                .getattr("set")?
                .call1(("r_top", "bogus", 1.0))
                .expect_err("unknown param must raise");
            assert!(err.is_instance_of::<pyo3::exceptions::PyKeyError>(py), "{err}");
            let msg = format!("{err}");
            assert!(msg.contains("bogus") && msg.contains("available parameters"), "{msg}");

            // LIVE-14 notice starts at zero (no structural set yet).
            let rebuilds = session.getattr("rebuilds")?.extract::<usize>()?;
            assert_eq!(rebuilds, 0);
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }

    /// The dio-shaped structural fixture: an exponential diode whose
    /// optional `ns` (`Real? = none`) adds a sidewall leak `ns·1e-4 S` when
    /// given (`get_or` → `$param_given`, the presence machinery the real
    /// `spice::diode` sidewall uses).
    const DIO: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) {
    param dc: Real = 5.0;
}
analog Vsrc { V(p, n) <- dc; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e4;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Dio(inout p: Electrical, inout n: Electrical) {
    param isat: Real = 1e-14;
    param vt: Real = 0.02585;
    param ns: Real? = none;
}
analog Dio {
    I(p, n) <+ isat * (exp(V(p, n) / vt) - 1.0);
    I(p, n) <+ ns.get_or(0.0) * 1e-4 * V(p, n);
}

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : Vsrc(.p = vin, .n = gnd) {};
    r1 : Resistor(.p = vin, .n = out) {};
    d1 : Dio(.p = out, .n = gnd) {};
}
";

    /// LIVE-14/15/17: a structural set (`ns` none→given) auto re-elaborates
    /// with a visible `rebuilds` notice and the sidewall behavior appears
    /// (result equals a fresh staged build); node voltages carry by net name
    /// as the next solve's warm start (fewer Newton iterations than a cold
    /// build); a failing re-elaboration surfaces the error and keeps the
    /// previous circuit fully usable.
    #[test]
    fn structural_set_auto_rebuilds_with_state_carry_and_failure_keeps_old_circuit()
    -> PyResult<()> {
        let path = std::env::temp_dir().join("piperine_python_live_t9_test.phdl");
        std::fs::write(&path, DIO)?;
        let path_str = path
            .to_str()
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("non-utf8 temp path"))?;

        let outcome = Python::with_gil(|py| -> PyResult<()> {
            let m = PyModule::new(py, "_piperine")?;
            crate::_piperine(&m)?;
            let design = m.getattr("load")?.call1((path_str,))?;
            let module = design.getattr("module")?.call1(("Top",))?;
            let session = module.getattr("compile")?.call0()?;
            let v_out = |op: &Bound<'_, PyAny>| -> PyResult<f64> {
                op.getattr("v")?.call1(("out",))?.extract::<f64>()
            };
            let iters = |op: &Bound<'_, PyAny>| -> PyResult<usize> {
                op.getattr("stats")?.getattr("newton_iterations")?.extract::<usize>()
            };

            // Baseline: bottom diode only, no sidewall.
            let op0 = session.getattr("op")?.call0()?;
            let v0 = v_out(&op0)?;
            assert!(v0 > 0.5 && v0 < 0.8, "diode drop expected, got {v0}");
            assert_eq!(session.getattr("rebuilds")?.extract::<usize>()?, 0);

            // Structural set: ns none → 1.2. Auto rebuild, visible notice.
            session.getattr("set")?.call1(("d1", "ns", 1.2))?;
            assert_eq!(
                session.getattr("rebuilds")?.extract::<usize>()?,
                1,
                "structural set must report the rebuild (LIVE-14)"
            );

            // Sidewall appears and the result equals a fresh staged build.
            let op1 = session.getattr("op")?.call0()?;
            let v1 = v_out(&op1)?;
            assert!(v0 - v1 > 1e-3, "sidewall leak must lower v(out): {v0} -> {v1}");
            module.getattr("stage")?.call1(("d1", "ns", 1.2))?;
            let fresh = module.getattr("op")?.call0()?;
            let v_fresh = v_out(&fresh)?;
            assert!(
                (v1 - v_fresh).abs() <= 1e-3 * v_fresh.abs() + 1e-6,
                "rebuilt session {v1} V vs fresh staged build {v_fresh} V (reltol 1e-3)"
            );

            // LIVE-15: the rebuilt session's op warm-started from the
            // carried voltages — fewer Newton iterations than the cold
            // fresh build of the identical circuit.
            let warm = iters(&op1)?;
            let cold = iters(&fresh)?;
            assert!(
                warm < cold,
                "carried-state warm start must save iterations: warm {warm} vs cold {cold}"
            );

            // After the rebuild `ns` is given — a further ns write is a
            // plain restamp (no second rebuild), matching the oracle.
            session.getattr("set")?.call1(("d1", "ns", 3.0))?;
            assert_eq!(session.getattr("rebuilds")?.extract::<usize>()?, 1);
            module.getattr("stage")?.call1(("d1", "ns", 3.0))?;
            let v_live = v_out(&session.getattr("op")?.call0()?)?;
            let v_oracle = v_out(&module.getattr("op")?.call0()?)?;
            assert!(
                (v_live - v_oracle).abs() <= 1e-3 * v_oracle.abs() + 1e-6,
                "{v_live} vs {v_oracle} (reltol 1e-3)"
            );

            // LIVE-17: a bogus param fails loud (the rebuild path rejects
            // it too), no rebuild is recorded, and the session still solves
            // on the previous circuit.
            let err = session
                .getattr("set")?
                .call1(("d1", "bogus", 1.0))
                .expect_err("bogus param must fail");
            assert!(err.is_instance_of::<pyo3::exceptions::PyKeyError>(py), "{err}");
            assert!(format!("{err}").contains("bogus"), "{err}");
            assert_eq!(session.getattr("rebuilds")?.extract::<usize>()?, 1);
            let v_after = v_out(&session.getattr("op")?.call0()?)?;
            assert!(
                (v_after - v_live).abs() <= 1e-3 * v_live.abs() + 1e-6,
                "previous circuit must stay usable after a failed set: {v_after} vs {v_live}"
            );
            Ok(())
        });
        let _ = std::fs::remove_file(&path);
        outcome
    }
}
