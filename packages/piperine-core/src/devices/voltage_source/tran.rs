use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::devices::voltage_source::VoltageSource;
use crate::math::linear::Stamp;
use crate::math::unit::UnitExt;
use crate::netlist::CircuitReference;
use crate::solver::Context;
use crate::state::CircuitState;

impl TransientAnalysis for VoltageSource {
    fn update_transient(
        &mut self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> crate::error::Result<()> {
        let t = transient_analysis_context.time;

        // Configuration: 5V Amplitude, 500Hz Frequency
        let amplitude = 5.0.V();
        let frequency = 500.0.Hz(); // 1 / 0.002

        // V(t) = A * sin(2 * pi * f * t)
        let omega = 2.0 * std::f64::consts::PI * frequency;
        self.voltage = amplitude * (omega * t).value.sin();
        Ok(())
    }

    fn load_transient(
        &self,
        _: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            // Stamp the voltage source into the matrix (MNA format)
            // Branch current enters node_plus, leaves node_minus
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            // Constraint equation: V_plus - V_minus = Voltage
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            // RHS: The value of the voltage source at time t
            Stamp::Rhs(self.branch.clone(), self.voltage.value),
        ]
    }
}
