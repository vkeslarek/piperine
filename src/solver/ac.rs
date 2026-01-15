use crate::analysis::ac::{AcAnalysisContext, AcAnalysisResult, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::array::IndexedArray2;
use crate::math::iv::InitialValue;
use crate::math::linear::{Stamp, SymbolicMatrix};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::{Array1, Array2, ArrayView1};
use num_complex::Complex;
use std::collections::HashMap;

pub struct AcSolver<'a> {
    pub linearizer: AcAnalysisStamper<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, Complex<f64>>,
}

impl<'a> AcSolver<'a> {
    pub fn new(circuit: &'a mut Circuit, context: Context) -> crate::result::Result<Self> {
        let dc_point = circuit.dc(context.clone())?.solve()?;

        let mut linearizer = AcAnalysisStamper { circuit, dc_point };

        let solver = NewtonRaphsonSolver::create(&mut linearizer, context)?;

        Ok(Self { linearizer, solver })
    }

    pub fn solve_sweep(
        &mut self,
        options: AcSweepAnalysisOptions,
    ) -> crate::result::Result<AcAnalysisResult> {
        let frequencies = self.generate_frequencies(&options);

        let mut data = Array2::zeros((frequencies.len(), self.solver.symbolic_matrix.size()));
        let mut inputs = HashMap::new();

        for (idx, &f_hz) in frequencies.iter().enumerate() {
            inputs.insert(CircuitReference::Frequency, Complex::new(f_hz, 0.0));

            let solution = self
                .solver
                .step_steady_state(&mut self.linearizer, &inputs)?;

            data.row_mut(idx).assign(&solution);
        }

        Ok(AcAnalysisResult {
            mapping: self.solver.symbolic_matrix.mapping.clone(),
            frequencies,
            data,
        })
    }

    fn generate_frequencies(&self, opt: &AcSweepAnalysisOptions) -> Vec<f64> {
        if opt.steps <= 1 {
            return vec![opt.start_frequency];
        }

        (0..opt.steps)
            .map(|i| {
                let ratio = i as f64 / (opt.steps - 1) as f64;
                if opt.logarithmic {
                    opt.start_frequency * (opt.stop_frequency / opt.start_frequency).powf(ratio)
                } else {
                    opt.start_frequency + (opt.stop_frequency - opt.start_frequency) * ratio
                }
            })
            .collect()
    }
}

pub struct AcAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
}

impl<'a> AcAnalysisStamper<'a> {
    fn get_context(
        &self,
        state: &IndexedArray2<CircuitReference, Complex<f64>>,
    ) -> AcAnalysisContext {
        let freq = state
            .latest()
            .and_then(|vals| vals.get(&CircuitReference::Frequency).cloned())
            .map(|c| c.re)
            .unwrap_or(1.0);

        AcAnalysisContext {
            frequency: freq.Hz(),
        }
    }
}

impl<'a> NewtonRaphsonStamper<CircuitReference, Complex<f64>> for AcAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &IndexedArray2<CircuitReference, Complex<f64>>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = self.get_context(state);

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
        _state: &IndexedArray2<CircuitReference, Complex<f64>>,
        _context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        Ok(Vec::new())
    }

    fn initial_conditions(
        &mut self,
        _context: &Context,
    ) -> crate::result::Result<Vec<InitialValue<CircuitReference, Complex<f64>>>> {
        Ok(Vec::new())
    }

    fn active_symbols(&self) -> Vec<CircuitReference> {
        self.circuit
            .netlist()
            .all_references()
            .into_iter()
            .filter(|s| s.is_dependent())
            .collect()
    }

    fn independent_symbols(&self) -> Vec<CircuitReference> {
        vec![CircuitReference::Frequency]
    }

    fn converged(
        &self,
        _state: &IndexedArray2<CircuitReference, Complex<f64>>,
        _sol: &ArrayView1<Complex<f64>>,
        _ctx: &Context,
    ) -> bool {
        true
    }
}
