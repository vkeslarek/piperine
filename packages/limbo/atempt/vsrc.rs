use crate::{
    Analysis, BranchReference, CircuitInstance, ComponentInstance, Device, NodeIdentifier,
    NodeReference, PiperineResult, RealStamper, TransientAnalysisContext,
};
use std::sync::Arc;

pub struct VoltageSource;

impl Device for VoltageSource {
    type ComponentInstance = VoltageSourceInstance;
    const NAME: &'static str = "VoltageSource";
    const DESCRIPTION: &'static str = "Independent voltage source";
    const PINS: &'static [&'static str] = &["V+", "V-"];
    const AVAILABLE_ANALYSIS: &'static [Analysis] = &[Analysis::OP];
}

pub struct VoltageSourceInstance {
    pub n_plus: Arc<NodeReference>,
    pub n_minus: Arc<NodeReference>,
    pub branch: Arc<BranchReference>, // The MNA current variable
    pub value: f64,
}

pub struct VoltageSourceParameters {
    pub name: String,
    pub n_plus: NodeIdentifier,
    pub n_minus: NodeIdentifier,
    pub value: f64,
}

impl Default for VoltageSourceParameters {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            n_plus: NodeIdentifier::Gnd,
            n_minus: NodeIdentifier::Gnd,
            value: 0.0,
        }
    }
}

impl ComponentInstance for VoltageSourceInstance {
    type ComponentParameters = VoltageSourceParameters;

    fn setup(params: Self::ComponentParameters, circ: &CircuitInstance) -> PiperineResult<Self> {
        let n_plus = circ.get_node_reference(params.n_plus)?;
        let n_minus = circ.get_node_reference(params.n_minus)?;

        // This is where your BranchDevice logic lives:
        // The source asks the circuit for a new matrix index for its current.
        let branch = circ.get_branch_reference()?;

        Ok(Self {
            n_plus,
            n_minus,
            branch,
            value: params.value,
        })
    }

    fn temperature(&mut self) {} // Ideal sources don't change with temp

    fn load_dc(&self, _: &CircuitInstance, _: &TransientAnalysisContext, stamp: &mut dyn RealStamper) {
        // 1. Current leaving/entering nodes (KCL)
        stamp.node_to_branch_stamp(&self.n_plus, &self.branch, 1.0);
        stamp.node_to_branch_stamp(&self.n_minus, &self.branch, -1.0);

        // 2. The Voltage Equation: V+ - V- = Value
        stamp.branch_to_node_stamp(&self.branch, &self.n_plus, 1.0);
        stamp.branch_to_node_stamp(&self.branch, &self.n_minus, -1.0);

        // 3. The target voltage goes into the RHS
        stamp.branch_rhs_stamp(&self.branch, self.value);
    }
}
