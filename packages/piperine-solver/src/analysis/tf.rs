use crate::circuit::netlist::{BranchIdentifier, CircuitVariable, NodeIdentifier};

/// Transfer Function analysis options.
///
/// Specifies the input source and output variable for transfer function analysis.
/// Transfer function analysis calculates DC small-signal transfer characteristics:
/// - **Gain:** dOutput/dInput
/// - **Input Resistance:** Resistance seen by the input source
/// - **Output Resistance:** Thévenin/Norton equivalent resistance at output
#[derive(Clone, Debug)]
pub struct TransferFunctionAnalysisOptions {
    /// Output variable to measure.
    ///
    /// Can be:
    /// - `CircuitVariable::Node(n)` for voltage at node n (referenced to GND)
    /// - `CircuitVariable::Branch(b)` for current through branch b
    pub output: CircuitVariable,

    /// Reference node for differential voltage measurement.
    ///
    /// - `None`: Single-ended measurement V(output) with implicit GND reference
    /// - `Some(n)`: Differential measurement V(output, n) = V(output) - V(n)
    ///
    /// Only used when output is a Node. Ignored for Branch outputs.
    pub output_ref: Option<NodeIdentifier>,

    /// Input source branch identifier.
    ///
    /// Identifies the voltage or current source to use as input.
    /// Examples:
    /// - `BranchIdentifier::from_component("V1")` for voltage source V1
    /// - `BranchIdentifier::from_component("I1")` for current source I1
    pub input_source: BranchIdentifier,
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
