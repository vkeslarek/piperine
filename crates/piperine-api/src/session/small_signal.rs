//! The DC small-signal analyses between a source and a node: the transfer function and the
//! poles and zeros.

use piperine_solver::prelude::{AnalogVariable, PoleZeroOptions, PoleZeroResult, TransferFunctionAnalysisOptions};

use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::table::Table;

/// Runs a `tf` or a `pz` request.
pub(super) struct SmallSignalRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> SmallSignalRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        SmallSignalRunner { run }
    }

    /// The DC transfer function from the input source to the output node.
    pub(super) fn transfer_function(self) -> Result<Table, Failure> {
        let AnalysisRun { session, mut circuit, design, .. } = self.run;
        let transfer = session.transfer()?;
        let options = TransferFunctionAnalysisOptions { output: AnalogVariable::Node(transfer.output.clone()), output_ref: None, input_source: transfer.input.clone() };
        let result = circuit
            .transfer_function(options, session.context.clone())
            .and_then(|mut tf| tf.solve())
            .map_err(|error| Failure::from_fault("tf", error, None, &design))?;
        Ok(Table::transfer(&result))
    }

    /// The poles and zeros of the transfer from the input source to the output node.
    pub(super) fn poles_and_zeros(self) -> Result<Table, Failure> {
        let AnalysisRun { session, mut circuit, design, .. } = self.run;
        let transfer = session.transfer()?;
        let options = PoleZeroOptions { input_source: transfer.input.clone(), output: AnalogVariable::Node(transfer.output.clone()), output_ref: None };
        let roots = circuit
            .pz(options, session.context.clone())
            .and_then(|pz| Ok(PoleZeroResult { poles: pz.poles()?, zeros: pz.zeros()? }))
            .map_err(|error| Failure::from_fault("pz", error, None, &design))?;
        Ok(Table::roots(&roots))
    }
}
