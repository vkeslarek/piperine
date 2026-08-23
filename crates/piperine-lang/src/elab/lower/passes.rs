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

use crate::pom::{Design, ElabError, ElabErrorKind};

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
/// **`ApplyDefparams` runs after `UnrollGenerate` (T51, LRMW2-52, §6.9.4:
/// "defparam statements shall be evaluated after generate statement
/// instantiation").** It used to run right after `ElabModules`, before
/// generate — a `defparam` naming a generate-produced instance
/// (`defparam top.gen_loop__1.r = 5k;`) had no target to find yet.
/// `ApplyDefparams::apply_one` now also searches/mutates
/// `Elaborator::generated_instances` (MD-25-safe: still never touches
/// `Design::modules`), so moving the pass is sufficient — no change to
/// *what* it resolves against, only *when*.
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
    &ResolveDisciplines,
    &InsertConnectModules,
    &UnrollGenerate,
    &super::defparam::ApplyDefparams,
    &Typecheck,
    &CheckPortSizes,
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
            // 1364-2005: the `timescale in force for this module is the
            // last directive at or before its start — resolved here (the
            // pass owns the design's `DirectiveState`) and handed to
            // `elab_module`, which scales its gate `#delay` values by it.
            let timescale = design.directives().timescale_at(decl.span.start);
            let module = elab.elab_module(&decl, &access_kinds, default_nettype, timescale)?;
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

/// §6.5.7.1 (LRMW2-53, T48): "A scalar port can be connected to a scalar
/// net and a vector port can be connected to a vector net or concatenated
/// net expression of the matching width. In other words, the sizes of the
/// ports and net need to match." Runs after `Typecheck` — every module is
/// registered in `design.modules_map` by then, so a callee's declared
/// port widths are on hand for every instance in every module, not just
/// the one currently being lowered (`ElabModules`'s own per-module pass
/// can't see this — a module can be instantiated before its own
/// declaration is reached in an earlier pass).
///
/// Array-of-instances broadcast connections (`Instance::range`, e.g.
/// `res r[7:0] (bus, gnd);`) have their own, already-implemented width
/// rule (`elab/lower/flatten.rs`'s `inline_sub_instances`, which
/// legitimately allows a 1-wide net to broadcast across every array
/// element) — out of scope here, skipped by checking only `range.is_none()`
/// instances so the two rules never overlap or double-report.
///
/// A net whose width cannot be determined from the *calling* module's own
/// declarations (an implicit/undeclared interconnect signal, §6.5.7.2's
/// own explicit allowance, or a concatenated net expression — this task's
/// scope is the common bare-net-name case) is not checked — silence here
/// is deliberate, not silent-wrong: nothing is asserted about a width
/// this pass cannot see.
struct CheckPortSizes;
impl ElabPass for CheckPortSizes {
    fn run(&self, _elab: &mut Elaborator, design: &mut Design) -> Result<(), ElabError> {
        let names: Vec<String> = design.modules().map(|m| m.name().to_string()).collect();
        for name in &names {
            let caller = design.module(name).expect("just listed from design.modules()");
            for inst in caller.instances() {
                if inst.range.is_some() {
                    continue;
                }
                if design.udp(inst.module_name()).is_some() {
                    continue;
                }
                let Some(target) = design.module(inst.module_name()) else { continue };
                for (i, port_ref) in inst.ports().iter().enumerate() {
                    if port_ref.is_unconnected() {
                        continue;
                    }
                    let Some(target_port) = target.ports().get(i) else { continue };
                    let target_width = target_port.net_type().width();
                    let Some(net_width) = super::vams::net_ref_width(caller, port_ref) else { continue };
                    if net_width != target_width {
                        return Err(ElabError::from(ElabErrorKind::Other(format!(
                            "instance `{}` (module `{}`), port `{}`: connected net `{port_ref}` has \
                             width {net_width}, but the port is declared width {target_width} (LRM \
                             §6.5.7.1: \"the sizes of the ports and net need to match\")",
                            inst.name(),
                            inst.module_name(),
                            target_port.name(),
                        ))));
                    }
                }
            }
        }
        Ok(())
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
    /// silently disappearing or a duplicate being added. Ten stages as of
    /// T48 (LRMW2-53): `CheckPortSizes` inserted after `Typecheck`.
    #[test]
    fn passes_has_the_documented_ten_stages() {
        assert_eq!(PASSES.len(), 10);
    }
}
