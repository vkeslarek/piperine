use crate::analog::{BranchIdentifier, AnalogVariable, NodeIdentifier};

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
    /// - `AnalogVariable::Node(n)` for voltage at node n (referenced to GND)
    /// - `AnalogVariable::Branch(b)` for current through branch b
    pub output: AnalogVariable,

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



/// Per-analysis config for TF. Thin wrapper over the analysis options.
#[derive(Debug, Clone)]
pub struct TfContext {
    pub options: TransferFunctionAnalysisOptions,
}
