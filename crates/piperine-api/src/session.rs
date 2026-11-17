//! `Session`: one run request taken from source to a circuit bound on a backend (CLI-04). The
//! top is chosen (the `top` key, else the one module without ports of the root crate), the
//! design is type-checked and elaborated, and the backend builds it once: the interpreter, or
//! the transpiler, which may call `rustc`. Every failure before an analysis happens here, each
//! naming its stage, and the run's `print` entries are resolved. No analysis ever compiles.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use piperine_digital_solver::net::NetId;
use piperine_interp::Interpreter;
use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic, Reflect};
use piperine_lang::types::Types;
use piperine_solver::prelude::{AcSweepAnalysisOptions, CircuitInstance, Context, ParamValue, Solver, TransientAnalysisOptions};
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::backend::Backend;
use crate::failure::Failure;
use crate::probe::Probe;
use crate::record::{Delivery, RecordSink, Streams};
use crate::table::Table;
use crate::project::Project;
use crate::request::{Analysis, Grid, RunSpec};

/// How the design of a session was built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Built {
    /// `rustc` built it now; `source` is the generated Rust.
    Compiled { source: PathBuf },
    /// The cache held it; `source` is the generated Rust.
    Cached { source: PathBuf },
    /// The interpreter runs it; nothing was compiled.
    Interpreted,
}

/// A design compiled onto a backend, ready to run one request. The fields drop in order: the
/// circuit, which holds the loaded library's modules, before the library.
pub struct Session {
    circuit: Option<CircuitInstance>,
    ir: Arc<Ir>,
    compiled: Option<Compiled>,
    top: String,
    backend: Backend,
    spec: RunSpec,
    probes: Vec<Probe>,
    /// The param a `dc` run sweeps, resolved against the design.
    swept: Option<ParamPath>,
}

/// A param of an instance of the design, by the instance's full path.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamPath {
    pub instance: String,
    pub param: String,
}

impl ParamPath {
    /// `entry`, `<instance>.<param>` split at the last dot, the instance in full or relative to
    /// `top`; `E0801` when it names no param of an instance of `ir`.
    pub fn resolve(ir: &Ir, top: &str, entry: &str) -> Result<ParamPath, Failure> {
        let nothing = |why: &str| Failure::Host { code: "E0801", message: format!("`{entry}` names no param of the design: {why}") };
        let (path, param) = entry.rsplit_once('.').ok_or_else(|| nothing("a param path is <instance>.<param>"))?;
        let instance = [path.to_string(), format!("{top}.{path}")].into_iter().find_map(|p| ir.instance_at(&p)).ok_or_else(|| nothing("no instance of that path"))?;
        if !instance.body().params().iter().any(|p| p.name == param) {
            return Err(nothing(&format!("the instance `{}` has no param `{param}`", instance.path())));
        }
        Ok(ParamPath { instance: instance.path().to_string(), param: param.to_string() })
    }

    /// The full path, `<instance>.<param>`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.instance, self.param)
    }
}

