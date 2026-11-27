//! The periodic steady state.

use piperine_solver::prelude::PssAnalysisOptions;

use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::table::Table;

/// Runs a `pss` request.
pub(super) struct PeriodicRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> PeriodicRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        PeriodicRunner { run }
    }

    /// The orbit of a circuit driven with period `period`, after a transient of `tstab`.
    pub(super) fn solve(self, period: f64, tstab: f64, max_step: Option<f64>, print_step: Option<f64>) -> Result<Table, Failure> {
        let AnalysisRun { session, mut circuit, design, held, .. } = self.run;
        let mut options = PssAnalysisOptions::new(period).with_tstab(tstab);
        options.dt = max_step;
        options.dt_max = max_step;
        options.max_steps = session.spec.options.max_steps.map(|budget| budget as usize);
        let orbit = circuit
            .pss(options, session.context.clone())
            .and_then(|pss| pss.solve())
            .map_err(|error| Failure::from_fault("pss", error, None, &design))?;
        // the orbit starts at `tstab` and holds one period
        let table = Table::transient_with(&session.probes, &orbit.trace, &held)?;
        match print_step {
            Some(step) => table.on_grid(step, tstab, tstab + period),
            None => Ok(table),
        }
    }
}
