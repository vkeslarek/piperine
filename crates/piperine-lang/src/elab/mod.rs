//! # Elaboration phase
//!
//! Transforms a parsed [`crate::ast::SourceFile`] into a fully
//! resolved [`Design`].
//!
//! ```text
//! SourceFile (VAMS AST)  ──Elaborator──▶  Design (elaborated IR + POM)
//! ```
//!
//! **MD-44/MD-45 (2026-08-12):** this phase is being rebuilt against the
//! VAMS AST task by task (`.specs/features/verilog-ams-support/tasks.md`
//! T21-T27). `piperine-lang/src/parse/` (PHDL) and the PHDL-syntax-only
//! value-layer machinery it carried (generics, capabilities, bundles,
//! enums, `constraint` blocks, `use`-decl resolution/prelude injection) are
//! gone, not retargeted — see MD-45 in `.specs/STATE.md`.

pub mod const_eval;
pub mod lower;
pub mod typecheck;
pub mod registry;
pub mod resolution;

use crate::pom::{Design, ElabError};
pub use lower::Elaborator;

use crate::ast::SourceFile;

impl Elaborator {
    /// Elaborate a parsed VAMS `SourceFile` directly — VAMS has no
    /// PHDL-style `use` declaration to expand or prelude to inject before
    /// elaboration proper (`` `include`` is a preprocessor-level, textual
    /// concern `crate::Parser` already resolved before this AST
    /// existed).
    pub fn elaborate_source(source: SourceFile) -> Result<Design, ElabError> {
        let mut elaborator = Elaborator::new();
        elaborator.elaborate(source)
    }

    /// Like [`elaborate_source`](Self::elaborate_source), but also returns a
    /// [`ResolutionIndex`](resolution::ResolutionIndex) built over the
    /// resulting `Design` (LSP-03/05).
    pub fn elaborate_source_with_index(
        source: SourceFile,
    ) -> Result<(Design, resolution::ResolutionIndex), ElabError> {
        let design = Self::elaborate_source(source)?;
        let index = resolution::index_design(&design);
        Ok((design, index))
    }
}