impl Session {
    /// `spec` of `project`, from source to a bound circuit on the backend `spec.backend` names
    /// (`rust` when it names none); `settings` locate `rustc` and the cache.
    pub fn open(project: &Project, spec: &RunSpec, settings: &Settings) -> Result<Session, Failure> {
        let backend = Backend::choose(None, None, spec.backend.as_deref())?;
        let design = project.design(spec)?;
        if !design.diagnostics().is_empty() {
            let found = design.diagnostics().to_vec();
            return Err(Failure::Diagnostics { stage: "parse", design: Box::new(design), found });
        }
        let top = Session::top_of(&design, spec)?;
        if let Err(found) = Types::check(&design, &top) {
            return Err(Session::diagnosed("typeck", design, found));
        }
        let ir = match Elaborator::new(&design).run(&top) {
            Ok(ir) => Arc::new(ir),
            Err(found) => return Err(Session::diagnosed("elaborate", design, found)),
        };
        let probes = Session::probes_of(&ir, spec)?;
        let swept = match &spec.analysis {
            Analysis::Dc { sweep, .. } => Some(ParamPath::resolve(&ir, ir.top().path(), sweep)?),
            _ => None,
        };
        // every param path resolves before anything compiles: a typo costs no `rustc`
        let params = spec.params.iter().map(|(entry, value)| Ok((ParamPath::resolve(&ir, ir.top().path(), entry)?, entry, *value))).collect::<Result<Vec<_>, Failure>>()?;
        let (mut circuit, compiled) = match backend {
            Backend::Interp => {
                let interpreter = Interpreter::new(ir.clone()).map_err(|rejected| {
                    let entries: Vec<String> = rejected.entries.iter().map(|e| format!("  {}: {}", e.item, e.what)).collect();
                    Failure::Backend(format!("the interpreter cannot run this design (use --backend rust):\n{}", entries.join("\n")))
                })?;
                (interpreter.circuit().map_err(|e| Failure::Backend(format!("the interpreter cannot bind the design: {e}")))?, None)
            }
            Backend::Rust => {
                let transpiler = Transpiler::new(ir.clone()).map_err(|e| Failure::Backend(e.to_string()))?;
                let compiled = transpiler.compile(settings).map_err(|e| Failure::Backend(e.to_string()))?;
                let circuit = compiled.circuit().map_err(|e| Failure::Backend(format!("the compiled design cannot bind: {e}")))?;
                (circuit, Some(compiled))
            }
        };
        // the table's params, then the flags', in the order given: the design's own check may
        // refuse a value, and then no analysis runs
        for (path, entry, value) in &params {
            let refused = |error| Failure::Host { code: "E0802", message: format!("`{entry}` refuses the value {value}: {error}") };
            circuit.set(&path.instance, &path.param, ParamValue::Real(*value)).map_err(refused)?;
        }
        Ok(Session { circuit: Some(circuit), ir, compiled, top, backend, spec: spec.clone(), probes, swept })
    }

    /// The top module's name.
    pub fn top(&self) -> &str {
        &self.top
    }

    /// The backend the design runs on.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The elaborated design.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// The request this session runs.
    pub fn spec(&self) -> &RunSpec {
        &self.spec
    }

    /// The columns the result shows: the run's `print` entries resolved, in order, or the top's
    /// own signals and ports when it has none.
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// How the design was built: compiled now, found in the cache, or interpreted.
    pub fn built(&self) -> Built {
        match &self.compiled {
            Some(c) if c.report().cache_hit => Built::Cached { source: c.source_path() },
            Some(c) => Built::Compiled { source: c.source_path() },
            None => Built::Interpreted,
        }
    }

    /// The bound circuit, for a host that runs an analysis of its own; `None` once the
    /// session's run has taken it.
    pub fn circuit(&mut self) -> Option<&mut CircuitInstance> {
        self.circuit.as_mut()
    }

    /// The request's analysis on the bound circuit, its result as a table; the records of the
    /// run reach `sink` as the solver delivers them. A session runs once: the analysis takes its
    /// circuit.
    pub fn run(&mut self, sink: Rc<RefCell<dyn RecordSink>>) -> Result<Table, Failure> {
        let circuit = self.circuit.take().ok_or_else(|| Failure::Backend("this session has already run: open another".into()))?;
        let failure = Rc::new(RefCell::new(None));
        let delivery = Box::new(Delivery { streams: Streams::of(self.ir.clone()), sink, failure: failure.clone() });
        let table = self.analysis(circuit, delivery);
        // a record that could not be decoded is a defect the run reports first: the records
        // before it were delivered, none after it was guessed
        match failure.borrow_mut().take() {
            Some(broken) => Err(broken),
            None => table,
        }
    }

