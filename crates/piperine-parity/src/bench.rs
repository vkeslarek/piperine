//! A twin loaded through the whole chain: `.msl` source, type check, elaboration to an `Ir`, the
//! interpreter and the solver circuit. A failing stage panics with its name and every diagnostic.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{BranchKind, Ir};
use piperine_lang::elab::Elaborator;
use piperine_lang::types::Types;
use piperine_lang::pom::{Design, DesignDiagnostic, Reflect};
use piperine_solver::prelude::{BranchIdentifier, CircuitInstance, NodeIdentifier, ParamValue};
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::backend::Backend;

/// A twin as a bound circuit that answers to the `.cir` names.
///
/// The fields drop in order: the circuit, which holds the loaded library's modules, goes before
/// the library and before the cache directory the library was built in.
pub struct Bench {
    circuit: CircuitInstance,
    compiled: Option<Compiled>,
    _cache: Option<tempfile::TempDir>,
    fixture: String,
    top: String,
    ir: Ir,
    backend: Backend,
}

impl Bench {
    /// The twin of `tests/ngspice/<fixture>.msl`.
    pub fn twin_path(fixture: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice").join(format!("{fixture}.msl"))
    }

    /// Loads `tests/ngspice/<fixture>.msl` on the interpreter.
    pub fn load(fixture: &str) -> Bench {
        Self::load_on(fixture, Backend::Interp)
    }

    /// Loads `tests/ngspice/<fixture>.msl` on `backend`.
    pub fn load_on(fixture: &str, backend: Backend) -> Bench {
        Self::load_file_on(fixture, Self::twin_path(fixture), backend)
    }

    /// Loads the twin text at `path` on the interpreter, reported under the name `fixture`.
    pub fn load_file(fixture: &str, path: impl Into<PathBuf>) -> Bench {
        Self::load_file_on(fixture, path, Backend::Interp)
    }

    /// Loads the twin text at `path` on `backend`, reported under the name `fixture`. The Rust
    /// backend transpiles the design and builds it with `rustc` into a cache directory of this
    /// bench's own, unless `PIPERINE_CACHE_DIR` names one; a failure names its stage.
    pub fn load_file_on(fixture: &str, path: impl Into<PathBuf>, backend: Backend) -> Bench {
        let design = Design::builder().root_file(path).build();
        if !design.diagnostics().is_empty() {
            Self::fail(fixture, "parse", design.diagnostics());
        }
        let tops: Vec<String> = design.modules().into_iter().filter(|m| m.ports().is_empty()).filter_map(|m| m.name().map(str::to_string)).collect();
        let [top] = tops.as_slice() else {
            panic!("{fixture}: stage `parse`: expected one module without a port, found {tops:?}");
        };
        if let Err(found) = Types::check(&design, top) {
            Self::fail(fixture, "typeck", &found);
        }
        let ir = Elaborator::new(&design).run(top).unwrap_or_else(|found| Self::fail(fixture, "elaborate", &found));
        let (circuit, compiled, cache) = match backend {
            Backend::Interp => {
                let interpreter = match Interpreter::new(Arc::new(ir.clone())) {
                    Ok(interpreter) => interpreter,
                    Err(rejected) => panic!("{fixture}: stage `interpret`: {} entries\n{:#?}", rejected.entries.len(), rejected.entries),
                };
                let circuit = interpreter.circuit().unwrap_or_else(|error| panic!("{fixture}: stage `bind`: {error}"));
                (circuit, None, None)
            }
            Backend::Rust => {
                let (settings, cache) = Self::settings(fixture);
                let transpiler = Transpiler::new(Arc::new(ir.clone())).unwrap_or_else(|error| panic!("{fixture}: stage `transpile`: {error}"));
                let compiled = transpiler.compile(&settings).unwrap_or_else(|error| panic!("{fixture}: stage `compile`: {error}"));
                let circuit = compiled.circuit().unwrap_or_else(|error| panic!("{fixture}: stage `bind`: {error}"));
                (circuit, Some(compiled), cache)
            }
        };
        Bench { circuit, compiled, _cache: cache, fixture: fixture.to_string(), top: top.clone(), ir, backend }
    }

    /// The compiler settings of a Rust bench: those of the environment when it names a cache
    /// directory, else the environment's compiler with a fresh temporary cache, so a test run
    /// never writes the user's own cache.
    fn settings(fixture: &str) -> (Settings, Option<tempfile::TempDir>) {
        let named = std::env::var("PIPERINE_CACHE_DIR").ok().filter(|dir| !dir.is_empty());
        let cache = named.is_none().then(|| tempfile::tempdir().unwrap_or_else(|error| panic!("{fixture}: stage `settings`: a temporary cache: {error}")));
        let temporary = cache.as_ref().map(|dir| dir.path().to_string_lossy().into_owned());
        let settings = Settings::resolve(&|name| match (name, &temporary) {
            ("PIPERINE_CACHE_DIR", Some(dir)) => Some(dir.clone()),
            _ => std::env::var(name).ok(),
        })
        .unwrap_or_else(|error| panic!("{fixture}: stage `settings`: {error}"));
        (settings, cache)
    }

    /// The backend this bench runs on.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The compiled design of a Rust bench; `None` on the interpreter.
    pub fn compiled(&self) -> Option<&Compiled> {
        self.compiled.as_ref()
    }

    fn fail(fixture: &str, stage: &str, found: &[DesignDiagnostic]) -> ! {
        let rendered: Vec<String> = found.iter().map(|d| format!("{} (file {:?}): {} at {:?}", d.diagnostic.code, d.file, d.diagnostic.message, d.diagnostic.primary.0)).collect();
        panic!("{fixture}: stage `{stage}` failed with {} diagnostic(s):\n{}", found.len(), rendered.join("\n"))
    }

    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    pub fn circuit(&mut self) -> &mut CircuitInstance {
        &mut self.circuit
    }

    /// The path of an instance or node of the top module: `Top.<name>`.
    pub fn path(&self, name: &str) -> String {
        format!("{}.{name}", self.top)
    }

    /// The solver node of the top module's signal `name`.
    pub fn node(&self, name: &str) -> NodeIdentifier {
        let path = self.path(name);
        let node = self.ir.node_at(&path).unwrap_or_else(|| panic!("{}: no node `{path}`", self.fixture));
        NodeIdentifier::Anonymous(node.id().index())
    }

    /// The branch whose flow is the current of the source instance `name`: the private flow pin
    /// of its first potential branch.
    pub fn source(&self, name: &str) -> BranchIdentifier {
        let path = self.path(name);
        let instance = self.ir.instance_at(&path).unwrap_or_else(|| panic!("{}: no instance `{path}`", self.fixture));
        let branch = instance
            .body()
            .branches()
            .iter()
            .position(|b| b.kind == BranchKind::Potential)
            .unwrap_or_else(|| panic!("{}: `{path}` has no potential branch", self.fixture));
        BranchIdentifier::new(path, format!("b{branch}.flow"))
    }

    /// Sets a param of an instance of the top module, as `CircuitInstance::set` does.
    pub fn set(&mut self, instance: &str, param: &str, value: f64) {
        let path = self.path(instance);
        self.circuit.set(&path, param, ParamValue::Real(value)).unwrap_or_else(|error| panic!("{}: set {path}.{param}: {error}", self.fixture));
    }
}
