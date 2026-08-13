//! Elaboration-context registries.
//!
//! **MD-45 (2026-08-12):** every sub-registry this module held before the
//! VAMS frontend swap (`TypeRegistry`, `CallableRegistry`, `SchemaRegistry`,
//! `OperatorRegistry`, `ImplMethodTable`, `ConstraintHelperRegistry`,
//! `ComponentRegistry`) existed to back PHDL's `extern`/capability/
//! generics/bundle/constraint-block syntax (MD-24's declared-language-
//! surface mechanism, dv-core's constraint helpers, generic module
//! monomorphization) — none of which has a Verilog-AMS Annex A production.
//! They are deleted with `piperine-lang/src/parse/`, not retargeted.
//! `ElabContext` is left as the seam future passes register into (T22+
//! rebuilds a module/instance registry as VAMS module lowering needs one;
//! a later phase re-anchors `@device`/`@port` schemas on VAMS
//! `(* attribute *)` syntax, VAMS-12/13, tasks.md open item #5).

pub struct ElabContext {}

impl Default for ElabContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ElabContext {
    pub fn new() -> Self {
        Self {}
    }
}
