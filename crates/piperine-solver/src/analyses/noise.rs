//! Noise (`.noise`) — the noise vocabulary (`Noise`/`NoiseKind`) and options,
//! plus the adjoint-method driver integrating per-source contributions over
//! the frequency sweep. A device declares each source as a `NOISE` input of its
//! shape; the assembly reports its columns and PSDs.
//!
//! There is no per-analysis `NoiseContext`: MD-03 is superseded
//! (p6-cleanup-architecture D2). The sweep, output/reference nodes and input
//! source travel on [`NoiseAnalysisOptions`], and the shared
//! [`Context`](crate::analyses::Context) carries what every analysis shares.
use piperine_analog_solver::device::Ambient;
use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::ac::AcSweepAnalysisOptions;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use piperine_analog_solver::math::linear::{LinearSystem, Stamp, SymbolicLinearSystem, SymbolicMatrix};
use crate::prelude::DcAnalysisResult;
use crate::prelude::NoiseAnalysisResult;

use ndarray::Array1;
use num_complex::Complex;
use std::collections::HashMap;

// ── request/state ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseKind { Thermal, Shot, Flicker, Other }

pub struct Noise {
    pub terminals: (AnalogReference, AnalogReference),
    pub value: f64,
    pub name: Option<String>,
    pub kind: NoiseKind,
}

impl Noise {
    pub fn new(terminals: (AnalogReference, AnalogReference), value: f64) -> Self {
        Self { terminals, value, name: None, kind: NoiseKind::Other }
    }
    pub fn named(mut self, name: impl Into<String>, kind: NoiseKind) -> Self {
        self.name = Some(name.into()); self.kind = kind; self
    }
}

#[derive(Clone, Debug)]
pub struct NoiseAnalysisOptions {
    pub sweep_options: AcSweepAnalysisOptions,
    pub output_node: NodeIdentifier,
    pub reference_node: NodeIdentifier,
    pub input_source_name: Option<String>,
}


#[cfg(test)]
mod noise_tests {
    use super::*;

    #[test]
    fn noise_new_defaults() {
        let n1 = AnalogReference::ground();
        let n2 = AnalogReference::ground();
        let noise = Noise::new((n1.clone(), n2.clone()), 1.0);
        assert_eq!(noise.name, None);
        assert_eq!(noise.kind, NoiseKind::Other);
    }

    #[test]
    fn noise_named() {
        let n1 = AnalogReference::ground();
        let n2 = AnalogReference::ground();
        let noise = Noise::new((n1.clone(), n2.clone()), 1.0).named("rn", NoiseKind::Thermal);
        assert_eq!(noise.name, Some("rn".to_string()));
        assert_eq!(noise.kind, NoiseKind::Thermal);
    }
}

// ── driver ───────────────────────────────────────────────────────────────

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
    ) -> crate::core::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        let dc_point = DcSolver::under(circuit, context.clone(), piperine_abi::raw::global::NOISE)?.solve()?;

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
    pub fn solve(&mut self) -> crate::core::result::Result<NoiseAnalysisResult> {
        let frequencies = self.options.sweep_options.generate_frequencies();
        let mut out_noise_sq = Vec::with_capacity(frequencies.len());
        let mut psd_map: HashMap<(String, String), (NoiseKind, Vec<f64>)> = HashMap::new();

        for (i, &f) in frequencies.iter().enumerate() {
            let stamps = Self::assemble_linearized(self.circuit, &self.dc_point, f, &self.context)?;
            let adjoint_sol = self.solve_adjoint_system(stamps)?;

            // Output density `Σ |H·b|²·PSD` over the assembly's noise columns.
            let mut step_density = 0.0;
            let columns = self.device_columns(f, &adjoint_sol)?;
            for (element, source, kind, val) in columns {
                step_density += val;
                Self::record(&mut psd_map, (element, source), kind, val, i, frequencies.len());
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
            contributions.push(crate::core::result::NoiseContribution {
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

    /// Appends `val` to the PSD series of `key` at frequency index `i`.
    fn record(
        psd_map: &mut HashMap<(String, String), (NoiseKind, Vec<f64>)>,
        key: (String, String),
        kind: NoiseKind,
        val: f64,
        i: usize,
        count: usize,
    ) {
        let entry = psd_map.entry(key).or_insert_with(|| {
            let mut vec = Vec::with_capacity(count);
            vec.resize(i, 0.0);
            (kind, vec)
        });
        while entry.1.len() < i {
            entry.1.push(0.0);
        }
        entry.1.push(val);
    }

    /// The noise columns of a circuit of devices at `f_hz`, each with its
    /// contribution `|H·b|²·PSD` to the output: `(instance, input, kind, value)`.
    fn device_columns(
        &mut self,
        f_hz: f64,
        adjoint: &Array1<Complex<f64>>,
    ) -> crate::core::result::Result<Vec<(String, String, NoiseKind, f64)>> {
        let op = crate::analyses::ac::operating_vector(self.circuit, &self.dc_point);
        let amb = Ambient {
            time: 0.0,
            freq: f_hz,
            is_static: false,
            gmin: self.context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = self.circuit.abi.as_mut().expect("a circuit of devices");
        let mut out = Vec::new();
        for column in bound.analog.noise_sources(&op, amb)? {
            let h_b = column.rows.iter().fold(Complex::new(0.0, 0.0), |sum, (r, b)| {
                sum + r.idx().map_or(Complex::new(0.0, 0.0), |i| adjoint[i]) * *b
            });
            let (element, source) = column.name.rsplit_once('.').unwrap_or(("", column.name.as_str()));
            let kind = match source {
                s if s.contains("thermal") => NoiseKind::Thermal,
                s if s.contains("shot") => NoiseKind::Shot,
                s if s.contains("flicker") => NoiseKind::Flicker,
                _ => NoiseKind::Other,
            };
            out.push((element.to_string(), source.to_string(), kind, h_b.norm_sqr() * column.psd));
        }
        Ok(out)
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
    ) -> crate::core::result::Result<Array1<Complex<f64>>> {
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
    /// Uses the DC operating point to get the small-signal matrix of the
    /// device assembly.
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
    ) -> crate::core::result::Result<Vec<Stamp<AnalogReference, Complex<f64>>>> {
        let op = crate::analyses::ac::operating_vector(circuit, dc_point);
        let amb = Ambient {
            time: 0.0,
            freq: f_hz,
            is_static: false,
            gmin: context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = circuit.abi.as_mut().expect("a circuit of devices");
        Ok(bound.analog.assemble_ac(&op, amb)?.0)
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
    ) -> crate::core::result::Result<(AnalogReference, AnalogReference)> {
        let net = circuit.netlist();
        let out = net
            .reference_for(&AnalogVariable::Node(opt.output_node.clone()))
            .ok_or_else(|| {
                piperine_analog_solver::error::Error::simple(piperine_analog_solver::error::SolverDomain::Noise, "Output node not found")
            })?;
        let ref_ = net
            .reference_for(&AnalogVariable::Node(opt.reference_node.clone()))
            .ok_or_else(|| {
                piperine_analog_solver::error::Error::simple(piperine_analog_solver::error::SolverDomain::Noise, "Reference node not found")
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
        piperine_analog_solver::math::integration::Integrator::trapezoid(freqs, psd).sqrt()
    }
}
