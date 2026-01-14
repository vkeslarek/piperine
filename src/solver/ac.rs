use crate::analysis::ac::{AcAnalysisContext, AcAnalysisResult, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::Stamp;
use crate::math::linear::SymbolicMatrix;
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper, SolverState};
use crate::math::unit::UnitExt;
use crate::math::vector::InitialValue;
use crate::solver::Context;
use ndarray::{Array1, Array2, ArrayView1};
use num_complex::Complex;

pub struct AcAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
    pub current_frequency: f64,
}

impl<'a> NewtonRaphsonStamper<CircuitReference, Complex<f64>> for AcAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        _state: &SolverState<CircuitReference, Complex<f64>>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.current_frequency.Hz(),
        };

        // Linearize around the stored DC operating point
        Ok(self
            .circuit
            .components_mut()
            .values_mut()
            .filter_map(|c| c.as_ac())
            .flat_map(|ac| ac.load_ac(&self.dc_point, &ac_ctx, context))
            .filter(|s| !s.has_ground_node())
            .collect())
    }

    fn dynamic_stamps(
        &mut self,
        _state: &SolverState<CircuitReference, Complex<f64>>,
        _context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        // In AC, reactive elements are part of the static complex matrix (jωC)
        Ok(Vec::new())
    }

    fn initial_conditions(
        &mut self,
        _context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<CircuitReference, Complex<f64>>>> {
        Ok(Vec::new()) // AC usually starts from zero/source phasors
    }

    fn active_symbols(&self) -> Vec<CircuitReference> {
        self.circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| s.is_dependent())
            .collect()
    }

    fn independent_symbols(&self) -> Vec<IndependentVariable> {
        vec![IndependentVariable::Time] // Using Time as the proxy for Frequency/Omega
    }

    fn converged(
        &self,
        _state: &SolverState<CircuitReference, Complex<f64>>,
        _sol: &ArrayView1<Complex<f64>>,
        _ctx: &Context,
    ) -> bool {
        true // AC is linear; it "converges" in one step
    }
}

pub struct AcSolver<'a> {
    pub linearizer: AcAnalysisStamper<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, Complex<f64>>,
}

impl<'a> AcSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        // 1. AC requires a DC operating point first
        let dc_point = circuit.dc(context.clone())?.solve()?;

        let mut linearizer = AcAnalysisStamper {
            circuit,
            dc_point,
            current_frequency: 1.0,
        };

        // 2. Build the complex symbolic solver
        let solver = NewtonRaphsonSolver::create(&mut linearizer, context)?;

        Ok(Self { linearizer, solver })
    }

    pub fn solve_sweep(
        &mut self,
        options: AcSweepAnalysisOptions,
    ) -> crate::result::Result<AcAnalysisResult> {
        let frequencies = self.generate_frequencies(&options);
        let mut data = Array2::zeros((frequencies.len(), self.solver.symbolic_matrix.size()));

        for (idx, &f) in frequencies.iter().enumerate() {
            self.linearizer.current_frequency = f;

            let solution = self.solver.step(
                &mut self.linearizer,
                &Array1::from_elem(1, f).view(),
                &IndependentVariable::Frequency,
            )?;

            data.row_mut(idx).assign(&solution);
        }

        Ok(AcAnalysisResult {
            mapping: self.solver.symbolic_matrix.mapping.clone(),
            frequencies,
            data,
        })
    }

    fn generate_frequencies(&self, opt: &AcSweepAnalysisOptions) -> Vec<f64> {
        (0..opt.steps)
            .map(|i| {
                let ratio = i as f64 / (opt.steps - 1).max(1) as f64;
                if opt.logarithmic {
                    opt.start_frequency * (opt.stop_frequency / opt.start_frequency).powf(ratio)
                } else {
                    opt.start_frequency + (opt.stop_frequency - opt.start_frequency) * ratio
                }
            })
            .collect()
    }
}
