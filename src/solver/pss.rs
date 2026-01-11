use crate::analysis::pss::{PssAnalysisOptions, PssAnalysisResult};
use crate::circuit::Circuit;
use crate::solver::Context;
use ndarray::Array1;

pub struct PssSolver<'a> {
    circuit: &'a mut Circuit,
    context: Context,
}

impl<'a> PssSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        Ok(Self { circuit, context })
    }
    pub fn solve(
        &mut self,
        options: PssAnalysisOptions,
    ) -> crate::result::Result<PssAnalysisResult> {
        todo!()
    }

    fn has_converged(
        &self,
        x0: &Array1<f64>,
        xT: &Array1<f64>,
        options: &PssAnalysisOptions,
    ) -> bool {
        let diff = (xT - x0).mapv(|a| a.abs());
        diff.iter().all(|&e| e < options.pss_reltol)
    }
}
