//! Post-elaboration typechecking over the finished [`Design`].
//!
//! **MD-45 (2026-08-12):** the PHDL-era checks this module ran (GAPS §B.1/
//! §B.2 connection-width and discipline-mismatch checks, pattern-match
//! exhaustiveness over PHDL `Stmt`/`Pattern`) were built directly against
//! `piperine-lang/src/parse/`'s AST types and are deleted with it, not
//! retargeted — none has a 1:1 VAMS shape. `.specs/features/
//! verilog-ams-support/tasks.md` T27 (Annex C analog-subset enforcement)
//! rebuilds this module's real checks against `piperine_vams::ast`.

use crate::pom::{Design, ElabError};

/// Typecheck every module in `design`, accumulating every independent
/// failure instead of stopping at the first (LSP-18/T16 precedent). Always
/// empty today — T27 is where the first real check (Annex C enforcement)
/// lands.
pub fn typecheck_program_accumulating(_design: &Design) -> Vec<ElabError> {
    Vec::new()
}
