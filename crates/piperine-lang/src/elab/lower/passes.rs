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
//! verilog-ams-support/tasks.md` T22 adds `ElabModules` (module/port/param/
//! net/instance/behavior lowering, `vams.rs`); T23 through T27 rebuild the
//! remaining VAMS-shaped stages (natures/disciplines/branches, generate,
//! Annex F, Annex C) task by task.

use crate::pom::{Design, ElabError};

use super::flatten::FlattenHierarchy;
use super::generate::UnrollGenerate;
use super::Elaborator;

/// One elaboration stage. `run` mutates the shared `Elaborator` state and
/// the `Design` under construction; the first error aborts the pipeline.
pub(crate) trait ElabPass {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError>;
}

/// The elaboration pipeline, in execution order.
///
/// `UnrollGenerate` (T25) runs after `ElabModules` — generate bodies
/// instantiate modules, so `lower_instance`'s target-port-order lookup
/// needs every module already registered — and before `FlattenHierarchy`,
/// which reads `Elaborator::generated_instances` when building the derived
/// flat form (design §6 ordering).
pub(crate) const PASSES: &[&dyn ElabPass] =
    &[&Register, &LowerDeclarations, &ElabModules, &UnrollGenerate, &FlattenHierarchy];

/// Index every top-level item into the elaborator's symbol tables.
struct Register;
impl ElabPass for Register {
    fn run(&self, elab: &mut Elaborator, _design: &mut Design) -> Result<(), ElabError> {
        let items = std::mem::take(&mut elab.pending_items);
        elab.register_items(items.iter())
    }
}

/// Lower top-level `nature`/`discipline` declarations into the design
/// (T23, VAMS-01) — must precede `ElabModules` since a module's port/net
/// disciplines don't need this, but branch/contribution lowering inside
/// behaviors reads nothing from it either; ordered first anyway so a
/// future pass reading `Design::natures`/`disciplines` during module
/// lowering (e.g. Annex F resolution, T26) can rely on it being populated.
struct LowerDeclarations;
impl ElabPass for LowerDeclarations {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        elab.lower_declarations(design)
    }
}

/// Lower every registered `module`/`macromodule`/`connectmodule` into the
/// design (T22, VAMS-04).
struct ElabModules;
impl ElabPass for ElabModules {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let names: Vec<String> = elab.syms.module_decls.keys().cloned().collect();
        for name in &names {
            let decl = elab.syms.module_decls[name].clone();
            let module = elab.elab_module(&decl)?;
            design.modules_map_mut().insert(name.clone(), module);
        }
        Ok(())
    }
}
