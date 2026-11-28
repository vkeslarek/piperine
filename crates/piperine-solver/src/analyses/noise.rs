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
use piperine_analog_solver::math::integration::Integrator;
use piperine_analog_solver::netlist::{AnalogReference, AnalogVariable, BranchIdentifier, NodeIdentifier};
use crate::analyses::Context;
use crate::analyses::ac::AcSweepAnalysisOptions;
use crate::analyses::dc::DcSolver;
use crate::core::circuit::CircuitInstance;
use piperine_analog_solver::math::faer::{FaerSparseLinearSystem, FaerSymbolicMatrix};
use piperine_analog_solver::math::linear::Stamp;
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
    /// The path of the instance whose `STIM` inputs are the analysis input: with it the result
    /// carries the input-referred noise (`inoise`). An instance with no `STIM` input is refused
    /// when the solver is built.
    pub input_source_name: Option<String>,
    /// A flow output: when present it replaces the `output_node`/`reference_node` pair, and
    /// `onoise` reads in A/√Hz.
    pub output_flow: Option<FlowOutput>,
}

/// A flow (current) output of the noise analysis.
#[derive(Clone, Debug, PartialEq)]
pub enum FlowOutput {
    /// The current of a branch unknown (a potential contribution's private flow pin): the
    /// adjoint's unit sits at that unknown.
    Branch(BranchIdentifier),
    /// The value of one term of an instance, for a flow contribution with no potential branch:
    /// the term's linearization is the adjoint's right side, and its direct partials against
    /// the noise inputs add to each source's transfer.
    Term {
        /// The path of the instance.
        instance: String,
        /// The name of the term.
        term: String,
    },
}

/// The output of a noise run, resolved against the circuit.
enum OutputTarget {
    /// `v(out) − v(reference)`: a unit at `out`, its negative at `reference`.
    Potential { out: AnalogReference, reference: AnalogReference },
    /// The current of a branch unknown.
    Branch(AnalogReference),
    /// A term's value.
    Term { instance: String, term: String },
}

