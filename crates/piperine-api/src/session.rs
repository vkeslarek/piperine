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
use piperine_solver::prelude::{CircuitInstance, Solver};
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::backend::Backend;
use crate::failure::Failure;
use crate::probe::Probe;
use crate::record::RecordSink;
use crate::table::Table;
use crate::project::Project;
use crate::request::{Analysis, RunSpec};

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
        let (circuit, compiled) = match backend {
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
        Ok(Session { circuit: Some(circuit), ir, compiled, top, backend, spec: spec.clone(), probes })
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
        let _ = sink;
        let circuit = self.circuit.take().ok_or_else(|| Failure::Backend("this session has already run: open another".into()))?;
        let mut solver = Solver::new(circuit).build();
        match &self.spec.analysis {
            Analysis::Op => {
                let result = solver.dc().and_then(|mut dc| dc.solve()).map_err(|error| Failure::Solver { analysis: "op", error })?;
                let bound = solver.circuit().bound();
                // a discrete net by its path: its index in the digital plan is its id
                let net = |path: &str| {
                    let b = bound?;
                    let at = b.digital.plan().nets.iter().position(|n| n.name == path)?;
                    Some(b.digital.read(NetId(at as u32)).to_vec())
                };
                Table::operating_point(&self.probes, &result, &net)
            }
            other => Err(Failure::Backend(format!("the `{}` analysis is not supported by `Session::run` yet", other.name()))),
        }
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
