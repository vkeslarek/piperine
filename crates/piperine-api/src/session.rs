//! `Session`: one run request taken from source to a circuit bound on a backend (CLI-04). The
//! top is chosen (the `top` key, else the one module without ports of the root crate), the
//! design is type-checked and elaborated, and the backend builds it once: the interpreter, or
//! the transpiler, which may call `rustc`. Every failure before an analysis happens here, each
//! naming its stage. No analysis ever compiles.

use std::path::PathBuf;
use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic, Reflect};
use piperine_lang::types::Types;
use piperine_solver::prelude::CircuitInstance;
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::backend::Backend;
use crate::failure::Failure;
use crate::project::Project;
use crate::request::RunSpec;

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
        Ok(Session { circuit: Some(circuit), ir, compiled, top, backend, spec: spec.clone() })
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

    fn diagnosed(stage: &'static str, design: Design, found: Vec<DesignDiagnostic>) -> Failure {
        Failure::Diagnostics { stage, design: Box::new(design), found }
    }
}
