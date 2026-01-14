use crate::analysis::ac::{AcAnalysisContext, AcSweepAnalysisOptions};
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseAnalysisResult};
use crate::circuit::Circuit;
use crate::circuit::netlist::{CircuitReference, IndependentVariable};
use crate::math::Stamp;
use crate::math::faer::FaerDenseSolver;
use crate::math::linear::{DenseLinearSystem, SymbolicMatrix};
use crate::math::newton_raphson::{NewtonRaphsonSolver, NewtonRaphsonStamper, SolverState};
use crate::math::unit::UnitExt;
use crate::math::vector::InitialValue;
use crate::solver::Context;
use ndarray::{Array1, Array2, ArrayView1};
use num_complex::Complex;

pub struct NoiseAnalysisStamper<'a> {
    pub circuit: &'a mut Circuit,
    pub dc_point: DcAnalysisResult,
    pub current_frequency: f64,
}

impl<'a> NewtonRaphsonStamper<CircuitReference, Complex<f64>> for NoiseAnalysisStamper<'a> {
    fn static_stamps(
        &mut self,
        _state: &SolverState<CircuitReference, Complex<f64>>,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<CircuitReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: self.current_frequency.Hz(),
        };

        // Reuse AC implementations
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

    fn independent_symbols(&self) -> Vec<IndependentVariable> {
        vec![IndependentVariable::Frequency]
    }

    fn converged(
        &self,
        _state: &SolverState<CircuitReference, Complex<f64>>,
        _sol: &ArrayView1<Complex<f64>>,
        _ctx: &Context,
    ) -> bool {
        true
    }
}

pub struct NoiseSolver<'a> {
    pub linearizer: NoiseAnalysisStamper<'a>,
    // We use the generic solver mainly as a container for the SymbolicMatrix and State
    pub solver: NewtonRaphsonSolver<CircuitReference, Complex<f64>>,
    pub options: NoiseAnalysisOptions,
}

impl<'a> NoiseSolver<'a> {
    pub fn new(
        circuit: &'a mut Circuit,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        // 1. Solve DC Operating Point first
        let dc_point = circuit.dc(context.clone())?.solve()?;

        let mut linearizer = NoiseAnalysisStamper {
            circuit,
            dc_point,
            current_frequency: 1.0,
        };

        // 2. Create the solver
        // This initializes the SymbolicMatrix and SolverState correctly via the stamper
        let solver = NewtonRaphsonSolver::create(&mut linearizer, context)?;

        Ok(Self {
            linearizer,
            solver,
            options,
        })
    }

    pub fn solve(&mut self) -> crate::result::Result<NoiseAnalysisResult> {
        let frequencies = self.generate_frequencies(&self.options.sweep_options);
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());

        let mapping = self.solver.symbolic_matrix.mapping().clone();
        let matrix_size = self.solver.symbolic_matrix.size();

        // Output indices
        let out_idx = mapping
            .get(&CircuitReference::Node(self.options.output_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Output node not found"))?;
        let ref_idx = mapping.get(&CircuitReference::Node(self.options.reference_node.clone()));

        // Pre-allocate the dense solver
        let mut dense_solver = FaerDenseSolver::<Complex<f64>>::new(matrix_size);

        // Pre-allocate reusable arrays
        let mut y_transposed = Array2::<Complex<f64>>::zeros((matrix_size, matrix_size));
        let mut rhs = Array1::<Complex<f64>>::zeros(matrix_size);

        for &f in &frequencies {
            // 1. Update Frequency
            self.linearizer.current_frequency = f;

            // 2. Get Stamps directly (Bypassing NR solver step logic)
            // We pass the existing state, though Noise stamps usually don't depend on state variables
            let stamps = self
                .linearizer
                .static_stamps(&self.solver.state, &self.solver.context)?;

            // 3. Build the Transposed Y Matrix
            // Reset matrix to zero
            y_transposed.fill(Complex::new(0.0, 0.0));

            for stamp in stamps {
                if let Stamp::Matrix(r, c, val) = stamp {
                    // Transpose Logic: swap rows and columns
                    // Matrix[col, row] += val
                    if let (Some(&r_i), Some(&c_i)) = (mapping.get(&r), mapping.get(&c)) {
                        y_transposed[[c_i, r_i]] += val;
                    }
                }
            }

            // 4. Configure Dense Solver
            dense_solver.set_matrix(&y_transposed);

            // 5. Setup RHS (Test Current of 1.0 at Output)
            rhs.fill(Complex::new(0.0, 0.0));
            rhs[*out_idx] = Complex::new(1.0, 0.0);
            if let Some(&r_i) = ref_idx {
                rhs[r_i] = Complex::new(-1.0, 0.0);
            }
            dense_solver.set_rhs(&rhs);

            // 6. Solve Adjoint System
            let adjoint_solution = dense_solver.solve()?;

            // 7. Accumulate Noise
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
                        let s_noise = n.value;

                        total_density += gain_sq * s_noise;
                    }
                }
            }
            out_noise_sq.push(total_density);
        }

        // 8. Integrate
        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);

        Ok(NoiseAnalysisResult {
            mapping,
            frequencies,
            out_noise_sq,
            integrated_noise,
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
