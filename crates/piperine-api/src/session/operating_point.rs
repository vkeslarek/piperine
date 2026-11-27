//! The operating point.

use piperine_solver::prelude::Solver;

use super::Session;
use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::table::Table;

/// Runs an `op` request.
pub(super) struct OperatingPointRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> OperatingPointRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        OperatingPointRunner { run }
    }

    /// Solves the operating point and tabulates the probes at it.
    pub(super) fn solve(self) -> Result<Table, Failure> {
        let AnalysisRun { session, circuit, delivery, design, held, .. } = self.run;
        let mut solver = Solver::new(circuit).with_context(session.context.clone()).with_sink(delivery).build();
        let result = solver
            .dc()
            .and_then(|mut dc| dc.solve())
            .map_err(|error| Failure::from_fault("op", error, Some(0.0), &design))?;
        let circuit = solver.circuit();
        let net = |path: &str| Session::discrete(circuit, path).or_else(|| Session::held_bytes(&held, path));
        let term = |instance: &str, name: &str| circuit.bound().analog.term_value(instance, name);
        Table::operating_point(&session.probes, &result, &net, &term)
    }
}
