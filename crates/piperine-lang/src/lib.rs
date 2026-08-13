//! # piperine-lang
//!
//! Parser and elaborator for Verilog-AMS (LRM 2.4) — produces the POM
//! (`Design`), the single resolved-enough object model. `piperine-codegen`
//! lowers a `Design` straight into devices; there is no separate IR crate.
//!
//! **MD-44/MD-45 (2026-08-12):** PHDL is retired. `piperine-lang/src/parse/`
//! (the PHDL lexer/parser/AST/formatter, plus the PHDL-syntax-only
//! value-layer surface it carried — generics, the capability system,
//! `bundle`/`enum`, `constraint` blocks, pattern matching, tuples, lambdas,
//! `Map`/`Set` literals) is deleted, not retargeted. `piperine-vams`
//! (LRM Annex A) is the sole frontend; the POM adapts to VAMS's shape
//! rather than the other way around. The deleted surface's replacement —
//! Piperine's own extensions re-expressed through VAMS `(* attribute *)`
//! syntax — is future work (`.specs/features/verilog-ams-support/tasks.md`
//! open item #5), not part of this crate today.
//!
//! ## Pipeline
//!
//! ```text
//! &str
//!  │
//!  ▼  piperine_vams::Parser
//! piperine_vams::ast::SourceFile   (LRM Annex A tree — types are strings)
//!  │
//!  ▼  SourceFile::elaborate
//! Design                  (elaborated design + POM root)
//!  │
//!  ▼  piperine_codegen::CircuitCompiler::from_design
//! CircuitInstance         (ready for piperine-solver)
//! ```
//!
//! ## Module organisation
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`elab`] | Elaborator, event registry, const evaluator |
//! | [`pom`] | Piperine Object Model — Design, Module, Port, Value, OverrideMap, ... |

pub mod elab;
pub mod eval;
pub mod pom;
pub mod value;
pub mod source_map;
pub mod math;
mod vams_include;
// ── POM types ────────────────────────────────────────────────────────────
pub use pom::{
    ElabError,
    Behavior, BehaviorStmt, Connection, Design, Function,
    Instance, Module, NetRef, NetType, Param, Port, Project, RfPort, TypeRef,
    ValueType, Wire,
    Id, Kind, Kinded, Named, NetTyped, OverrideMap, ReflectError, Selection, Value,
};
pub use source_map::SourceMap;
pub use elab::resolution::{BindingId, BindingInfo, BindingKind, ResolutionIndex};
pub use vams_include::SourceMapIncludeResolver;

/// Parse a Verilog-AMS source string and run the full elaboration pipeline.
pub fn parse_and_elaborate(input: &str, source_map: &SourceMap) -> Result<Design, miette::Report> {
    parse_and_elaborate_seeded(input, source_map, |_| {})
}

/// Like [`parse_and_elaborate`], but seeds the elaboration registries first —
/// the hook plugin hosts use to contribute attribute schemas before the
/// design elaborates (SPEC Part VI §11).
pub fn parse_and_elaborate_seeded(
    input: &str,
    source_map: &SourceMap,
    seed: impl FnOnce(&mut elab::registry::ElabContext),
) -> Result<Design, miette::Report> {
    let parser = piperine_vams::Parser::with_include_resolver(SourceMapIncludeResolver::new(source_map));
    let source = parser
        .parse(input)
        .map_err(|e| miette::Report::from(ElabError::from(e)).with_source_code(input.to_string()))?;
    let mut elaborator = elab::Elaborator::new();
    seed(&mut elaborator.ctx);
    elaborator
        .elaborate(source)
        .map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))
}

/// Register the `@device`/`@port` schemas into an elaboration context — the
/// plugin system's own attribute schemas (declared-language-surface
/// DLS-21), needed by any `@device`-declared module regardless of which
/// binding mode (plugin or host, dv-tester T17) resolves it at
/// circuit-build time.
///
/// **T35 (VAMS-12/13)**: re-anchored on VAMS `(* attribute *)` syntax.
/// MD-24's textual `extern attribute` declaration this used to read from
/// (`headers/device_port.phdl`) is gone with the PHDL parser (T21) — there
/// is no `extern` keyword in VAMS Annex A to carry it. The schema names
/// are registered natively here instead (`ctx.register_schema`); their
/// field shapes (`device { plugin, type }`, `port { name, kind? }`) are
/// validated where `elab::lower::vams` lowers a `(* ... *)` instance
/// naming one of them, not read from a textual table anymore. MD-24's
/// "no silent fallback" still holds: an attribute-instance name that is
/// neither of these two fails loud (`ElabErrorKind::UnknownAttrSchema`).
pub fn seed_device_port_schemas(ctx: &mut elab::registry::ElabContext) {
    ctx.register_schema("device");
    ctx.register_schema("port");
}

/// Like [`parse_and_elaborate`], but also returns a [`ResolutionIndex`]
/// built over the resulting `Design` (LSP-03/05).
pub fn parse_and_elaborate_with_index(
    input: &str,
    source_map: &SourceMap,
) -> Result<(Design, ResolutionIndex), miette::Report> {
    let design = parse_and_elaborate(input, source_map)?;
    let index = crate::elab::resolution::index_design(&design);
    Ok((design, index))
}