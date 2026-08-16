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
//! in the same crate as the plain byte `PreprocessedSpan`s it renders.
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
pub mod source_provenance;

pub mod elab;
pub mod eval;
pub mod pom;
pub mod annex_e;
pub mod value;
pub mod source_map;
pub mod math;
mod vams_include;

pub use error::{ParseError, ParseErrorKind, PreprocessedSpan, UnderscorePosition};
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
    let source = parser.parse(input).map_err(|e| {
        // `Parser::parse`'s own `Result<SourceFile, ParseError>` signature
        // (consumed by piperine-lang-server and this crate's own test
        // suite, beyond this task's lib.rs-only scope) doesn't carry a
        // `SourceProvenance` out on its error path. A genuine
        // preprocessor-stage failure (e.g. `IncludeNotFound`) therefore has
        // none to resolve through — spec's own Edge Cases section states
        // that exact path is unaffected by this feature, so it keeps
        // today's raw-input rendering (the `Err` arm below). A lexer/
        // grammar failure only happens *after* preprocessing itself
        // already succeeded, which is the case this feature exists to fix
        // (SPANMAP-01/02) — a second, cheap preprocessor pass recovers the
        // provenance `Parser::parse` had internally but couldn't surface.
        let mut pp = crate::preprocessor::Preprocessor::with_include_resolver(SourceMapIncludeResolver::new(source_map));
        match pp.run(input) {
            Ok((_, prov)) => resolve_elab_error(ElabError::from(e), &prov),
            Err(_) => miette::Report::from(ElabError::from(e)).with_source_code(input.to_string()),
        }
    })?;
    let mut elaborator = elab::Elaborator::new();
    seed(&mut elaborator.ctx);
    // Captured before `elaborate` consumes `source` by value — the same
    // provenance a successful `Design` would carry (T7).
    let prov = source.source_provenance.clone();
    let design = elaborator.elaborate(source).map_err(|e| resolve_elab_error(e, &prov))?;
    Ok((design, elaborator.ctx))
}

/// Resolves `err`'s flat-preprocessed-buffer span (if any) to its real
/// originating file and local byte range via `prov`, rewriting the error's
/// own span field(s) in place — `miette::SourceSpan { offset, len }` is
/// numerically identical to the `PreprocessedSpan` it was built from, so
/// this reconstructs and re-resolves it — then attaches a `NamedSource`
/// built from that file's own text. This is the render-boundary fix this
/// feature exists for (SPANMAP-01/02/03/07): the CLI/host no longer
/// blindly assumes every span indexes into the top-level `input` string.
fn resolve_elab_error(mut err: ElabError, prov: &source_provenance::SourceProvenance) -> miette::Report {
    // Exactly one of these is ever populated by construction: `with_span`
    // only sets the outer field when it was `None`, and `From<ParseError>`
    // deliberately leaves the outer field `None`, relying on the wrapped
    // `VamsParse` variant's own span instead.
    let effective = err.span.or_else(|| match &err.kind {
        pom::ElabErrorKind::VamsParse { span, .. } => *span,
        _ => None,
    });
    let Some(sp) = effective else {
        // No span recorded at all — nothing to resolve; render exactly as
        // today (an un-spanned diagnostic never had a location to get
        // wrong).
        return miette::Report::from(err);
    };
    let flat = error::PreprocessedSpan::new(sp.offset(), sp.offset() + sp.len());
    let resolved = prov.resolve(flat);
    let (named, local_span) = resolved.to_named_source_span(prov);
    if err.span.is_some() {
        err.span = Some(local_span);
    } else if let pom::ElabErrorKind::VamsParse { span, .. } = &mut err.kind {
        *span = Some(local_span);
    }
    miette::Report::from(err).with_source_code(named)
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
