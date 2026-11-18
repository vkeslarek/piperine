//! Analysis result containers handed back to hosts (waveforms, operating points).
use piperine_analog_solver::netlist::{BranchIdentifier, AnalogVariable, NodeIdentifier};
use crate::core::net::Net;
use ndarray::Array2;
use num_complex::Complex;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;
use crate::analyses::noise::NoiseKind;

pub use piperine_analog_solver::error::Result;

/// Per-analysis convergence + performance diagnostics. Accumulated during
/// the solve; returned on the result type. Always-on (counter increments
/// are negligible); `Default::default()` zeroes everything for analyses
/// that haven't been instrumented yet.
#[derive(Debug, Clone, Default)]
pub struct SolverStats {
    // Newton (DC + each transient step's inner loop)
    pub newton_iterations: usize,
    pub converged: bool,
    // Transient step loop
    pub steps_accepted: usize,
    pub steps_rejected: usize,
    /// Steps retaken at `dt_min` after a larger attempt failed.
    pub dt_min_floor_hits: usize,
    /// Steps accepted at `dt_min` although they fail the truncation test (nothing smaller is
    /// allowed, and ngspice does not fail those either).
    pub floor_accepts: usize,
    pub dt_min: f64,
    pub dt_max: f64,
    // Homotopy / convergence strategy
    pub homotopy_strategy: Option<String>,
    pub homotopy_levels: usize,
    // Timing (nanoseconds)
    pub assembly_time_ns: u64,
    pub solve_time_ns: u64,
}

#[derive(Debug)]
pub struct DcAnalysisResult {
    values: HashMap<Arc<AnalogVariable>, f64>,
    pub stats: SolverStats,
}

impl DcAnalysisResult {
    pub fn new(
        values: HashMap<Arc<AnalogVariable>, f64>,
    ) -> Self {
        Self {
            values,
            stats: SolverStats::default(),
        }
    }

