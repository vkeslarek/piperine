//! The compiled backend (feature `ir-rust`, MD-63): an elaborated [`Ir`] printed as one Rust
//! source text, built by `rustc` into a `cdylib` that exports `piperine_abi_1`, cached by hash
//! and loaded into the solver. Each body is planned by `piperine-plan`, the plan the interpreter
//! runs, so both backends give the solver the same shapes.

use std::sync::Arc;

use piperine_ir::{BodyId, Ir};
use piperine_plan::BodyPlan;

use crate::print::LibraryPrinter;

mod abi_source;
mod cache;
mod compile;
mod compiled;
mod error;
mod load;
pub mod print;
pub mod rt;
mod rustc;
mod settings;

pub use abi_source::AbiSource;
pub use cache::{Cache, Entry, Key, Manifest};
pub use compiled::{Compiled, Report};
pub use error::Error;
pub use load::Loaded;
pub use rustc::Rustc;
pub use settings::Settings;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

/// A checked `Ir`, each body that runs planned.
#[derive(Debug)]
pub struct Transpiler {
    ir: Arc<Ir>,
    /// One plan per body, by body index; `None` for a body with nothing to run.
    plans: Vec<Option<BodyPlan>>,
}

impl Transpiler {
    /// Accepts an `Ir` whose `check()` is empty, by the interpreter's rule; otherwise returns
    /// `Error::Rejected` with the entries. A body runs when it has an `analog` block or a node
    /// with an initial potential.
    pub fn new(ir: Arc<Ir>) -> Result<Transpiler, Error> {
        let entries = ir.check();
        if !entries.is_empty() {
            return Err(Error::Rejected(entries));
        }
        let plans = ir
            .bodies()
            .map(|body| {
                let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some());
                runs.then(|| BodyPlan::new(body))
            })
            .collect();
        Ok(Transpiler { ir, plans })
    }

    /// The design this transpiler prints.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// The plan of `body`; `None` for a body with nothing to run, or an id out of range.
    pub fn plan(&self, body: BodyId) -> Option<&BodyPlan> {
        self.plans.get(body.index()).and_then(Option::as_ref)
    }

    /// The design as one Rust source text (`design.rs`); a form with no lowering is
    /// `Error::Unsupported` and gives no text. The same `Ir` always gives the same bytes.
    pub fn source(&self) -> Result<String, Error> {
        LibraryPrinter::design(&self.ir, &self.plans)
    }
}
