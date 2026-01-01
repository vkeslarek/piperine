use crate::{
    Analysis, CircuitInstance, ComponentInstance, Device, NodeIdentifier, NodeReference,
    PiperineResult, RealStamper, TransientAnalysisContext,
};
use std::sync::Arc;

pub struct Capacitor;

impl Device for Capacitor {
    type ComponentInstance = CapacitorInstance;
    const NAME: &'static str = "Capacitor";
    const DESCRIPTION: &'static str = "Linear capacitor";
    const PINS: &'static [&'static str] = &["C+", "C-"];
    const AVAILABLE_ANALYSIS: &'static [Analysis] = &[Analysis::OP];
}

pub struct CapacitorInstance {
    pub n_plus: Arc<NodeReference>,
    pub n_minus: Arc<NodeReference>,
    pub capacitance: f64,
}

pub struct CapacitorParameters {
    pub name: String,
    pub n_plus: NodeIdentifier,
    pub n_minus: NodeIdentifier,
    pub value: f64,
}

impl Default for CapacitorParameters {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            n_plus: NodeIdentifier::Gnd,
            n_minus: NodeIdentifier::Gnd,
            value: 0.0,
        }
    }
}

impl ComponentInstance for CapacitorInstance {
    type ComponentParameters = CapacitorParameters;

    fn setup(params: Self::ComponentParameters, circ: &CircuitInstance) -> PiperineResult<Self> {
        Ok(Self {
            n_plus: circ.get_node_reference(params.n_plus)?,
            n_minus: circ.get_node_reference(params.n_minus)?,
            capacitance: params.value,
        })
    }

    fn temperature(&mut self) {}

    fn load_dc(
        &self,
        circ: &CircuitInstance,
        ctx: &TransientAnalysisContext,
        stamp: &mut dyn RealStamper,
    ) {
        // // In DC Analysis, a capacitor is an OPEN circuit (0 conductance)
        // if ctx.is_dc() {
        //     return;
        // }

        // In Transient Analysis, we use the companion model (Backward Euler)
        let dt = circ.get_time_step();
        let g_eq = self.capacitance / dt;

        // Get the voltage from the previous converged timepoint
        let v_prev_plus = circ.get_history_voltage(&self.n_plus, 1);
        let v_prev_minus = circ.get_history_voltage(&self.n_minus, 1);
        let i_eq = g_eq * (v_prev_plus - v_prev_minus);

        // Stamp equivalent conductance (like a resistor)
        stamp.nodal_stamp(&self.n_plus, &self.n_plus, g_eq);
        stamp.nodal_stamp(&self.n_minus, &self.n_minus, g_eq);
        stamp.nodal_stamp(&self.n_plus, &self.n_minus, -g_eq);
        stamp.nodal_stamp(&self.n_minus, &self.n_plus, -g_eq);

        // Stamp equivalent current source into RHS
        stamp.nodal_rhs_stamp(&self.n_plus, i_eq);
        stamp.nodal_rhs_stamp(&self.n_minus, -i_eq);
    }
}
