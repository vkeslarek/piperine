//! The reference interpreter (feature `ir-interp`): an elaborated [`Ir`] run through the solver
//! by walking each body's `analog` block over dual numbers. It spawns no process and reads no
//! file; the solver meets it through shapes and `eval` only (abi.md).

use std::sync::Arc;

use piperine_ir::{Broken, Ir};

/// An `Ir` that failed `check()`: the entries, each naming an item and its origin.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub entries: Vec<Broken>,
}

/// A checked `Ir` ready to be planned and run.
#[derive(Debug, Clone)]
pub struct Interpreter {
    ir: Arc<Ir>,
}

impl Interpreter {
    /// Accepts an `Ir` whose `check()` is empty; otherwise returns the entries.
    pub fn new(ir: Arc<Ir>) -> Result<Interpreter, Rejected> {
        let entries = ir.check();
        if entries.is_empty() { Ok(Interpreter { ir }) } else { Err(Rejected { entries }) }
    }

    /// The design this interpreter runs.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }
}