/// The output as a linear functional of the small-signal solution at one frequency: it reads
/// `Σ_j u_j·x_j + Σ_k h_k·n_k` over the unknowns `x` and the noise inputs `n` of its instance.
struct OutputFunctional {
    /// The coefficient of each unknown: the adjoint system's right side.
    unknowns: Vec<(AnalogReference, Complex<f64>)>,
    /// The instance whose noise inputs the output reads directly, and the coefficient of each
    /// input by position (empty unless the output is a term).
    direct: Option<(String, Vec<(usize, Complex<f64>)>)>,
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
    output: OutputTarget,
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
        Self::observed_by(circuit, options, context, Default::default())
    }

    /// As [`new`](Self::new), the operating point the noise is taken about delivered to the host's
    /// sink `host` (every equilibrium solve writes its trace records, analog-operators AO-09 AC 1).
    pub(crate) fn observed_by(
        circuit: &'a mut CircuitInstance,
        options: NoiseAnalysisOptions,
        context: Context,
        host: crate::analyses::adapters::HostHooks,
    ) -> crate::core::result::Result<Self> {
        Context::init_global();
        circuit.setup_all(&context)?;

        let mut dc_solver = DcSolver::under(circuit, context.clone(), piperine_abi::raw::global::NOISE)?;
        dc_solver.host = host;
        let dc_point = dc_solver.solve()?;

        let output = Self::resolve_output(circuit, &dc_point, &options, &context)?;
        if let Some(path) = &options.input_source_name {
            Self::require_stimulus(circuit, &dc_point, path, &options, &context)?;
        }
        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);

        let symbolic_stamps = Self::assemble_linearized(circuit, &dc_point, 1.0, &context)?;
        let symbolic_matrix = FaerSymbolicMatrix::new(size, symbolic_stamps)?;

        Ok(Self {
            circuit,
            context,
            dc_point,
            symbolic_matrix,
            options,
            output,
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
        // |H_in|² at each frequency, with an input: the power gain from it to the output.
        let mut input_gain_squared = Vec::with_capacity(frequencies.len());

        for (i, &f) in frequencies.iter().enumerate() {
            let stamps = Self::assemble_linearized(self.circuit, &self.dc_point, f, &self.context)?;
            let functional = self.output_functional(f)?;
            let adjoint_sol = self.solve_adjoint_system(stamps, &functional)?;

            // Output density `Σ |H·b|²·PSD` over the assembly's noise columns.
            let mut step_density = 0.0;
            let columns = self.device_columns(f, &adjoint_sol, &functional)?;
            for (element, source, kind, val) in columns {
                step_density += val;
                Self::record(&mut psd_map, (element, source), kind, val, i, frequencies.len());
            }
            out_noise_sq.push(step_density);
            if self.options.input_source_name.is_some() {
                input_gain_squared.push(self.input_gain_squared(f, &adjoint_sol)?);
            }
            for (_, entry) in psd_map.iter_mut() {
                if entry.1.len() == i {
                    entry.1.push(0.0);
                }
            }
        }

        let integrated_noise = self.integrate_noise(&frequencies, &out_noise_sq);

        let (contributions, output_power, input_power) = Self::contributions(&frequencies, psd_map, &input_gain_squared);

        Ok(NoiseAnalysisResult {
            frequencies,
            integrated_noise,
            output_total: output_power.sqrt(),
            input_noise_sq: (!input_gain_squared.is_empty())
                .then(|| out_noise_sq.iter().zip(&input_gain_squared).map(|(n, gain)| n / gain).collect()),
            input_total: (!input_gain_squared.is_empty()).then(|| input_power.sqrt()),
            out_noise_sq,
            contributions,
        })
    }

    /// The contribution of each noise source (its band-integrated power and its density), and the band powers of the
    /// output and, with an input (`input_gain_squared` not empty), of the input-referred noise.
    fn contributions(
        frequencies: &[f64],
        psd_map: HashMap<(String, String), (NoiseKind, Vec<f64>)>,
        input_gain_squared: &[f64],
    ) -> (Vec<crate::core::result::NoiseContribution>, f64, f64) {
        let mut contributions = Vec::new();
        let (mut output_power, mut input_power) = (0.0, 0.0);
        for ((element, source), (kind, psd)) in psd_map {
            output_power += Integrator::power_law(frequencies, &psd);
            if !input_gain_squared.is_empty() {
                let referred: Vec<f64> = psd.iter().zip(input_gain_squared).map(|(n, gain)| n / gain).collect();
                input_power += Integrator::power_law(frequencies, &referred);
            }
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
        (contributions, output_power, input_power)
    }

    /// Adds `val` to the PSD series of `key` at frequency index `i`: a second input of the same
    /// key at one frequency (two inputs sharing a name) sums into that value, so a series holds
    /// one value per frequency and stays aligned with the sweep (design D7, H2).
    fn record(
        psd_map: &mut HashMap<(String, String), (NoiseKind, Vec<f64>)>,
        key: (String, String),
        kind: NoiseKind,
        val: f64,
        i: usize,
        count: usize,
    ) {
        let entry = psd_map.entry(key).or_insert_with(|| (kind, Vec::with_capacity(count)));
        while entry.1.len() < i {
            entry.1.push(0.0);
        }
        match entry.1.get_mut(i) {
            Some(already) => *already += val,
            None => entry.1.push(val),
        }
    }

    /// The noise columns of a circuit of devices at `f_hz`, each with its
    /// contribution `|T|²·PSD` to the output: `(instance, input, kind, value)`, where the
    /// transfer is `T = h − z·b` for the adjoint solution `z` and the injection `b` of the
    /// column (a noise injection enters the system as `A·x = −b·n`, design D10), and `h` is the
    /// output's direct coefficient on that input (nonzero only for a term output).
    fn device_columns(
        &mut self,
        f_hz: f64,
        adjoint: &Array1<Complex<f64>>,
        functional: &OutputFunctional,
    ) -> crate::core::result::Result<Vec<(String, String, NoiseKind, f64)>> {
        let op = crate::analyses::ac::AcSystem::operating_vector(self.circuit, &self.dc_point);
        let amb = Ambient {
            time: 0.0,
            freq: f_hz,
            is_static: false,
            gmin: self.context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = &mut self.circuit.abi;
        let mut out = Vec::new();
        for column in bound.analog.noise_sources(&op, amb)? {
            let through_circuit = column.rows.iter().fold(Complex::new(0.0, 0.0), |sum, (r, b)| {
                sum + r.idx().map_or(Complex::new(0.0, 0.0), |i| adjoint[i]) * *b
            });
            let direct = functional
                .direct
                .as_ref()
                .filter(|(instance, _)| *instance == column.instance)
                .and_then(|(_, inputs)| inputs.iter().find(|(input, _)| *input == column.input))
                .map_or(Complex::new(0.0, 0.0), |(_, h)| *h);
            let transfer = direct - through_circuit;
            let (element, source) = column.name.rsplit_once('.').unwrap_or(("", column.name.as_str()));
            let kind = match source {
                s if s.contains("thermal") => NoiseKind::Thermal,
                s if s.contains("shot") => NoiseKind::Shot,
                s if s.contains("flicker") => NoiseKind::Flicker,
                _ => NoiseKind::Other,
            };
            out.push((element.to_string(), source.to_string(), kind, transfer.norm_sqr() * column.psd));
        }
        Ok(out)
    }

    /// The operating point as the assembly's solution vector, and the ambient at `frequency`.
    fn small_signal_point(circuit: &CircuitInstance, dc_point: &DcAnalysisResult, frequency: f64, context: &Context) -> (Vec<f64>, Ambient) {
        let operating_point = crate::analyses::ac::AcSystem::operating_vector(circuit, dc_point);
        let ambient = Ambient { time: 0.0, freq: frequency, is_static: false, gmin: context.tolerances.gmin, source_scale: 1.0 };
        (operating_point, ambient)
    }

    /// Refuses an input instance that has no `STIM` input, naming it (a noise run's input must
    /// stimulate something: its transfer to the output is what `inoise` divides by).
    fn require_stimulus(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        path: &str,
        options: &NoiseAnalysisOptions,
        context: &Context,
    ) -> crate::core::result::Result<()> {
        let (operating_point, ambient) = Self::small_signal_point(circuit, dc_point, options.sweep_options.start_frequency, context);
        let bound = &mut circuit.abi;
        if bound.analog.stimulus_columns(&operating_point, ambient, path)?.is_empty() {
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Noise,
                format!("input `{path}` has no small-signal stimulus (`ac_stim`): there is no transfer to refer the noise to"),
            ));
        }
        Ok(())
    }

    /// `|H_in|²` at `frequency`: the power gain from the input instance to the output, with the
    /// adjoint solution `z` of the output. Each `STIM` input `s` of the instance excites at unit
    /// magnitude and its own phase `φ_s` (design D11), so `H_in = Σ_s (z·g_s)·e^{jφ_s}`. An
    /// input with no transfer to the output has no input-referred noise: it fails naming the
    /// input and the frequency.
    fn input_gain_squared(&mut self, frequency: f64, adjoint: &Array1<Complex<f64>>) -> crate::core::result::Result<f64> {
        let path = self.options.input_source_name.clone().expect("an input instance");
        let (operating_point, ambient) = Self::small_signal_point(self.circuit, &self.dc_point, frequency, &self.context);
        let bound = &mut self.circuit.abi;
        let mut transfer = Complex::new(0.0, 0.0);
        for column in bound.analog.stimulus_columns(&operating_point, ambient, &path)? {
            let from_column = column.rows.iter().fold(Complex::new(0.0, 0.0), |sum, (r, g)| {
                sum + r.idx().map_or(Complex::new(0.0, 0.0), |i| adjoint[i]) * *g
            });
            transfer += from_column * Complex::from_polar(1.0, column.phase);
        }
        let gain_sq = transfer.norm_sqr();
        if gain_sq == 0.0 {
            return Err(piperine_analog_solver::error::Error::simple(
                piperine_analog_solver::error::SolverDomain::Noise,
                format!("input `{path}` has no transfer to the output at {frequency:e} Hz"),
            ));
        }
        Ok(gain_sq)
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
    /// 2. Apply the output functional as the right side: I_out = +1, I_ref = -1 for a node
    ///    pair, a unit at a branch's flow unknown, the term's partials for a term output
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
        functional: &OutputFunctional,
    ) -> crate::core::result::Result<Array1<Complex<f64>>> {
        let mut system = FaerSparseLinearSystem::new(self.symbolic_matrix.size());

        for stamp in stamps {
            if let Stamp::Matrix(r, c, val) = stamp {
                system.apply_stamps(vec![Stamp::Matrix(c, r, val)]);
            }
        }

        system.apply_stamps(functional.unknowns.iter().map(|(r, u)| Stamp::Rhs(r.clone(), *u)).collect());

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
        let op = crate::analyses::ac::AcSystem::operating_vector(circuit, dc_point);
        let amb = Ambient {
            time: 0.0,
            freq: f_hz,
            is_static: false,
            gmin: context.tolerances.gmin,
            source_scale: 1.0,
        };
        let bound = &mut circuit.abi;
        Ok(bound.analog.assemble_ac(&op, amb)?.0)
    }

    /// Resolves the options' output against the circuit: a flow output when `output_flow` is
    /// given (a branch must be in the netlist, a term must be in its instance: both are refused
    /// by name), else the `output_node`/`reference_node` pair.
    fn resolve_output(
        circuit: &mut CircuitInstance,
        dc_point: &DcAnalysisResult,
        options: &NoiseAnalysisOptions,
        context: &Context,
    ) -> crate::core::result::Result<OutputTarget> {
        let refuse = |detail: String| {
            piperine_analog_solver::error::Error::simple(piperine_analog_solver::error::SolverDomain::Noise, detail)
        };
        match &options.output_flow {
            None => {
                let (out, reference) = Self::resolve_nodes(circuit, options)?;
                Ok(OutputTarget::Potential { out, reference })
            }
            Some(FlowOutput::Branch(branch)) => circuit
                .netlist()
                .reference_for(&AnalogVariable::Branch(branch.clone()))
                .map(|reference| OutputTarget::Branch(reference.clone()))
                .ok_or_else(|| refuse(format!("output branch `{branch:?}` not found"))),
            Some(FlowOutput::Term { instance, term }) => {
                let (operating_point, ambient) = Self::small_signal_point(circuit, dc_point, options.sweep_options.start_frequency, context);
                let bound = &mut circuit.abi;
                bound.analog.term_linearization(&operating_point, ambient, instance, term)?;
                Ok(OutputTarget::Term { instance: instance.clone(), term: term.clone() })
            }
        }
    }

    /// The output as a functional of the solution at `frequency`.
    fn output_functional(&mut self, frequency: f64) -> crate::core::result::Result<OutputFunctional> {
        match &self.output {
            OutputTarget::Potential { out, reference } => Ok(OutputFunctional {
                unknowns: vec![(out.clone(), Complex::new(1.0, 0.0)), (reference.clone(), Complex::new(-1.0, 0.0))],
                direct: None,
            }),
            OutputTarget::Branch(branch) => {
                Ok(OutputFunctional { unknowns: vec![(branch.clone(), Complex::new(1.0, 0.0))], direct: None })
            }
            OutputTarget::Term { instance, term } => {
                let (instance, term) = (instance.clone(), term.clone());
                let (operating_point, ambient) = Self::small_signal_point(self.circuit, &self.dc_point, frequency, &self.context);
                let bound = &mut self.circuit.abi;
                let linearization = bound.analog.term_linearization(&operating_point, ambient, &instance, &term)?;
                Ok(OutputFunctional { unknowns: linearization.unknowns, direct: Some((instance, linearization.noise)) })
            }
        }
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
    /// This is the trapezoid over the grid; the band total ngspice reports is
    /// [`NoiseAnalysisResult::output_total`], by the power-law rule.
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
        Integrator::trapezoid(freqs, psd).sqrt()
    }
}
