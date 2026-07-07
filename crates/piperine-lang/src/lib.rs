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
    Behavior, BehaviorStmt, BenchBlock, Connection, Design, Function, ImplBlock,
    Instance, MatchArm, Module, NetRef, NetType, Param, Port, TypeRef,
    ValueType, Wire,
    Id, Kind, Kinded, Named, NetTyped, OverrideMap, ReflectError, Selection, Value,
};
pub use parse::{parse_str, Lexed, Lexer, Tok};
pub use resolve::{ResolveError, Resolver};
pub use source_map::SourceMap;

/// Parse a PHDL source string and run the full elaboration pipeline.
pub fn parse_and_elaborate(input: &str, source_map: &SourceMap) -> Result<Design, miette::Report> {
    let source = parse_str(input).map_err(|e| miette::miette!("{}", e).with_source_code(input.to_string()))?;
    source.elaborate(source_map).map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))
}