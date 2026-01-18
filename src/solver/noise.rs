use crate::analysis::ac::{AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseAnalysisResult};
use crate::circuit::Circuit;
use crate::circuit::netlist::CircuitReference;
use crate::math::array::IndexedArray2;
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::iv::InitialValue;
use crate::math::linear::{SparseLinearSystem, Stamp, SymbolicMatrix};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper};
use crate::math::unit::UnitExt;
use crate::solver::Context;
use ndarray::ArrayView1;
use num_complex::Complex;

pub struct NoiseAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
}

impl<'a> NoiseAnalysisStamper<'a> {
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

impl<'a> NewtonRaphsonStamper<CircuitReference, Complex<f64>> for NoiseAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        state: &IndexedArray2<CircuitReference, Complex<f64>>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = self.get_context(state);

        // Reuse AC implementations for component linearization
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

pub struct NoiseSolver<'a> {
    pub linearizer: NoiseAnalysisStamper<'a>,
    pub solver: NewtonRaphsonSolver<CircuitReference, Complex<f64>>,
    pub options: NoiseAnalysisOptions,
    pub adjoint_symbolic_matrix: FaerSymbolicMatrix<CircuitReference>,
}

impl<'a> NoiseSolver<'a> {
    pub fn new(
        circuit: &'a mut Circuit,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        let dc_point = circuit.dc(context.clone())?.solve()?;

        let mut linearizer = NoiseAnalysisStamper { circuit, dc_point };

        let solver = NewtonRaphsonSolver::create(&mut linearizer, context.clone())?;

        let stamps = linearizer.static_stamps(&solver.state, &context)?;
        let adjoint_stamps: Vec<_> = stamps
            .into_iter()
            .map(|s| match s {
                Stamp::Matrix(r, c, val) => Stamp::Matrix(c, r, val), // Transpose!
                s => s,
            })
            .collect();

        let adjoint_symbolic_matrix = FaerSymbolicMatrix::new(
            solver.symbolic_matrix.mapping().keys().cloned().collect(),
            adjoint_stamps,
        )?;

        Ok(Self {
            linearizer,
            solver,
            options,
            adjoint_symbolic_matrix,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<NoiseAnalysisResult> {
        let frequencies = self.generate_frequencies(&self.options.sweep_options);
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());

        let mapping = self.adjoint_symbolic_matrix.mapping().clone();
        let matrix_size = self.adjoint_symbolic_matrix.size();

        let out_idx = mapping
            .get(&CircuitReference::Node(self.options.output_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Output node not found"))?;
        let ref_idx = mapping.get(&CircuitReference::Node(self.options.reference_node.clone()));

        for &f in &frequencies {
            if let Some(mut view) = self.solver.state.latest_mut() {
                view.set(&CircuitReference::Frequency, &Complex::new(f, 0.0));
            }

            let stamps = self
                .linearizer
                .static_stamps(&self.solver.state, &self.solver.context)?;

            let mut sparse_system = FaerSparseLinearSystem::new(matrix_size);

            let adjoint_stamps: Vec<_> = stamps
                .into_iter()
                .map(|s| match s {
                    Stamp::Matrix(r, c, val) => Stamp::Matrix(c, r, val),
                    _ => s,
                })
                .collect();

            sparse_system.apply_stamps(&self.adjoint_symbolic_matrix, adjoint_stamps);

            sparse_system.b_vec[*out_idx] = Complex::new(1.0, 0.0);
            if let Some(&r_i) = ref_idx {
                sparse_system.b_vec[r_i] = Complex::new(-1.0, 0.0);
            }

            let adjoint_solution =
                sparse_system.solve_with_backend(&self.adjoint_symbolic_matrix)?;

            let mut total_density = 0.0;
            let ac_ctx = AcAnalysisContext { frequency: f.Hz() };

            for comp in self.linearizer.circuit.components_mut().values_mut() {
                if let Some(source) = comp.as_noise_source() {
                    let noises = source.noise_current_psd(&self.linearizer.dc_point, &ac_ctx);

                    for n in noises {
                        let p_idx = mapping.get(&n.terminals.0);
                        let n_idx = mapping.get(&n.terminals.1);

                        let z_p = p_idx
                            .map(|&i| adjoint_solution[i])
                            .unwrap_or(Complex::new(0.0, 0.0));
                        let z_n = n_idx
                            .map(|&i| adjoint_solution[i])
                            .unwrap_or(Complex::new(0.0, 0.0));

                        let h_vec = z_p - z_n;
                        let gain_sq = h_vec.norm_sqr();
                        total_density += gain_sq * n.value;
                    }
                }
            }
            out_noise_sq.push(total_density);
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);

        Ok(NoiseAnalysisResult {
            mapping,
            frequencies,
            out_noise_sq,
            integrated_noise,
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

    fn integrate_noise(&self, freqs: &[f64], psd: &[f64]) -> f64 {
        let mut sum_sq = 0.0;
        for i in 0..freqs.len() - 1 {
            let df = freqs[i + 1] - freqs[i];
            let avg_psd = (psd[i] + psd[i + 1]) / 2.0;
            sum_sq += avg_psd * df;
        }
        sum_sq.sqrt()
    }
}
