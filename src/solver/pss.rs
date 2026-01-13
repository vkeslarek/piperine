use crate::analysis::pss::{PssAnalysisOptions, PssAnalysisResult};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::newton_raphson::{NewtonRaphsonStamper, SolverState};
use crate::solver::Context;
use crate::solver::transient::TransientAnalysisStamper;
use ndarray::ArrayView1;
use crate::math::{InitialValue, Stamp, Symbol};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct OffsetCircuitReference {
    pub offset: usize,
    pub reference: CircuitReference,
}

impl Symbol for OffsetCircuitReference {}

pub struct PssAnalysisStamper<'a> {
    pub transient_analysis_stamper: TransientAnalysisStamper<'a>,
    pub period: f64,
    pub number_of_timepoints: usize,
}

impl<'a> PssAnalysisStamper<'a> {
    pub fn new(circuit: &'a mut Circuit, period: f64, number_of_timepoints: usize) -> Self {
        Self {
            transient_analysis_stamper: TransientAnalysisStamper::new(circuit),
            period,
            number_of_timepoints,
        }
    }
}

impl NewtonRaphsonStamper<OffsetCircuitReference, f64> for PssAnalysisStamper<'_> {
    fn static_stamps(
        &mut self,
        state: &SolverState<OffsetCircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<OffsetCircuitReference, f64>>> {
        todo!()
    }

    fn dynamic_stamps(
        &mut self,
        state: &SolverState<OffsetCircuitReference, f64>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<OffsetCircuitReference, f64>>> {
        todo!()
    }

    fn initial_conditions(
        &mut self,
        context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<OffsetCircuitReference, f64>>> {
        todo!()
    }

    fn active_symbols(&self) -> Vec<OffsetCircuitReference> {
        todo!()
    }

    fn independent_symbols(&self) -> Vec<IndependentVariable> {
        todo!()
    }

    fn converged(
        &self,
        state: &SolverState<OffsetCircuitReference, f64>,
        solution: &ArrayView1<f64>,
        context: &Context,
    ) -> bool {
        todo!()
    }
}

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
}
