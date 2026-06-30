use crate::analysis::ac::AcAnalysisContext;
use crate::analysis::dc::DcAnalysisResult;
use crate::analysis::noise::{NoiseAnalysisOptions, NoiseAnalysisResult};
use crate::circuit::CircuitInstance;
use crate::analog::{AnalogReference, AnalogVariable};
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::math::unit::UnitExt;
use crate::solver::dc::DcSolver;
use crate::solver::Context;
use ndarray::Array1;
use num_complex::Complex;

/// Noise analysis solver for computing circuit noise floor.
///
/// This solver uses the adjoint method to efficiently compute the noise contribution
/// from all noise sources (thermal, shot, flicker) to the output. It performs:
/// 1. DC operating point calculation
/// 2. Circuit linearization at each frequency
/// 3. Adjoint system solution to find noise transfer
/// 4. Integration of noise power spectral density
///
/// The adjoint method solves the transposed system once per frequency, which is
/// more efficient than solving for each noise source individually.
pub struct NoiseSolver<'a> {
    pub circuit: &'a mut CircuitInstance,
    pub context: Context,
    pub dc_point: DcAnalysisResult,
    pub symbolic_matrix: FaerSymbolicMatrix,
    pub options: NoiseAnalysisOptions,
    pub out_ref: AnalogReference,
    pub ref_ref: AnalogReference,
}

impl<'a> NoiseSolver<'a> {
    /// Creates a new noise solver and computes DC operating point.
    ///
    /// # Process
    /// 1. Initialize solver configuration
    /// 2. Solve for DC operating point (required for small-signal parameters)
    /// 3. Resolve output and reference node references
    /// 4. Build symbolic matrix structure (sparsity pattern) for efficiency
    ///
    /// # Arguments
    /// * `circuit` - Circuit instance to analyze
    /// * `options` - Noise analysis parameters (frequency sweep, output nodes)
    /// * `context` - Solver context with tolerances and temperature
    ///
    /// # Returns
    /// Initialized noise solver ready for analysis
    pub fn new(
        circuit: &'a mut CircuitInstance,
        options: NoiseAnalysisOptions,
        context: Context,
    ) -> crate::result::Result<Self> {
        Context::init_global();

        let dc_point = DcSolver::new(circuit, context.clone())?.solve()?;

        let (out_ref, ref_ref) = Self::resolve_nodes(circuit, &options)?;
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let symbolic_stamps = Self::assemble_linearized(circuit, &dc_point, 1.0, &context)?;
        let symbolic_matrix = FaerSymbolicMatrix::new(size, symbolic_stamps)?;

        Ok(Self {
            circuit,
            context,
            dc_point,
            symbolic_matrix,
            options,
            out_ref,
            ref_ref,
        })
    }

    /// Performs noise analysis across the frequency sweep.
    ///
    /// # Algorithm
    /// For each frequency:
    /// 1. Linearize circuit at current frequency
    /// 2. Solve adjoint system (transposed matrix) to find noise transfer
    /// 3. For each noise source:
    ///    - Get noise power spectral density (PSD)
    ///    - Calculate transfer from source to output: |H(f)|²
    ///    - Accumulate: total_PSD += |H(f)|² × source_PSD
    /// 4. Integrate total PSD over frequency to get RMS noise voltage
    ///
    /// # Returns
    /// Noise analysis result containing:
    /// - Frequency points
    /// - Output noise PSD at each frequency (V²/Hz)
    /// - Integrated RMS noise voltage (V)
    pub fn solve(&mut self) -> crate::result::Result<NoiseAnalysisResult> {
        let frequencies = self.options.sweep_options.generate_frequencies();
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());

