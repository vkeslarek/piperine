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
//! let design = piperine_lang::parse_and_elaborate(
//!     "discipline Electrical { potential v: Real; flow i: Real; }
//!      mod Resistor (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }"
//! )?;
//! # Ok::<(), String>(())
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
pub mod lowering;
pub mod parse;
pub mod pom;
pub mod resolve;
pub mod runtime;

// ── POM types ────────────────────────────────────────────────────────────
pub use pom::{
    ElabError,
    Behavior, BehaviorStmt, Connection, Design, Function, ImplBlock,
    Instance, MatchArm, Module, NetRef, NetType, Param, Port, TypeRef,
    ValueType, Wire,
    Id, Kind, Kinded, Named, NetTyped, OverrideMap, ReflectError, Selection, Value,
};
pub use parse::{parse_str, Lexed, Lexer, Tok};
pub use resolve::{ResolveError, Resolver};

// ── IR lowering + runtime ─────────────────────────────────────────────────
pub use lowering::ppr_to_ir;
pub use runtime::from_ir;
pub use runtime::device::PhdlDevice;
pub use runtime::digital_lower::ir_digital_to_interp;
pub use runtime::digital::{compile_digital_module, DigitalInterpreter, DigitalVal};

/// Parse a PHDL source string and run the full elaboration pipeline.
pub fn parse_and_elaborate(input: &str) -> Result<Design, String> {
    let source = parse_str(input).map_err(|e| e.to_string())?;
    source.elaborate().map_err(|e| e.to_string())
}