//! # piperine-lang
//!
//! The MSL frontend: lexer, parser, AST and printer live in [`msl`]. The
//! former Verilog-AMS frontend was retired. The object model of a parsed
//! source set lives in [`pom`]; elaboration is not built yet.

pub mod msl;
pub mod pom;
