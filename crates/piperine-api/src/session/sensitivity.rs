//! The DC sensitivity of the printed signals to params.

use piperine_solver::prelude::{Net, SensAnalysisOptions};

use super::Session;
use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::table::Table;

/// Runs a `sens` request.
pub(super) struct SensitivityRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> SensitivityRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        SensitivityRunner { run }
    }

    /// The sensitivity of each printed signal to each param `wrt` names.
    pub(super) fn solve(self) -> Result<Table, Failure> {
        let AnalysisRun { session, mut circuit, design, .. } = self.run;
        let nets = circuit.nets();
        let outputs = session.probes.iter().map(|probe| Session::net_of(&nets, probe)).collect::<Result<Vec<Net>, Failure>>()?;
        let params = session.wrt.iter().map(|path| (path.instance.clone(), path.param.clone())).collect();
        let result = circuit
            .sens(SensAnalysisOptions::new(outputs.clone(), params), session.context.clone())
            .and_then(|sens| sens.solve())
            .map_err(|error| Failure::from_fault("sens", error, None, &design))?;
        Table::sensitivities(&session.probes, &outputs, &session.wrt, &result)
    }
}
