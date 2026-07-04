//! # piperine-lang
//!
//! Parser, elaborator, and IR lowering for PHDL.
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
//!  ▼  lowering::ppr_to_ir
//! IrProgram               (the central IR for piperine-codegen)
//!  │
//!  ▼  runtime::from_ir
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
//! | [`lowering`] | Design → IrProgram (`ppr_to_ir`) |
//! | [`runtime`] | IrProgram → Device/CircuitInstance (`from_ir`, `PhdlDevice`, `DigitalInterpreter`) |
//! | [`resolve`] | `use` declaration resolver |

pub mod elab;
pub mod eval;
pub mod lowering;
pub mod parse;
pub mod pom;
pub mod resolve;
pub mod value;
pub mod source_map;

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

// ── IR lowering + runtime ─────────────────────────────────────────────────
pub use lowering::{ppr_to_ir, LowerError, LowerErrors};

/// Parse a PHDL source string and run the full elaboration pipeline.
pub fn parse_and_elaborate(input: &str, source_map: &SourceMap) -> Result<Design, miette::Report> {
    let source = parse_str(input).map_err(|e| miette::miette!("{}", e).with_source_code(input.to_string()))?;
    source.elaborate(source_map).map_err(|e| miette::Report::from(e).with_source_code(input.to_string()))
}