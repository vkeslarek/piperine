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
//!
//! **T35 (VAMS-12/13)**: `@device`/`@port` re-anchored on VAMS
//! `(* attribute *)` syntax. `ElabContext` regains one narrow seam —
//! [`ElabContext::schemas`] — the set of attribute-schema names the
//! current elaboration run recognizes. It is not MD-24's textual
//! `extern attribute` mechanism reborn (there is no field/type table,
//! only a name), because that mechanism's declaration vehicle (the
//! `extern` keyword) has no VAMS Annex A production and was deleted with
//! it (T21). It exists solely so `piperine_lang::seed_device_port_schemas`/
//! `PluginHost::seed_schemas` (the plugin system's own schema authority,
//! MD-21/D2) can register `device`/`port` before elaboration runs, and so
//! `elab::lower::vams`'s attribute-instance lowering can fail loud
//! (`ElabErrorKind::UnknownAttrSchema`) on anything else — MD-24's "no
//! silent fallback" rule ported verbatim, not reinvented.

#[derive(Default)]
pub struct ElabContext {
    /// Attribute-instance schema names registered for this elaboration
    /// run (VAMS-12/13). Empty unless a caller seeds it (via
    /// `parse_and_elaborate_seeded`) before elaboration starts.
    pub schemas: std::collections::HashSet<String>,
}

impl ElabContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one attribute-schema name as known for this run.
    pub fn register_schema(&mut self, name: impl Into<String>) {
        self.schemas.insert(name.into());
    }
}
