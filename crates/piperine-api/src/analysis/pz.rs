use crate::node::Node;
use crate::options::SolverOptions;
use crate::spice::{Measurement, SpiceAnalysis};
use super::{emit_common, emit_meas};

/// Pole-Zero transfer function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PzTransferType {
    /// Transfer function of type (output voltage) / (input current).
    Current,
    /// Transfer function of type (output voltage) / (input voltage).
    Voltage,
}

impl PzTransferType {
    pub fn to_spice(&self) -> &'static str {
        match self {
            PzTransferType::Current => "cur",
            PzTransferType::Voltage => "vol",
        }
    }
}

/// Pole-Zero analysis mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PzAnalysisMode {
    Poles,
    Zeros,
    Both,
}

impl PzAnalysisMode {
    pub fn to_spice(&self) -> &'static str {
        match self {
            PzAnalysisMode::Poles => "pol",
            PzAnalysisMode::Zeros => "zer",
            PzAnalysisMode::Both => "pz",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoleZeroAnalysis {
    pub input_pos: Node,
    pub input_neg: Node,
    pub output_pos: Node,
    pub output_neg: Node,
    pub transfer_type: PzTransferType,
    pub analysis_mode: PzAnalysisMode,
    pub solver: SolverOptions,
    pub measurements: Vec<Measurement>,
    pub nodesets: Vec<(Node, f64)>,
}

impl PoleZeroAnalysis {
    pub fn new(
        input_pos: Node,
        input_neg: Node,
        output_pos: Node,
        output_neg: Node,
        transfer_type: PzTransferType,
        analysis_mode: PzAnalysisMode,
    ) -> Self {
        Self {
            input_pos,
            input_neg,
            output_pos,
            output_neg,
            transfer_type,
            analysis_mode,
            solver: SolverOptions::default(),
            measurements: Vec::new(),
            nodesets: Vec::new(),
        }
    }
}

crate::impl_solver_options!(PoleZeroAnalysis);
crate::impl_analysis_common!(PoleZeroAnalysis);

impl SpiceAnalysis for PoleZeroAnalysis {
    fn to_spice_control_commands(&self) -> Vec<String> {
        let mut lines = Vec::new();
        emit_common(&self.solver, "", &self.nodesets, &mut lines);
        lines.push(format!(
            "pz {} {} {} {} {} {}",
            self.input_pos,
            self.input_neg,
            self.output_pos,
            self.output_neg,
            self.transfer_type.to_spice(),
            self.analysis_mode.to_spice(),
        ));
        emit_meas(&self.measurements, "pz", &mut lines);
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pz_voltage_both() {
        let cmds = PoleZeroAnalysis::new(
            Node::from("in"), Node::GROUND, Node::from("out"), Node::GROUND,
            PzTransferType::Voltage, PzAnalysisMode::Both,
        )
        .to_spice_control_commands();
        assert_eq!(cmds, vec!["pz in 0 out 0 vol pz"]);
    }

    #[test]
    fn pz_current_poles() {
        let cmds = PoleZeroAnalysis::new(
            Node::from("in"), Node::GROUND, Node::from("out"), Node::GROUND,
            PzTransferType::Current, PzAnalysisMode::Poles,
        )
        .to_spice_control_commands();
        assert_eq!(cmds, vec!["pz in 0 out 0 cur pol"]);
    }

    #[test]
    fn pz_zeros_only() {
        let cmds = PoleZeroAnalysis::new(
            Node::from("a"), Node::from("b"), Node::from("c"), Node::from("d"),
            PzTransferType::Voltage, PzAnalysisMode::Zeros,
        )
        .to_spice_control_commands();
        assert_eq!(cmds, vec!["pz a b c d vol zer"]);
    }
}
