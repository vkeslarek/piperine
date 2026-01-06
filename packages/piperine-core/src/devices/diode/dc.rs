use crate::analysis::dc::DcAnalysis;
use crate::devices::diode::Diode;
use crate::math::linear::Stamp;
use crate::netlist::CircuitReference;
use crate::solver::Context;

impl DcAnalysis for Diode {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        let g_leak = 1e-9; // 1 Giga-Ohm leakage path (Conductance)

        vec![
            // Place the fixed conductance in the matrix
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_leak),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_leak),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_leak),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_leak),
            // NO RHS: Passive components have no current source I_eq in a fixed-G model.
            // This effectively sets the initial guess for the diode current to 0.
        ]
    }
}
