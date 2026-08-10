//! [`InterpreterEvaluator`] — wraps [`piperine_ir::interp::Interpreter`],
//! the reference interpreter (T13), as an [`Evaluator`] for the
//! differential harness.

use super::fixture::{Fixture, ObservePoint};
use super::harness::Evaluator;
use piperine_ir::interp::{CellValue, Interpreter};

pub struct InterpreterEvaluator;

impl Evaluator for InterpreterEvaluator {
    fn name(&self) -> &'static str {
        "reference interpreter"
    }

    fn run(&self, fixture: &Fixture) -> Vec<CellValue> {
        let mut interp = Interpreter::new(&fixture.module);
        for step in &fixture.steps {
            for (v, val) in &step.seed_values {
                interp.seed_value(*v, val.clone());
            }
            for (n, across, through) in &step.node_values {
                interp.set_node_values(*n, *across, *through);
            }
            for (n, v) in &step.signal_values {
                interp.set_signal(*n, *v);
            }
            let outcome = match step.time {
                Some(t) => interp.step_to(t),
                None => interp.step(),
            };
            outcome.expect("interpreter step must not fail on a well-formed fixture");
        }
        fixture
            .observe
            .iter()
            .map(|obs| match obs.point {
                ObservePoint::Cell(c) => interp
                    .cell_value(c)
                    .unwrap_or_else(|| panic!("observation `{}`: cell {c:?} has no value", obs.label))
                    .clone(),
                ObservePoint::Value(v) => interp
                    .value(v)
                    .unwrap_or_else(|| panic!("observation `{}`: value {v:?} has no value", obs.label))
                    .clone(),
            })
            .collect()
    }
}
