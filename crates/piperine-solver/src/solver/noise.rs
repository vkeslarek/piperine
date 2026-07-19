use crate::analysis::ac::AcAnalysisContext;
use crate::prelude::DcAnalysisResult;
use crate::analysis::noise::NoiseAnalysisOptions;
use crate::prelude::NoiseAnalysisResult;
use std::collections::HashMap;
use crate::core::circuit::CircuitInstance;
use crate::analog::{AnalogReference, AnalogVariable};
use crate::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use crate::math::linear::{LinearSystem, Stamp, SymbolicLinearSystem, SymbolicMatrix};
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
        circuit.setup_all(&context)?;

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
        let mut psd_map: HashMap<(String, String), (crate::analysis::noise::NoiseKind, Vec<f64>)> = HashMap::new();

        for (i, &f) in frequencies.iter().enumerate() {
            let ac_ctx = AcAnalysisContext { frequency: f };

            let stamps = Self::assemble_linearized(self.circuit, &self.dc_point, f, &self.context)?;
            let adjoint_sol = self.solve_adjoint_system(stamps)?;

            let mut step_density = 0.0;
            for source in &mut self.circuit.devices {
                let noises = source.noise_current_psd(&self.dc_point, &ac_ctx);
                for (idx, n) in noises.into_iter().enumerate() {
                    let z_p = n.terminals.0
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let z_n = n.terminals.1
                        .idx()
                        .map(|i| adjoint_sol[i])
                        .unwrap_or_default();
                    let gain_sq = (z_p - z_n).norm_sqr();

                    let val = gain_sq * n.value;
                    step_density += val;

                    let key = (
                        source.name().to_string(),
                        n.name.clone().unwrap_or_else(|| idx.to_string())
                    );
                    let entry = psd_map.entry(key).or_insert_with(|| {
                        let mut vec = Vec::with_capacity(frequencies.len());
                        vec.resize(i, 0.0);
                        (n.kind, vec)
                    });
                    while entry.1.len() < i {
                        entry.1.push(0.0);
                    }
                    entry.1.push(val);
                }
            }
            out_noise_sq.push(step_density);
            for (_, entry) in psd_map.iter_mut() {
                if entry.1.len() == i {
                    entry.1.push(0.0);
                }
            }
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);

        let mut contributions = Vec::new();
        for ((element, source), (kind, psd)) in psd_map {
            let mut integrated_sq = 0.0;
            if frequencies.len() > 1 {
                for i in 0..frequencies.len() - 1 {
                    let df = frequencies[i + 1] - frequencies[i];
                    integrated_sq += 0.5 * df * (psd[i] + psd[i + 1]);
                }
            }
            contributions.push(crate::result::NoiseContribution {
                element,
                source,
                kind,
                integrated_sq,
                psd,
            });
        }

        Ok(NoiseAnalysisResult {
            frequencies,
            integrated_noise,
            out_noise_sq,
            contributions,
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
            frequency: f_hz,
        };
        let mut all_stamps = Vec::new();
        for ac in &mut circuit.devices {
            all_stamps.extend(ac.load_ac(dc_point, &ac_ctx, context));
        }
        Ok(all_stamps)
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
            .ok_or_else(|| {
                crate::error::Error::simple(crate::error::SolverDomain::Noise, "Output node not found")
            })?;
        let ref_ = net
            .reference_for(&AnalogVariable::Node(opt.reference_node.clone()))
            .ok_or_else(|| {
                crate::error::Error::simple(crate::error::SolverDomain::Noise, "Reference node not found")
            })?;
        Ok((out.clone(), ref_.clone()))
    }

    /// Integrates noise power spectral density to get RMS noise voltage.
    ///
    /// Trapezoidal integration through the shared [`Integrator`]:
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
        crate::math::integration::Integrator::trapezoid(freqs, psd).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::element::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
    use crate::analog::{AnalogReference, Netlist, NodeIdentifier};
    use crate::math::linear::Stamp;
    use num_complex::Complex64;
    use crate::analysis::ac::{AcSweepAnalysisOptions, AcAnalysisContext};
    use crate::prelude::DcAnalysisResult;
    use crate::analysis::noise::{Noise, NoiseKind};
    use crate::solver::Context;

    struct NoisyResistor {
        name: String,
        r: f64,
        n1: AnalogReference,
        n2: AnalogReference,
    }

    impl AnalogDevice for NoisyResistor {
        fn load_dc(
            &mut self,
            _state: &crate::abi::DcAnalysisState<'_>,
            _ctx: &Context,
        ) -> Vec<Stamp<AnalogReference, f64>> {
            let g = 1.0 / self.r;
            vec![
                Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
                Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
                Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
                Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
            ]
        }

        fn load_ac(
            &mut self,
            _dc_op: &DcAnalysisResult,
            _ac_ctx: &AcAnalysisContext,
            _context: &Context,
        ) -> Vec<Stamp<AnalogReference, Complex64>> {
            let g = Complex64::new(1.0 / self.r, 0.0);
            vec![
                Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
                Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
                Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
                Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
            ]
        }

        fn noise_current_psd(
            &mut self,
            _dc_point: &DcAnalysisResult,
            _ac_context: &AcAnalysisContext,
        ) -> Vec<Noise> {
            let thermal_psd = 4.0 * 1.380649e-23 * 300.0 / self.r;
            vec![
                Noise::new((self.n1.clone(), self.n2.clone()), thermal_psd).named("thermal", NoiseKind::Thermal),
                Noise::new((self.n1.clone(), self.n2.clone()), 1e-24).named("flicker", NoiseKind::Flicker)
            ]
        }
    }

    impl DigitalDevice for NoisyResistor {}

    impl Introspect for NoisyResistor {}

    impl Element for NoisyResistor {
        fn name(&self) -> &str { &self.name }

        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC | ElementCapabilities::EMITS_NOISE
        }
    }

    #[test]
    fn test_per_source_noise_reporting_and_conservation() {
        let mut netlist = Netlist::new();
        let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
        let gnd = netlist.connect_node(NodeIdentifier::Gnd);

        let r1 = NoisyResistor { name: "r1".to_string(), r: 1000.0, n1: top.clone(), n2: gnd.clone() };
        let r2 = NoisyResistor { name: "r2".to_string(), r: 1000.0, n1: top.clone(), n2: gnd.clone() };

        let elements: Vec<Box<dyn Element>> = vec![Box::new(r1), Box::new(r2)];
        let mut circuit = CircuitInstance::from_devices_and_netlist("test", elements, netlist);

        let ctx = Context::default();
        let mut dc = circuit.dc(ctx).unwrap();
        let _dc_res = dc.solve().unwrap();

        let sweep = AcSweepAnalysisOptions {
            start_frequency: 1.0,
            stop_frequency: 100.0,
            steps: 10,
            logarithmic: false,
        };
        
        let opts = NoiseAnalysisOptions {
            sweep_options: sweep,
            output_node: NodeIdentifier::Anonymous(1),
            reference_node: NodeIdentifier::Gnd,
            input_source_name: None,
        };

        let mut solver = circuit.noise(opts, Context::default()).unwrap();
        let res = solver.solve().unwrap();

        let contribs = res.contributions();
        assert_eq!(contribs.len(), 4);

        let mut found_thermal = 0;
        let mut found_flicker = 0;
        for c in contribs {
            if c.source == "thermal" && c.kind == NoiseKind::Thermal {
                found_thermal += 1;
                assert_eq!(c.psd.len(), 10);
            }
            if c.source == "flicker" && c.kind == NoiseKind::Flicker {
                found_flicker += 1;
                assert_eq!(c.psd.len(), 10);
            }
        }
        assert_eq!(found_thermal, 2);
        assert_eq!(found_flicker, 2);

        for i in 0..res.frequencies.len() {
            let sum_psd: f64 = contribs.iter().map(|c| c.psd[i]).sum();
            let total = res.out_noise_sq[i];
            
            let err = (sum_psd - total).abs();
            let rel_err = if total > 1e-30 { err / total } else { err };
            assert!(rel_err < 1e-9, "freq idx {}, sum {} != total {}", i, sum_psd, total);
        }
        
        let mut manual_int = 0.0;
        for i in 0..res.frequencies.len() - 1 {
            let df = res.frequencies[i + 1] - res.frequencies[i];
            manual_int += 0.5 * df * (res.out_noise_sq[i] + res.out_noise_sq[i + 1]);
        }
        assert!((res.integrated_noise - manual_int.sqrt()).abs() < 1e-12);
    }
}
