//! The elaboration pass list.
//!
//! [`Elaborator::elaborate`] runs these in order over shared state — the
//! `Elaborator` itself (symbol tables collected by `Register`) and the
//! `Design` under construction. Each pass is one struct with one job; the
//! pipeline *order* is the [`PASSES`] array, not something buried in a
//! driver.
//!
//! **MD-45 (2026-08-12):** the PHDL-era pipeline (`ValidateEvents`,
//! `FoldGlobals`, `ElabFns`, `AttachBehaviors`, `ElabConstraints`,
//! `ResolveCalls`, `Typecheck`) is deleted with `piperine-lang/src/parse/`
//! — each existed to serve PHDL-syntax-only surface (the event registry's
//! `<+`/`<-` misuse checks, global-constant folding over `enum`/`const`,
//! `impl`/`fn` elaboration, dv-core constraint attachment, `extern`-call
//! resolution, GAPS width/discipline typechecking). `Register` is VAMS-
//! shaped from the start (`register.rs`); `FlattenHierarchy` operates on
//! the POM, not the surface AST, and needed no change. `.specs/features/
//! verilog-ams-support/tasks.md` T22 (module lowering) through T27 (Annex C
//! enforcement) rebuild this pipeline's remaining VAMS-shaped stages task
//! by task.

use crate::pom::{Design, ElabError};

use super::flatten::FlattenHierarchy;
use super::Elaborator;

/// One elaboration stage. `run` mutates the shared `Elaborator` state and
/// the `Design` under construction; the first error aborts the pipeline.
pub(crate) trait ElabPass {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError>;
}

/// The elaboration pipeline, in execution order.
pub(crate) const PASSES: &[&dyn ElabPass] = &[&Register, &FlattenHierarchy];

/// Index every top-level item into the elaborator's symbol tables.
struct Register;
impl ElabPass for Register {
    fn run(&self, elab: &mut Elaborator, _design: &mut Design) -> Result<(), ElabError> {
        let items = std::mem::take(&mut elab.pending_items);
        elab.register_items(items.iter())
    }
}
