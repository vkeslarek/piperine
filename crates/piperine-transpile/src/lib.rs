//! The compiled backend (feature `ir-rust`, MD-63): an elaborated [`Ir`] printed as one Rust
//! source text, built by `rustc` into a `cdylib` that exports `piperine_abi_1`, cached by hash
//! and loaded into the solver. Each body is planned by `piperine-plan`, the plan the interpreter
//! runs, so both backends give the solver the same shapes.

use std::sync::Arc;

use piperine_ir::{Body, BodyId, Expr, Ir, ValueType};
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
mod shape_check;
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

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/alone.rs"]
mod alone;

/// A checked `Ir`, each body that runs planned.
#[derive(Debug)]
pub struct Transpiler {
    ir: Arc<Ir>,
    /// One plan per body, by body index; `None` for a body with nothing to run.
    plans: Vec<Option<BodyPlan>>,
}

impl Transpiler {
    /// Accepts an `Ir` whose `check()` is empty, by the interpreter's rule; otherwise returns
    /// `Error::Rejected` with the entries. A body runs when it has an `analog` block, a node
    /// with an initial potential or a digital form.
    pub fn new(ir: Arc<Ir>) -> Result<Transpiler, Error> {
        let entries = ir.check();
        if !entries.is_empty() {
            return Err(Error::Rejected(entries));
        }
        let plans = ir
            .bodies()
            .map(|body| {
                let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some()) || Transpiler::has_digital_form(body);
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

    /// Whether `body` holds a digital form: a carrier, a drive, an event, a reaction or a record
    /// stream. A body with reactions and no carrier (a monitor that only emits) is one.
    fn has_digital_form(body: Body<'_>) -> bool {
        !(body.carriers().is_empty()
            && body.drives().is_empty()
            && body.events().is_empty()
            && body.reactions().is_empty()
            && body.streams().is_empty())
    }

    /// Whether the design holds a digital form in any body, so its runtime is the three files
    /// of `rt` and not `real.rs` alone.
    pub fn digital(&self) -> bool {
        self.ir.bodies().any(Transpiler::has_digital_form)
    }

    /// Whether the design holds a loop or a recursive function, whose printed forms count with
    /// `rt::Trips` and `rt::Depth` (`rt/control.rs`): its runtime is the files of `rt` and not
    /// `real.rs` alone.
    pub fn loops(&self) -> bool {
        self.ir.expressions().any(|e| matches!(e, Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. }))
            || self.ir.functions().any(|f| print::ValuePrinter::is_recursive(&self.ir, f.id()))
    }

    /// Whether a param of the design holds a type the pin carries beyond `f64` and `bool`: an
    /// integer, an enum or an `Option` (PI-01 to PI-03). Its generated reads spell `rt::UInt` and
    /// `rt::Carried`, so its runtime is the files of `rt` and not `real.rs` alone.
    pub fn carried_params(&self) -> bool {
        self.ir.bodies().any(|body| body.params().iter().any(|param| !matches!(param.ty, ValueType::F64 | ValueType::Bool)))
    }

    /// Whether the design reads a session parameter or writes a trace: a trace's record is written by
    /// `rt::Carried`, and a session parameter's pin holds an `Option<f64>` the design reads as one, so
    /// its runtime is the files of `rt` and not `real.rs` alone.
    pub fn carried_sites(&self) -> bool {
        self.ir.bodies().any(|body| !body.session_parameters().is_empty() || !body.traces().is_empty())
    }

    /// Whether the design derives a param from another: its derive functions read pins through `rt::Carried`.
    pub fn derives(&self) -> bool {
        !piperine_plan::DerivePlan::of(&self.ir).bindings().is_empty()
    }

    /// Whether the design holds a `table_model`, whose generated code builds an `rt::TableModel`.
    pub fn tables(&self) -> bool {
        self.ir.expressions().any(|e| matches!(e, Expr::Table { .. } | Expr::TableSlope { .. }))
    }

    /// The design as one Rust source text (`design.rs`); a form with no lowering is
    /// `Error::Unsupported` and gives no text. The same `Ir` always gives the same bytes.
    pub fn source(&self) -> Result<String, Error> {
        LibraryPrinter::design(&self.ir, &self.plans)
    }
}
