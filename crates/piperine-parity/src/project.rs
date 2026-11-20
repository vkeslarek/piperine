//! `Project`: a crate graph read from disk (a root crate and the dependency crates it names),
//! taken through type check and elaboration to an `Ir`, and through the transpiler to a circuit
//! on the solver. It is how the tests of `msl-digital` run MSL that places items of another
//! package (the buck) without a host API. A stage that refuses the design is returned with its
//! name, never a panic.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use piperine_ir::Ir;
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{CrateGraph, Design, DesignDiagnostic};
use piperine_lang::types::Types;
use piperine_solver::prelude::CircuitInstance;
use piperine_transpile::{Compiled, Settings, Transpiler};

/// The name the root crate of a project takes in its graph.
const ROOT: &str = "bench";

/// A root crate and its named dependency crates, each by the path of its root file.
#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    dependencies: Vec<(String, PathBuf)>,
    cache: Option<PathBuf>,
}

/// A design a stage refused: the stage (`graph`, `parse`, `typeck`, `elaborate`, `transpile`,
/// `compile`, `bind`) and what it said.
#[derive(Debug, Clone)]
pub struct Rejection {
    pub stage: &'static str,
    pub diagnostics: Vec<DesignDiagnostic>,
    pub message: String,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stage `{}`: {}", self.stage, self.message)?;
        for d in &self.diagnostics {
            write!(f, "\n  {} (file {:?}): {}", d.diagnostic.code, d.file, d.diagnostic.message)?;
        }
        Ok(())
    }
}

/// A compiled design bound to the solver. The fields drop in order: the circuit, which holds the
/// loaded library's modules, before the library.
pub struct Running {
    pub circuit: CircuitInstance,
    pub ir: Arc<Ir>,
    compiled: Compiled,
}

impl Running {
    /// The compiled library the circuit runs.
    pub fn compiled(&self) -> &Compiled {
        &self.compiled
    }
}

impl Project {
    /// A project whose root crate is the file `root`.
    pub fn new(root: &Path) -> Project {
        Project { root: root.to_path_buf(), dependencies: Vec::new(), cache: None }
    }

    /// This project with a dependency crate called `name`, rooted at the file `lib`.
    pub fn dependency(mut self, name: &str, lib: &Path) -> Project {
        self.dependencies.push((name.to_string(), lib.to_path_buf()));
        self
    }

    /// This project building its compiled library in `dir` (a test passes a directory under its
    /// own temporary space, so the user's cache is never written).
    pub fn cache(mut self, dir: &Path) -> Project {
        self.cache = Some(dir.to_path_buf());
        self
    }

    /// The design of the crate graph, or the stage that refused it.
    pub fn design(&self) -> Result<Design, Rejection> {
        let mut graph = CrateGraph::new();
        let refused = |message: String| Rejection { stage: "graph", diagnostics: Vec::new(), message };
        let root = graph.add_crate(ROOT, &self.root).map_err(|e| refused(format!("{e:?}")))?;
        for (name, lib) in &self.dependencies {
            let id = graph.add_crate(name, lib).map_err(|e| refused(format!("{e:?}")))?;
            graph.add_dependency(root, name, id).map_err(|e| refused(format!("{e:?}")))?;
        }
        let design = Design::load_crates(graph);
        if !design.diagnostics().is_empty() {
            return Err(Self::diagnosed("parse", design.diagnostics().to_vec()));
        }
        Ok(design)
    }

    /// `top`, a module of the root crate, type-checked and elaborated.
    pub fn elaborate(&self, top: &str) -> Result<Ir, Rejection> {
        let design = self.design()?;
        // every item is typed, reached from the top or not, as `Session::elaborate` does (S3, GN-09)
        Types::check_model(&design).map_err(|found| Self::diagnosed("typeck", found))?;
        Types::check(&design, top).map_err(|found| Self::diagnosed("typeck", found))?;
        Elaborator::new(&design).run(top).map_err(|found| Self::diagnosed("elaborate", found))
    }

    /// `top` elaborated, transpiled, compiled and bound to the solver.
    pub fn compile(&self, top: &str) -> Result<Running, Rejection> {
        let ir = Arc::new(self.elaborate(top)?);
        let refused = |stage: &'static str, message: String| Rejection { stage, diagnostics: Vec::new(), message };
        let settings = self.settings().map_err(|e| refused("settings", e))?;
        let transpiler = Transpiler::new(ir.clone()).map_err(|e| refused("transpile", e.to_string()))?;
        let compiled = transpiler.compile(&settings).map_err(|e| refused("compile", e.to_string()))?;
        let circuit = compiled.circuit().map_err(|e| refused("bind", e.to_string()))?;
        Ok(Running { circuit, ir, compiled })
    }

    /// The compiler of the environment, building into this project's cache directory when one
    /// is given.
    fn settings(&self) -> Result<Settings, String> {
        let cache = self.cache.as_ref().map(|dir| dir.to_string_lossy().into_owned());
        Settings::resolve(&|name| match (name, &cache) {
            ("PIPERINE_CACHE_DIR", Some(dir)) => Some(dir.clone()),
            _ => std::env::var(name).ok(),
        })
        .map_err(|e| e.to_string())
    }

    fn diagnosed(stage: &'static str, diagnostics: Vec<DesignDiagnostic>) -> Rejection {
        let message = format!("{} diagnostic(s)", diagnostics.len());
        Rejection { stage, diagnostics, message }
    }
}
