//! # piperine-lang
//!
//! The MSL language front end, in two layers.
//!
//! * [`msl`]: lexer, parser, syntax tree and printer. Source text in, a
//!   recovered [`msl::ast::File`] and diagnostics out. The grammar is the specification's
//!   Appendix B.
//! * [`pom`]: the Piperine object model. One call turns source files into an immutable
//!   [`pom::Design`], a lossless model of what was written with every name resolved
//!   and classified, read through Java-reflection-style handles (`design.module("Front")`,
//!   `module.ports()`, `signal.writers()`). The model mirrors the source: a signal declared in
//!   module `abc` is found there and nowhere else, and resolution only adds links.
//!
//! Elaboration (generics, unrolling, flattening) is not part of this crate yet; it would be
//! built *from* a `Design`, never into it.
//!
//! The reference documentation of the object model is under `docs/pom/` and its examples run
//! as doctests of [`pom::guide`].
//!
//! ```
//! use piperine_lang::pom::{Design, Reflect};
//!
//! let design = Design::builder()
//!     .source("top.msl", "nature Bit { value: bool }\nmodule Top(a: Bit) { signal s: Bit; s = a; }")
//!     .build();
//! let top = design.module("Top").unwrap();
//! assert_eq!(top.signal("s").unwrap().declared_in().unwrap().name(), Some("Top"));
//! assert_eq!(top.signal("s").unwrap().writers().len(), 1);
//! ```

pub mod msl;
pub mod pom;
