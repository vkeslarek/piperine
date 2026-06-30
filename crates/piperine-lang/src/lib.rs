//! # piperine-lang
//!
//! Parser and elaborator for the Piperine Hardware Definition Language (PHDL).
//!
//! ## Pipeline
//!
//! ```text
//! &str
//!  │
//!  ▼  parse::Lexer
//! Vec<Lexed>          (token sequence with byte-range spans)
//!  │
//!  ▼  parse::Parser
//! parse::SourceFile   (unresolved AST — types are strings, generics are present)
//!  │
//!  ▼  elab::Elaborator
//! elab::ElabProgram   (resolved IR — no generics, bundles expanded, for/if eliminated)
//! ```
//!
//! ## Quick start
//!
//! ```rust
//! // Just parse:
//! let ast = piperine_lang::parse::parse_str("mod R (inout p: Electrical);")?;
//!
//! // Parse + elaborate:
//! let program = piperine_lang::parse_and_elaborate(
//!     "discipline Electrical { potential v: Real; flow i: Real; }\
//!      mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }"
//! )?;
//! # Ok::<(), String>(())
//! ```
//!
//! ## Module organisation
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`parse`] | Lexer, parser, and parse-AST types |
//! | [`resolve`] | `use` declaration resolver: built-in + file-based module loading |
//! | [`elab`] | Elaborator, elaborated-IR types, event registry, const evaluator |
//! | [`stdlib`] | Embedded `.phdl` sources for the standard library |
//!
//! For the IR → Device pipeline (analog and digital blocks → solver
//! `CircuitInstance`), see the `piperine-codegen` crate.

pub mod elab;
pub mod parse;
pub mod pom;
pub mod resolve;
pub mod stdlib;

// Re-export POM types (the reflection API surface).
pub use elab::{
    elaborate, elaborate_with,
    Behavior, BehaviorStmt, Connection, Design, ElabError, Function, ImplBlock,
    Instance, MatchArm, Module, NetRef, NetType, Param, Port, TypeRef,
    ValueType, Wire,
};
pub use pom::{Id, Kind, OverrideMap, ReflectError, Selection, Value};
pub use parse::{parse_str, Lexed, Lexer, Tok};
pub use resolve::{ResolveError, Resolver};

/// Parse a PHDL source string and run the full elaboration pipeline.
///
/// Equivalent to calling [`parse::parse_str`] and then [`elab::elaborate`].
pub fn parse_and_elaborate(input: &str) -> Result<Design, String> {
    let source = parse_str(input)?;
    elaborate(source).map_err(|e| e.to_string())
}