        for &f in &frequencies {
            let ac_ctx = AcAnalysisContext { frequency: f.Hz() };

            let stamps = Self::assemble_linearized(self.circuit, &self.dc_point, f, &self.context)?;
            let adjoint_sol = self.solve_adjoint_system(stamps)?;

            let mut step_density = 0.0;
            for source in self.circuit.all_devices_mut() {
                let noises = source.noise_current_psd(&self.dc_point, &ac_ctx);
                for n in noises {
                    let z_p = self
                        .out_ref
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let z_n = self
                        .ref_ref
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let gain_sq = (z_p - z_n).norm_sqr();

                    step_density += gain_sq * n.value;
                }
            }
            out_noise_sq.push(step_density);
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);
        Ok(NoiseAnalysisResult {
            frequencies,
            integrated_noise,
            out_noise_sq,
        })
    }

    /// Solves the adjoint system to find noise transfer functions.
    ///
    /// The adjoint method transposes the system matrix and solves with a unit
    /// excitation at the output nodes. This gives the transfer function from
    /// every node to the output in a single solve, which is much more efficient
    /// than solving for each noise source individually.
    ///
    /// # Algorithm
    /// 1. Build transposed system: swap row/col indices of all stamps
    /// 2. Apply unit current at output: I_out = +1, I_ref = -1
    /// 3. Solve: [Y^T] × Z = I_unit
    /// 4. Result Z contains transfer impedances to output
    ///
    /// # Arguments
    /// * `stamps` - Linearized AC stamps at current frequency
    ///
    /// # Returns
    /// Adjoint solution vector (transfer impedances)
    fn solve_adjoint_system(
        &self,
        stamps: Vec<Stamp<AnalogReference, Complex<f64>>>,
    ) -> crate::result::Result<Array1<Complex<f64>>> {
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp {
                system.apply_stamps(vec![Stamp::Matrix(c, r, val)]);
            }
        }

        system.apply_stamps(vec![
            Stamp::Rhs(self.out_ref.clone(), Complex::new(1.0, 0.0)),
            Stamp::Rhs(self.ref_ref.clone(), Complex::new(-1.0, 0.0)),
        ]);

        system.solve_with_backend(&self.symbolic_matrix)
    }

    /// Assembles linearized circuit stamps at a given frequency.
    ///
    /// Uses the DC operating point to get small-signal parameters from all
    /// AC-capable devices (resistors, capacitors, inductors, diodes, etc.).
    ///
    /// # Arguments
    /// * `circuit` - Circuit instance
    /// * `dc_point` - DC operating point for linearization
    /// * `f_hz` - Frequency in Hz
    /// * `context` - Solver context
    ///
    /// # Returns
    /// Vector of complex-valued stamps for the linearized system
    fn assemble_linearized(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        f_hz: f64,
        context: &Context,
    ) -> crate::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let ac_ctx = AcAnalysisContext {
            frequency: f_hz.Hz(),
        };
        Ok(circuit.all_devices_mut()
            .iter_mut()
            .flat_map(|ac| ac.load_ac(dc_point, &ac_ctx, context))
            .collect())
    }

    /// Resolves output and reference node identifiers to circuit references.
    ///
    /// # Arguments
    /// * `circuit` - Circuit instance
    /// * `opt` - Noise analysis options containing node identifiers
    ///
    /// # Returns
    /// Tuple of (output_reference, reference_reference)
    ///
    /// # Errors
    /// Returns error if either node is not found in the netlist
    fn resolve_nodes(
        circuit: &CircuitInstance,
        opt: &NoiseAnalysisOptions,
    ) -> crate::result::Result<(AnalogReference, AnalogReference)> {
        let net = circuit.netlist();
        let out = net
            .reference_for(&AnalogVariable::Node(opt.output_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Output node not found"))?;
        let ref_ = net
            .reference_for(&AnalogVariable::Node(opt.reference_node.clone()))
            .ok_or_else(|| crate::error::Error::simple("Noise", "Reference node not found"))?;
        Ok((out.clone(), ref_.clone()))
    }

    /// Integrates noise power spectral density to get RMS noise voltage.
    ///
    /// Uses trapezoidal integration to calculate total noise power:
    ///   P_total = ∫ PSD(f) df
    ///   V_rms = √P_total
    ///
    /// # Arguments
    /// * `freqs` - Frequency points (Hz)
    /// * `psd` - Noise power spectral density at each frequency (V²/Hz)
    ///
    /// # Returns
    /// Integrated RMS noise voltage (V)
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

