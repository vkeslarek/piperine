//! The rejection of an `Ir` that cannot run.

use piperine_ir::Broken;

/// An `Ir` that failed `check()`: the entries, each naming an item and its origin.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub entries: Vec<Broken>,
}
