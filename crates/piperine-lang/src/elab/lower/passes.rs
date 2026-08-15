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

use super::connect::InsertConnectModules;
use super::disciplines::ResolveDisciplines;
use super::flatten::FlattenHierarchy;
use super::generate::UnrollGenerate;
use super::Elaborator;

/// One elaboration stage. `run` mutates the shared `Elaborator` state and
/// the `Design` under construction; the first error aborts the pipeline.
///
/// `: std::any::Any` costs nothing on any impl (every pass here is a
/// `'static` unit struct, which is `Any` automatically) and lets
/// `tests::resolve_disciplines_runs_strictly_before_insert_connect_modules`
/// identify a `PASSES` entry's concrete type via `downcast_ref` (trait
/// upcasting, stable) instead of a fragile reflection trick.
pub(crate) trait ElabPass: std::any::Any {
    fn run(&self, elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError>;
}

/// The elaboration pipeline, in execution order.
///
/// `UnrollGenerate` (T25) runs after `ElabModules` — generate bodies
/// instantiate modules, so `lower_instance`'s target-port-order lookup
/// needs every module already registered — and before `FlattenHierarchy`,
/// which reads `Elaborator::generated_instances` when building the derived
/// flat form (design §6 ordering).
///
/// `InsertConnectModules` (T52) runs immediately after `ResolveDisciplines`
/// and before `UnrollGenerate`/`FlattenHierarchy`, per this task's own
/// stated position ("after `ResolveDisciplines`, before
/// `FlattenHierarchy`" — both hold with this placement). It writes only
/// `Elaborator::generated_instances` (the same derived-only field
/// `UnrollGenerate` writes), so ordering between the two doesn't matter
/// for correctness.
///
/// **Corrected (T37, LRMC-C7-01):** this comment used to claim the pass
/// "is a no-op over every design in today's corpus" — stale even before
/// T37, since T8 already fixed `ResolveDisciplines` to defer (not veto) a
/// connectrules-eligible mismatch via the shared `eligible_bridge` check.
/// A genuine cross-discipline crossing with an eligible `connectrules`
/// pair and no explicit connectmodule instance now reaches this pass,
/// gets bridged, and simulates end-to-end from real `.vams` source — see
/// root `tests/vams_connect_modules_e2e.rs`'s
/// `auto_inserted_connect_module_bridges_a_genuine_discipline_crossing_
/// end_to_end`.
pub(crate) const PASSES: &[&dyn ElabPass] = &[
    &Register,
    &LowerDeclarations,
    &ElabModules,
    &super::defparam::ApplyDefparams,
    &ResolveDisciplines,
    &InsertConnectModules,
    &UnrollGenerate,
    &Typecheck,
    &FlattenHierarchy,
];

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
        // LRMC-C1-09: every discipline's `access` name -> BranchKind,
        // built once from the disciplines/natures `LowerDeclarations`
        // already populated, so every module's contribution resolution
        // (elab_module -> resolve_contribution_branches) can promote a
        // custom access name, not only V/I/potential/flow.
        let access_kinds = super::vams::build_access_kind_map(design);
        let default_nettype = design.directives().default_nettype;
        let names: Vec<String> = elab.syms.module_decls.keys().cloned().collect();
        for name in &names {
            let decl = elab.syms.module_decls[name].clone();
            let module = elab.elab_module(&decl, &access_kinds, default_nettype)?;
            design.modules_map_mut().insert(name.clone(), module);
        }
        Ok(())
    }
}

/// Annex C analog-subset enforcement (T27, VAMS-25) — every failure found
/// across every analog behavior in the design is reported (LSP-18/T16:
/// like `ElabModules`, each behavior's check is independent of every
/// other's), while still returning the *first* one to satisfy the
/// ordinary `ElabPass::run` fail-fast contract.
struct Typecheck;
impl ElabPass for Typecheck {
    fn run(&self, _elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        crate::elab::typecheck::typecheck_program_accumulating(design)
            .into_iter()
            .next()
            .map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::connect::InsertConnectModules;
    use super::super::disciplines::ResolveDisciplines;

    /// T8 (LRMC-C7-01): `eligible_bridge` only makes sense if
    /// `ResolveDisciplines` still runs strictly before
    /// `InsertConnectModules` — this is the ordering invariant the fix
    /// depends on, asserted mechanically so a future reorder can't
    /// silently break it. Trait-object identity is read via
    /// `std::any::type_name_of_val` (stable through `&dyn Trait`, unlike
    /// `Any::downcast`, which every pass here — zero-sized unit structs —
    /// would need `'static` + explicit `as &dyn Any` plumbing for no
    /// benefit over just reading the concrete type name).
    #[test]
    fn resolve_disciplines_runs_strictly_before_insert_connect_modules() {
        let resolve_idx = PASSES
            .iter()
            .position(|p| (*p as &dyn std::any::Any).downcast_ref::<ResolveDisciplines>().is_some())
            .expect("ResolveDisciplines is in PASSES");
        let insert_idx = PASSES
            .iter()
            .position(|p| (*p as &dyn std::any::Any).downcast_ref::<InsertConnectModules>().is_some())
            .expect("InsertConnectModules is in PASSES");
        assert!(
            resolve_idx < insert_idx,
            "ResolveDisciplines (index {resolve_idx}) must run before InsertConnectModules \
             (index {insert_idx}) — eligible_bridge's deferral only works in this order"
        );
    }

    /// `PASSES`' exact length and shape — a coarse guard against a pass
    /// silently disappearing or a duplicate being added. Nine stages as
    /// of T28 (LRMC-C5-06): `ApplyDefparams` inserted after `ElabModules`.
    #[test]
    fn passes_has_the_documented_nine_stages() {
        assert_eq!(PASSES.len(), 9);
    }
}