    /// Replace the default (zeroed) stats with populated values.
    pub fn set_stats(&mut self, stats: SolverStats) {
        self.stats = stats;
    }
    pub fn get(&self, variable: impl Into<Arc<AnalogVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<f64> {
        self.get(AnalogVariable::Node(node_identifier.clone()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(AnalogVariable::Branch(branch_identifier.into()))
    }

    pub fn values(&self) -> &HashMap<Arc<AnalogVariable>, f64> {
        &self.values
    }

    /// Read the solved value by [`Net`] — the unified naming layer used by
    /// hosts, diagnostics, and result mappers. Returns `None` for any net
    /// the result does not cover (pseudo nets like ground, or unmapped
    /// digital nets — those live on a separate path).
    pub fn get_net(&self, net: &Net) -> Option<f64> {
        let var = net.analog_variable()?;
        self.values.get(var).copied()
    }
}

#[derive(Debug, Clone)]
pub struct TransientAnalysisResult {
    values: Vec<TransientStep>,
    pub stats: SolverStats,
}

impl TransientAnalysisResult {
    pub fn new(values: Vec<TransientStep>) -> Self {
        Self {
            values,
            stats: SolverStats::default(),
        }
    }

    /// Replace the default (zeroed) stats with populated values.
    pub fn set_stats(&mut self, stats: SolverStats) {
        self.stats = stats;
    }

    pub fn push(&mut self, step: TransientStep) {
        self.values.push(step)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&TransientStep> {
        assert!(index < self.values.len());

        self.values.get(index)
    }

    pub fn last(&self) -> Option<&TransientStep> {
        self.values.last()
    }

    pub fn iter(&self) -> Iter<'_, TransientStep> {
        self.values.iter()
    }
}

#[derive(Debug, Clone)]
pub struct TransientStep {
    time: f64,
    values: HashMap<Arc<AnalogVariable>, f64>,
    /// Opt-in per-step device runtime banks (state, vars), keyed by device
    /// label. Recorded only when the run enables
    /// `TransientAnalysisOptions::record_device_state`; lets a host recompute
    /// a stateful device's branch current at this step (`Trace.i`).
    device_state: HashMap<String, (Vec<f64>, Vec<f64>)>,
    /// Every digital net's bytes at this step (after its instant ran), with
    /// the nets' names: lets a transient trace read sequential logic over
    /// time, which an operating point cannot express.
    digital: Option<Arc<DigitalNets>>,
    /// For a circuit of devices, the state a transient re-enters from at this
    /// step: recorded on every accepted step of a run (a full circuit state).
    reentry: Option<Arc<ReentryState>>,
}

/// Every digital net's bytes at one step, by net id, with the nets' names
/// (the bytes are the net's layout: a `BIT` is `[value, known]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitalNets {
    names: Arc<[String]>,
    bytes: Vec<Vec<u8>>,
}

impl DigitalNets {
    /// `bytes[i]` is the net named `names[i]`.
    ///
    /// # Panics
    /// When the two lengths differ.
    pub fn new(names: Arc<[String]>, bytes: Vec<Vec<u8>>) -> Self {
        assert_eq!(names.len(), bytes.len(), "one name per net");
        Self { names, bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The bytes of the net with id `idx`.
    pub fn get(&self, idx: usize) -> Option<&[u8]> {
        self.bytes.get(idx).map(Vec::as_slice)
    }

    /// The bytes of the net named `name`.
    pub fn by_name(&self, name: &str) -> Option<&[u8]> {
        self.names.iter().position(|n| n == name).map(|i| self.bytes[i].as_slice())
    }

    /// The nets' names, by net id.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Every net as `(name, bytes)`, by net id.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.names.iter().map(String::as_str).zip(self.bytes.iter().map(Vec::as_slice))
    }

    pub(crate) fn bytes(&self) -> &[Vec<u8>] {
        &self.bytes
    }
}

/// What a transient of a circuit of devices re-enters from besides the
/// solution (abi.md §5 checkpoint): the discrete engine's checkpoint and every
/// net's bytes in it, the assembly's term histories and limit states, and the
/// step bound of the accepted solution.
#[derive(Debug, Clone)]
pub(crate) struct ReentryState {
    pub(crate) digital: piperine_digital_solver::engine::Checkpoint,
    /// Every net's bytes, by net id.
    pub(crate) nets: Arc<DigitalNets>,
    pub(crate) assembly: piperine_analog_solver::device::AssemblySnapshot,
    pub(crate) cap: f64,
    /// What the analog time loop carries from one step to the next.
    pub(crate) analog: AnalogResume,
}

/// The analog side's resume state: what a run re-entered at a captured step needs, besides the
/// solution, to take every later step the continuous run took (HYG-04). The Newton history the
/// predictor reads, the step size the controller proposed, the previous accepted step, the size of
/// the last accepted step, whether the last step was accepted, whether a restart is pending (the
/// step after a discontinuity is first order and exempt), and the controller's memory.
#[derive(Debug, Clone)]
pub(crate) struct AnalogResume {
    newton: piperine_analog_solver::math::circular_array::CircularArrayBuffer2<f64>,
    dt: f64,
    prev_h: f64,
    last_dt: f64,
    last_step_accepted: bool,
    restart_pending: bool,
    stepper: piperine_analog_solver::convergence::StepperMemory,
}

impl AnalogResume {
    pub(crate) fn new(
        newton: piperine_analog_solver::math::circular_array::CircularArrayBuffer2<f64>,
        dt: f64,
        prev_h: f64,
        last_dt: f64,
        last_step_accepted: bool,
        restart_pending: bool,
        stepper: piperine_analog_solver::convergence::StepperMemory,
    ) -> Self {
        Self { newton, dt, prev_h, last_dt, last_step_accepted, restart_pending, stepper }
    }

    pub(crate) fn newton(&self) -> &piperine_analog_solver::math::circular_array::CircularArrayBuffer2<f64> {
        &self.newton
    }

    pub(crate) fn dt(&self) -> f64 {
        self.dt
    }

    pub(crate) fn prev_h(&self) -> f64 {
        self.prev_h
    }

    pub(crate) fn last_dt(&self) -> f64 {
        self.last_dt
    }

    pub(crate) fn last_step_accepted(&self) -> bool {
        self.last_step_accepted
    }

    pub(crate) fn restart_pending(&self) -> bool {
        self.restart_pending
    }

    pub(crate) fn stepper(&self) -> &piperine_analog_solver::convergence::StepperMemory {
        &self.stepper
    }
}

impl TransientStep {
    pub fn new(time: f64, values: HashMap<Arc<AnalogVariable>, f64>) -> Self {
        Self {
            time,
            values,
            device_state: HashMap::new(),
            digital: None,
            reentry: None,
        }
    }

    /// Attach the step's digital net values.
    pub fn with_digital(mut self, digital: DigitalNets) -> Self {
        self.digital = Some(Arc::new(digital));
        self
    }

    /// Every digital net's bytes at this step, or `None` if unrecorded.
    pub fn digital_nets(&self) -> Option<&DigitalNets> {
        self.digital.as_deref()
    }

    /// This step's bytes of the digital net with id `idx`, or `None` if unrecorded.
    pub fn digital(&self, idx: usize) -> Option<&[u8]> {
        self.digital.as_ref()?.get(idx)
    }

    /// This step's bytes of the digital net named `name`, or `None` if unrecorded.
    pub fn digital_named(&self, name: &str) -> Option<&[u8]> {
        self.digital.as_ref()?.by_name(name)
    }

    /// Read the digital net's bytes by [`Net`]. Returns `None` for analog and
    /// pseudo nets, or for digital nets that were not recorded this step.
    pub fn digital_net(&self, net: &Net) -> Option<&[u8]> {
        if !matches!(net.kind(), crate::core::net::NetKind::Digital) {
            return None;
        }
        self.digital(net.dense()?)
    }

    /// The re-entry state of a circuit of devices, when this step carries one.
    pub(crate) fn reentry(&self) -> Option<&Arc<ReentryState>> {
        self.reentry.as_ref()
    }

    /// The re-entry state, and with it the step's digital net values.
    pub(crate) fn set_reentry(&mut self, state: Option<Arc<ReentryState>>) {
        if let Some(state) = &state {
            self.digital = Some(state.nets.clone());
        }
        self.reentry = state;
    }

    /// Attach the per-step device runtime banks (label → (state, vars)).
    pub fn with_device_state(mut self, device_state: HashMap<String, (Vec<f64>, Vec<f64>)>) -> Self {
        self.device_state = device_state;
        self
    }

    /// This step's recorded runtime banks (state, vars) for the device
    /// labelled `label`, or `None` when recording was off (or the device has
    /// no runtime banks).
    pub fn device_state(&self, label: &str) -> Option<&(Vec<f64>, Vec<f64>)> {
        self.device_state.get(label)
    }

    /// The labels of devices recorded into this step's `device_state`. Used
    /// by the selective-recording contract (ABI-34): a `ProbeSelection`
    /// requesting one device yields exactly that device's label here.
    pub fn device_state_keys(&self) -> impl Iterator<Item = &str> {
        self.device_state.keys().map(String::as_str)
    }

    pub fn get(&self, variable: impl Into<Arc<AnalogVariable>>) -> Option<f64> {
        self.values.get(&variable.into()).cloned()
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<f64> {
        self.get(AnalogVariable::Node(node_identifier.clone()))
    }

    pub fn get_branch(&self, branch_identifier: impl Into<BranchIdentifier>) -> Option<f64> {
        self.get(AnalogVariable::Branch(branch_identifier.into()))
    }

    /// Read the analog value by [`Net`] (the unified naming layer). Returns
    /// `None` for digital and pseudo nets.
    pub fn get_net(&self, net: &Net) -> Option<f64> {
        let var = net.analog_variable()?;
        self.values.get(var).copied()
    }

    pub fn values(&self) -> &HashMap<Arc<AnalogVariable>, f64> {
        &self.values
    }

    pub fn time(&self) -> f64 {
        self.time
    }
}

pub struct AcAnalysisResult {
    values: Vec<AcAnalysisStep>,
    /// Per-analysis statistics.
    pub stats: SolverStats,
}

impl AcAnalysisResult {
    pub fn new(values: Vec<AcAnalysisStep>) -> Self {
        Self {
            values,
            stats: SolverStats::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&AcAnalysisStep> {
        assert!(index < self.values.len());

        self.values.get(index)
    }

    pub fn iter(&self) -> Iter<'_, AcAnalysisStep> {
        self.values.iter()
    }
}

pub struct AcAnalysisStep {
    pub frequency: f64,
    values: HashMap<Arc<AnalogVariable>, Complex<f64>>,
}

impl AcAnalysisStep {
    pub fn new(frequency: f64, values: HashMap<Arc<AnalogVariable>, Complex<f64>>) -> Self {
        Self { frequency, values }
    }

    pub fn get(&self, circuit_var: &AnalogVariable) -> Option<&Complex<f64>> {
        self.values.get(circuit_var)
    }

    pub fn get_branch(
        &self,
        branch_identifier: impl Into<BranchIdentifier>,
    ) -> Option<&Complex<f64>> {
        self.get(&AnalogVariable::Branch(branch_identifier.into()))
    }

    pub fn get_node(&self, node_identifier: &NodeIdentifier) -> Option<&Complex<f64>> {
        self.get(&AnalogVariable::Node(node_identifier.clone()))
    }

    /// Read the small-signal value by [`Net`]. Returns `None` for digital
    /// and pseudo nets — those have no AC representation here.
    pub fn get_net(&self, net: &Net) -> Option<&Complex<f64>> {
        let var = net.analog_variable()?;
        self.values.get(var)
    }
}

#[derive(Debug, Clone)]
pub struct NoiseContribution {
    pub element: String,
    pub source: String,
    pub kind: NoiseKind,
    pub integrated_sq: f64,
    pub psd: Vec<f64>,
}

pub struct NoiseAnalysisResult {
    pub frequencies: Vec<f64>,
    pub out_noise_sq: Vec<f64>,
    pub integrated_noise: f64,
    pub contributions: Vec<NoiseContribution>,
}

impl NoiseAnalysisResult {
    pub fn contributions(&self) -> &[NoiseContribution] {
        &self.contributions
    }
}

/// Transfer Function analysis result.
///
/// Contains the three fundamental transfer function parameters calculated at the DC operating point.
#[derive(Clone, Debug)]
pub struct TransferFunctionAnalysisResult {
    /// Transfer function gain (dOutput/dInput).
    ///
    /// Units depend on input and output types:
    /// - **V→V:** Dimensionless (voltage gain)
    /// - **V→I:** Siemens (transconductance, g_m)
    /// - **I→V:** Ohms (transresistance, r_m)
    /// - **I→I:** Dimensionless (current gain, β)
    pub gain: f64,

    /// Input resistance seen by the source (Ohms).
    ///
    /// For voltage source: `R_in = V_source / I_source`
    /// For current source: `R_in = V_across / I_source`
    ///
    /// Represents the effective load on the input source.
    pub input_resistance: f64,

    /// Output resistance at the output terminals (Ohms).
    ///
    /// Thévenin equivalent resistance looking back into the circuit from the output.
    /// Calculated by applying a test perturbation at the output.
    pub output_resistance: f64,

    /// Type of transfer function for clarity and unit interpretation.
    pub tf_type: TransferType,
}

/// Classification of transfer function type based on input/output variables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferType {
    /// Voltage input → Voltage output (dimensionless gain)
    VoltageGain,

    /// Voltage input → Current output (Siemens, transconductance)
    Transconductance,

    /// Current input → Voltage output (Ohms, transresistance)
    Transresistance,

    /// Current input → Current output (dimensionless gain)
    CurrentGain,
}

/// Pole-zero analysis (`.pz`) result: the finite poles and zeros of the
/// linearized input→output transfer function, in rad/s — see
/// [`PoleZeroSolver`](crate::analyses::pz::PoleZeroSolver).
#[derive(Debug, Clone, Default)]
pub struct PoleZeroResult {
    pub poles: Vec<Complex<f64>>,
    pub zeros: Vec<Complex<f64>>,
}

/// N-port scattering-parameter analysis (`.sp`) result: the `S` matrix at
/// every swept frequency, in port order — see
/// [`SpSolver`](crate::analyses::sp::SpSolver).
#[derive(Debug, Clone, Default)]
pub struct SpResult {
    pub frequencies: Vec<f64>,
    /// `s[k]` is the `n_ports × n_ports` matrix at `frequencies[k]`,
    /// `s[k][[i, j]] = S_ij` (port `i` response / port `j` excitation).
    pub s: Vec<Array2<Complex<f64>>>,
    /// Reference impedance of each port, indexed by port position (not
    /// necessarily the declared `num`).
    pub z0: Vec<f64>,
    pub n_ports: usize,
}

impl std::fmt::Display for TransferType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferType::VoltageGain => write!(f, "Voltage Gain (V/V)"),
            TransferType::Transconductance => write!(f, "Transconductance (I/V, S)"),
            TransferType::Transresistance => write!(f, "Transresistance (V/I, Ω)"),
            TransferType::CurrentGain => write!(f, "Current Gain (I/I)"),
        }
    }
}
