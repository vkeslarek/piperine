use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::circuit::netlist::{CircuitReference, CircuitVariable};
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::math::linear::{Stamp, Stamp2};
use crate::solver::Context;
use std::f64::consts::PI;

impl TransientAnalysis for VoltageSource {
    fn update_transient(
        &mut self,
        _: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        _: &Context,
    ) -> crate::result::Result<()> {
        let voltage = match self.waveform {
            Waveform::DC(value) => value,
            Waveform::Sine {
                amplitude,
                frequency,
                phase,
            } => {
                let t = transient_analysis_context.time;
                let omega = 2.0 * PI * frequency;

                amplitude * (omega * t + phase).sin()
            }
            Waveform::Step {
                initial,
                final_value,
                delay,
                rise_time,
            } => {
                let t = transient_analysis_context.time;
                if t < delay {
                    initial
                } else if t >= delay && t < delay + rise_time {
                    let slope = (final_value - initial) / rise_time;
                    initial + slope * (t - delay)
                } else {
                    final_value
                }
            }
        };

        self.voltage = voltage;
        Ok(())
    }

    fn load_transient(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp2<CircuitReference, f64>> {
        vec![
            // Stamp the voltage source into the matrix (MNA format)
            // Branch current enters node_plus, leaves node_minus
            Stamp2::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp2::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            // Constraint equation: V_plus - V_minus = Voltage
            Stamp2::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp2::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            // RHS: The value of the voltage source at time t
            Stamp2::Rhs(self.branch.clone(), self.voltage),
        ]
    }
}
