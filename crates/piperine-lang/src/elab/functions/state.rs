//! The state of function lowering (§3.4): [`FunctionTables`], the IR functions a run has made. It goes to
//! `elab/functions/` with the methods that read it.

use super::variants::Variants;

/// The IR functions of one run: one variant per `fn` and flag pattern a call reached.
#[derive(Default)]
pub(in crate::elab) struct FunctionTables {
    /// The IR function of each `fn` and flag pattern a call reached.
    pub(in crate::elab) variants: Variants,
}
