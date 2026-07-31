//! # piperine-lang
//!
//! Parser and elaborator for PHDL — produces the POM (`Design`), the single
//! resolved-enough object model. `piperine-codegen` lowers a `Design`
//! straight into devices; there is no separate IR crate.
//!
//! ## Pipeline
//!
//! ```text
//! &str
//!  │
//!  ▼  parse::Lexer
//! Vec<Lexed>              (token sequence with byte-range spans)
//!  │
//!  ▼  parse::Parser
//! parse::SourceFile       (unresolved AST — types are strings)
//!  │
//!  ▼  SourceFile::elaborate
//! Design                  (elaborated design + POM root)
//!  │
//!  ▼  piperine_codegen::CircuitCompiler::from_design
//! CircuitInstance         (ready for piperine-solver)
//! ```
//!
//! ## Quick start
//!
//! ```rust
//! let mut sm = piperine_lang::SourceMap::new(std::path::PathBuf::from("."));
//! let design = piperine_lang::parse_and_elaborate(
//!     "discipline Electrical { potential v: Real; flow i: Real; }
//!      mod Resistor (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }",
//!      &mut sm
//! ).unwrap();
//! ```
//!
//! ## Module organisation
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`parse`] | Lexer, parser, and parse-AST types |
//! | [`elab`] | Elaborator, event registry, const evaluator |
//! | [`pom`] | Piperine Object Model — Design, Module, Port, Value, OverrideMap, ... |
//! | [`resolve`] | `use` declaration resolver |

pub mod elab;
pub mod eval;
pub mod parse;
pub mod pom;
pub mod resolve;
pub mod value;
pub mod source_map;
pub mod math;
// ── POM types ────────────────────────────────────────────────────────────
pub use pom::{
    ElabError,
    Behavior, BehaviorStmt, Connection, Design, Function, ImplBlock,
    Instance, MatchArm, Module, NetRef, NetType, Param, Port, Project, RfPort, TypeRef,
    ValueType, Wire,
    Id, Kind, Kinded, Named, NetTyped, OverrideMap, ReflectError, Selection, Value,
};
pub use parse::{parse_str, Lexed, Lexer, Tok};
pub use resolve::{ResolveError, Resolver};
pub use source_map::SourceMap;
pub use elab::resolution::{BindingId, BindingInfo, BindingKind, ResolutionIndex};

/// Parse a PHDL source string and run the full elaboration pipeline.
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
    let source = parse_str(input).map_err(|e| miette::miette!("{}", e).with_source_code(input.to_string()))?;
    let mut resolver = Resolver::new(source_map);
    source
        .elaborate_seeded(&mut resolver, seed)
        .map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))
}

/// Register the `@device`/`@port` schemas (`headers/device_port.phdl`) into
/// an elaboration context — the plugin system's own attribute schemas
/// (declared-language-surface DLS-21), needed by any `@device`-declared
/// module regardless of which binding mode (plugin or host, dv-tester T17)
/// resolves it at circuit-build time. `piperine_plugin::PluginHost::seed_schemas`
/// calls the equivalent registration gated on a loaded plugin; this is the
/// same declarations, callable directly by a caller (`piperine-api`'s host
/// binding path) that has no plugin host at all.
pub fn seed_device_port_schemas(ctx: &mut elab::registry::ElabContext) {
    if let Ok(source) = parse::parse_str(include_str!("../headers/device_port.phdl")) {
        for item in &source.items {
            if let parse::ast::Item::ExternDecl(parse::ast::ExternDecl::Attribute {
                span,
                name,
                fields,
                doc,
            }) = item
            {
                let attr_fields = fields
                    .iter()
                    .map(|f| elab::registry::AttrField {
                        name: f.name.clone(),
                        ty: f.ty.name.clone(),
                        required: !f.ty.optional,
                        default: None,
                        decl_span: f.span,
                    })
                    .collect();
                ctx.schemas.register_declared(name, attr_fields, *span, doc.clone());
            }
        }
    }
}

/// Like [`parse_and_elaborate`], but also returns a [`ResolutionIndex`]
/// built over the resulting `Design` (LSP-03/05) — see
/// [`parse::SourceFile::elaborate_with_index`].
pub fn parse_and_elaborate_with_index(
    input: &str,
    source_map: &SourceMap,
) -> Result<(Design, ResolutionIndex), miette::Report> {
    let source = parse_str(input).map_err(|e| miette::miette!("{}", e).with_source_code(input.to_string()))?;
    source
        .elaborate_with_index(source_map)
        .map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))
}