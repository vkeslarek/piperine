use crate::error::Error;
use crate::analog::{BranchIdentifier, AnalogVariable, NodeIdentifier};
use crate::core::net::Net;
use crate::digital::LogicValue;
use crate::math::unit::Hertz;
use num_complex::Complex;
use std::collections::HashMap;
use std::slice::Iter;
use std::sync::Arc;
use crate::analysis::noise::NoiseKind;

pub type Result<T> = std::result::Result<T, Error>;

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
    pub dt_min_floor_hits: usize,
    pub dt_min: f64,
    pub dt_max: f64,
    // Device-level
    pub bypass_hits: usize,
    pub bypass_misses: usize,
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
    /// Snapshot of every digital net's logic value at this step, indexed by
    /// `DigitalNet` id. Lets a transient trace read sequential logic over time
    /// (`Trace.v(bit_net)` → 0/1/NaN), which `$op` cannot express (it is a
    /// stateless operating point).
    digital: Vec<LogicValue>,
}

impl TransientStep {
    pub fn new(time: f64, values: HashMap<Arc<AnalogVariable>, f64>) -> Self {
        Self { time, values, digital: Vec::new() }
    }

    /// Attach a digital-net snapshot (by `DigitalNet` id).
    pub fn with_digital(mut self, digital: Vec<LogicValue>) -> Self {
        self.digital = digital;
        self
    }

    /// This step's logic value for digital net `idx`, or `None` if unrecorded.
    pub fn digital(&self, idx: usize) -> Option<LogicValue> {
        self.digital.get(idx).copied()
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

    /// Read the digital logic value by [`Net`]. Returns `None` for analog
    /// and pseudo nets, or for digital nets that were not recorded this
    /// step.
    pub fn digital_net(&self, net: &Net) -> Option<LogicValue> {
        if !matches!(net.kind(), crate::core::net::NetKind::Digital) {
            return None;
        }
        let idx = net.dense()?;
        self.digital.get(idx).copied()
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
}

impl AcAnalysisResult {
    pub fn new(values: Vec<AcAnalysisStep>) -> Self {
        Self {
            values,
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
    pub frequency: Hertz,
    values: HashMap<Arc<AnalogVariable>, Complex<f64>>,
}

impl AcAnalysisStep {
    pub fn new(frequency: Hertz, values: HashMap<Arc<AnalogVariable>, Complex<f64>>) -> Self {
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
