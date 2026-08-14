//! # piperine-lang
//!
//! Parser and elaborator for Verilog-AMS (LRM 2.4) — produces the POM
//! (`Design`), the single resolved-enough object model. `piperine-codegen`
//! lowers a `Design` straight into devices; there is no separate IR crate.
//!
//! Every grammar production this crate implements is written from
//! `VAMS-LRM-2-4.md` Annex A (the normative formal syntax) and cited by
//! clause in the implementing method's doc comment. No third-party
//! Verilog-A/AMS compiler — its code, its grammar files, or its comments —
//! is consulted as a source. See `.specs/features/verilog-ams-support/design.md`
//! §0 (F3) for why.
//!
//! **MD-44/MD-45 (2026-08-12):** PHDL is retired. The PHDL-syntax-only
//! value-layer surface it used to carry (generics, the capability system,
//! `bundle`/`enum`, `constraint` blocks, pattern matching, tuples, lambdas,
//! `Map`/`Set` literals) is deleted, not retargeted. VAMS (LRM Annex A) is
//! the sole frontend; the POM adapts to VAMS's shape rather than the other
//! way around.
//!
//! **MD-47 (the old `piperine-lang` and `piperine-vams` crates merged into
//! this one, keeping the `piperine-lang` name):** the parser
//! (`ast`/`grammar`/`lexer`/`preprocessor`/`parser`) and the elaborator
//! (`elab`/`pom`/`eval`) live in one crate — the purity-gate boundary
//! between "text -> AST" and "AST -> POM" (MD-43) was a real cost (an
//! external-crate seam between two layers that only ever ship together)
//! for a guarantee nothing downstream relied on independently. Diagnostic
//! rendering (`miette`, source spans tied to file identity) now happens
//! in the same crate as the plain byte `Span`s it renders.
//!
//! ## Pipeline
//!
//! ```text
//! &str
//!  │
//!  ▼  crate::Parser
//! crate::ast::SourceFile   (LRM Annex A tree — types are strings)
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
//! | [`ast`] | LRM Annex A parse tree |
//! | [`grammar`], [`lexer`], [`preprocessor`], [`parser`] | text -> AST |
//! | [`elab`] | Elaborator, event registry, const evaluator |
//! | [`pom`] | Piperine Object Model — Design, Module, Port, Value, OverrideMap, ... |

pub mod ast;
pub mod directives;
pub mod error;
pub mod fmt;
pub mod grammar;
pub mod keywords;
pub mod lexer;
pub mod parser;
pub mod preprocessor;

pub mod elab;
pub mod eval;
pub mod pom;
pub mod annex_e;
pub mod value;
pub mod source_map;
pub mod math;
mod vams_include;

pub use error::{ParseError, ParseErrorKind, Span, UnderscorePosition};
pub use fmt::Formatter;
pub use keywords::Keywords;
pub use lexer::{BasedNumber, Lexed, Lexer, NumberBase, NumberLit, Tok};
pub use preprocessor::{IncludeResolver, NoIncludes, Preprocessor};
pub use ast::{Attr, Item, SourceFile};
pub use grammar::Grammar;
pub use parser::Parser;

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
    parse_and_elaborate_seeded_with_context(input, source_map, seed).map(|(design, _ctx)| design)
}

/// Like [`parse_and_elaborate_seeded`], but also returns the populated
/// [`elab::registry::ElabContext`] — needed by consumers that resolve
/// names against `ElabContext::schemas` *after* elaboration succeeds
/// (T35, VAMS-12/13; `piperine-lang-server`'s `symbol_index::resolve_at`
/// is the first such consumer, T37).
pub fn parse_and_elaborate_seeded_with_context(
    input: &str,
    source_map: &SourceMap,
    seed: impl FnOnce(&mut elab::registry::ElabContext),
) -> Result<(Design, elab::registry::ElabContext), miette::Report> {
    let parser = crate::Parser::with_include_resolver(SourceMapIncludeResolver::new(source_map));
    let source = parser
        .parse(input)
        .map_err(|e| miette::Report::from(ElabError::from(e)).with_source_code(input.to_string()))?;
    let mut elaborator = elab::Elaborator::new();
    seed(&mut elaborator.ctx);
    let design = elaborator
        .elaborate(source)
        .map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))?;
    Ok((design, elaborator.ctx))
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