    /// The request's analysis on `circuit`, every observation of the solver to `delivery`.
    fn analysis(&self, circuit: CircuitInstance, delivery: Box<Delivery>) -> Result<Table, Failure> {
        match &self.spec.analysis {
            Analysis::Op => {
                let mut solver = Solver::new(circuit).with_sink(delivery).build();
                let result = solver.dc().and_then(|mut dc| dc.solve()).map_err(|error| Failure::Solver { analysis: "op", error })?;
                let circuit = solver.circuit();
                let net = |path: &str| Session::discrete(circuit, path);
                Table::operating_point(&self.probes, &result, &net)
            }
            Analysis::Transient { stop, max_step, from, .. } => {
                // design "How an analysis reaches the solver": `max_step` is the first step and
                // the largest; without it the first step is `stop / 1000` and the largest the
                // solver's default; `from` is the first time kept (contract D10)
                let mut options = match max_step {
                    Some(step) => TransientAnalysisOptions::new(*stop, *step).with_dt_max(*step),
                    None => TransientAnalysisOptions::new(*stop, stop / 1000.0),
                };
                if let Some(from) = from {
                    options = options.with_record_from(*from);
                }
                let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(delivery).build();
                let result = solver.tran().and_then(|mut tran| tran.solve()).map_err(|error| Failure::Solver { analysis: "transient", error })?;
                Table::transient(&self.probes, &result)
            }
            Analysis::Dc { from, to, step, .. } => {
                let mut circuit = circuit;
                let swept = self.swept.as_ref().ok_or_else(|| Failure::Backend("a dc run resolved no param to sweep".into()))?;
                // `from + k*step` for every k while the point is not past `to`; the tolerance on
                // k absorbs the rounding of `(to - from) / step` (a whole number of steps that
                // lands a hair under)
                let count = ((to - from) / step + 1e-9).floor() as usize + 1;
                let mut points = Vec::with_capacity(count);
                for k in 0..count {
                    let value = from + k as f64 * step;
                    let refused = |error| Failure::Host { code: "E0802", message: format!("`{}` refuses {value}: {error}", swept.label()) };
                    circuit.set(&swept.instance, &swept.param, ParamValue::Real(value)).map_err(refused)?;
                    let result = circuit.dc(Context::default()).and_then(|mut dc| dc.solve()).map_err(|error| Failure::Solver { analysis: "dc", error })?;
                    let net = |path: &str| Session::discrete(&circuit, path);
                    points.push((value, Table::point(&self.probes, &result, &net)?));
                }
                Ok(Table::sweep(&swept.label(), &self.probes, points))
            }
            Analysis::Ac { from, to, grid } => {
                let options = AcSweepAnalysisOptions {
                    start_frequency: *from,
                    stop_frequency: *to,
                    steps: grid.steps(*from, *to),
                    logarithmic: matches!(grid, Grid::PerDecade(_)),
                };
                let mut solver = Solver::new(circuit).with_sink(delivery).build();
                let result = solver.ac().and_then(|mut ac| ac.solve_sweep(options)).map_err(|error| Failure::Solver { analysis: "ac", error })?;
                Table::frequency(&self.probes, &result)
            }
        }
    }

    /// The bytes of the discrete net at `path` in the bound design: its index in the digital
    /// plan is its id.
    fn discrete(circuit: &CircuitInstance, path: &str) -> Option<Vec<u8>> {
        let b = circuit.bound()?;
        let at = b.digital.plan().nets.iter().position(|n| n.name == path)?;
        Some(b.digital.read(NetId(at as u32)).to_vec())
    }

    /// The `top` key, else the one module without ports of the root crate (`P0011` when there
    /// is none or several, the candidates listed).
    fn top_of(design: &Design, spec: &RunSpec) -> Result<String, Failure> {
        if let Some(top) = &spec.top {
            return Ok(top.clone());
        }
        let root = design.root_crate();
        let candidates: Vec<&str> = root.modules().into_iter().filter(|m| m.ports().is_empty()).filter_map(|m| m.name()).collect();
        match candidates.as_slice() {
            [one] => Ok(one.to_string()),
            [] => Err(Failure::Host {
                code: "P0011",
                message: format!("no top: the crate `{}` holds no module without ports; name one with `top`", root.name()),
            }),
            several => Err(Failure::Host {
                code: "P0011",
                message: format!("no top: the crate `{}` holds several modules without ports ({}); name one with `top`", root.name(), several.join(", ")),
            }),
        }
    }

    /// The run's `print` entries resolved against `ir` (`E0801` for one that names nothing),
    /// or the top's defaults.
    fn probes_of(ir: &Ir, spec: &RunSpec) -> Result<Vec<Probe>, Failure> {
        let top = ir.top().path();
        if spec.print.is_empty() {
            return Ok(Probe::defaults(ir, top));
        }
        spec.print.iter().map(|entry| Probe::resolve(ir, top, entry)).collect()
    }

    fn diagnosed(stage: &'static str, design: Design, found: Vec<DesignDiagnostic>) -> Failure {
        Failure::Diagnostics { stage, design: Box::new(design), found }
    }
}
