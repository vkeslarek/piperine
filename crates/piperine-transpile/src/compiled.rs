//! A design built and loaded (design "Transpiler and Compiled"): the library the solver binds
//! its modules from, the source kept beside it, and what the build cost.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use piperine_abi::raw::Module;
use piperine_ir::{BodyId, Ir};
use piperine_plan::{BodyPlan, DerivePlan, Modules, Netlist};
use piperine_solver::prelude::CircuitInstance;

use crate::cache::Entry;
use crate::error::Error;
use crate::load::Loaded;
use crate::shape_check::ShapeCheck;

/// What one `Transpiler::compile` cost (RS-08).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Printing the IR as Rust.
    pub transpile: Duration,
    /// Running `rustc` (the ABI `rlib` when it was missing, and the design's `cdylib`); zero on
    /// a cache hit.
    pub compile: Duration,
    /// Checking and opening the library.
    pub load: Duration,
    /// Builds `rustc` ran for this compile: 0 on a cache hit.
    pub rustc_invocations: u32,
    /// Whether the entry was already in the cache and loaded as it was.
    pub cache_hit: bool,
    /// The compiler's standard error for the design's build: its warnings. Empty on a cache
    /// hit, where nothing was built.
    pub warnings: String,
}

/// A design compiled, cached and loaded.
#[derive(Debug)]
pub struct Compiled {
    pub(crate) ir: Arc<Ir>,
    pub(crate) entry: Entry,
    pub(crate) loaded: Loaded,
    pub(crate) report: Report,
    /// One module per body, by body index; `None` for a body with nothing to run.
    modules: Vec<Option<&'static Module>>,
}

impl Compiled {
    /// Binds each running body of `ir` (one with a plan) to the module of the library that
    /// carries its name, after checking that module's prototype against the plan's shape entry
    /// by entry: a mismatch is a transpiler defect, named here instead of simulated wrongly.
    pub(crate) fn bind(
        ir: Arc<Ir>,
        plans: &[Option<BodyPlan>],
        entry: Entry,
        loaded: Loaded,
        report: Report,
    ) -> Result<Compiled, Error> {
        let mut modules = Vec::new();
        for body in ir.bodies() {
            let Some(plan) = plans.get(body.id().index()).and_then(Option::as_ref) else {
                modules.push(None);
                continue;
            };
            let mismatch = |entry: String| Error::ShapeMismatch { body: body.name().to_string(), entry };
            let module = loaded
                .module(body.name())
                .ok_or_else(|| mismatch(format!("a module named `{}` (the library has {:?})", body.name(), loaded.names())))?;
            if let Some(difference) = ShapeCheck::first_difference(plan.shape().shape(), module.prototype()) {
                return Err(mismatch(difference));
            }
            modules.push(Some(module));
        }
        if !DerivePlan::of(&ir).is_empty() && !loaded.derives() {
            return Err(Error::Load {
                file: loaded.file().to_path_buf(),
                cause: format!("has no symbol `{}` and the design derives params from others", crate::print::derive::SYMBOL),
            });
        }
        Ok(Compiled { ir, entry, loaded, report, modules })
    }

    /// The design as a solver circuit, each instance bound to its body's compiled module
    /// (`Netlist::circuit`, the loop the interpreter's circuit runs).
    pub fn circuit(&self) -> piperine_solver::prelude::Result<CircuitInstance> {
        Netlist::new(&self.ir).circuit(self)
    }

    /// Derived param `k` of the design's `DerivePlan` (a parent before its children) over the
    /// params of its parent, in declaration order (a `bool` as 0.0 or 1.0): the value the
    /// placement's expression gives. NaN for a `k` the design has no binding for, or a number of
    /// params other than its parent's, and for a design that derives nothing.
    pub fn derive(&self, k: u32, params: &[f64]) -> f64 {
        self.loaded.derive(k, params).unwrap_or(f64::NAN)
    }

    /// What this build cost.
    pub fn report(&self) -> &Report {
        &self.report
    }

    /// The generated Rust (`design.rs`), kept on disk beside the library (RS-02, R3): read it
    /// to see what runs.
    pub fn source_path(&self) -> PathBuf {
        self.entry.source()
    }

    /// The cache entry: its source, runtime source, library and manifest.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// The loaded library.
    pub fn loaded(&self) -> &Loaded {
        &self.loaded
    }

    /// The design this was built from.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }
}

impl Modules for Compiled {
    fn module(&self, body: BodyId) -> Option<&'static Module> {
        self.modules.get(body.index()).copied().flatten()
    }
}
