//! The analyses over frequency: the small-signal `ac` sweep and the `noise` density.

use piperine_solver::prelude::{AcSweepAnalysisOptions, NodeIdentifier, NoiseAnalysisOptions, Solver};

use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::noise::{NoiseOutput, NoiseReport};
use crate::request::Grid;
use crate::table::Table;

/// Runs an `ac` or a `noise` request.
pub(super) struct FrequencyRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> FrequencyRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        FrequencyRunner { run }
    }

    /// The sweep from `from` to `to` hertz on `grid`.
    fn sweep(from: f64, to: f64, grid: Grid) -> AcSweepAnalysisOptions {
        AcSweepAnalysisOptions { start_frequency: from, stop_frequency: to, steps: grid.steps(from, to), logarithmic: matches!(grid, Grid::PerDecade(_)) }
    }

    /// The `ac` sweep from `from` to `to` hertz.
    pub(super) fn ac(self, from: f64, to: f64, grid: Grid) -> Result<Table, Failure> {
        let AnalysisRun { session, circuit, delivery, design, .. } = self.run;
        let options = Self::sweep(from, to, grid);
        let mut solver = Solver::new(circuit).with_context(session.context.clone()).with_sink(delivery).build();
        let result = solver.ac().and_then(|mut ac| ac.solve_sweep(options)).map_err(|error| Failure::from_fault("ac", error, None, &design))?;
        Table::frequency(&session.probes, &result)
    }

    /// The `noise` density of the output the run resolved, from `from` to `to` hertz.
    pub(super) fn noise(self, from: f64, to: f64, grid: Grid) -> Result<Table, Failure> {
        let AnalysisRun { session, circuit, delivery, .. } = self.run;
        let probe = session.noise.as_ref().ok_or_else(|| Failure::Backend("a noise run resolved no output".into()))?;
        let (output_node, reference_node, output_flow) = match &probe.output {
            NoiseOutput::Potential { plus, minus } => (plus.clone(), minus.clone(), None),
            NoiseOutput::Flow(flow) => (NodeIdentifier::Gnd, NodeIdentifier::Gnd, Some(flow.clone())),
        };
        let options = NoiseAnalysisOptions { sweep_options: Self::sweep(from, to, grid), output_node, reference_node, input_source_name: probe.input.clone(), output_flow };
        let unit = probe.unit();
        let mut solver = Solver::new(circuit).with_context(session.context.clone()).with_sink(delivery).build();
        let result = solver.noise(options).and_then(|mut noise| noise.solve()).map_err(|error| Failure::Solver { analysis: "noise", error })?;
        let report = NoiseReport::of(&result, unit);
        let table = Table::noise(&report);
        session.noise_report = Some(report);
        Ok(table)
    }
}
