use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::netlist::CircuitReference;
use crate::circuit::state::CircuitState;
use crate::devices::voltage_source::{VoltageSource, Waveform};
use crate::math::Stamp;
use crate::math::unit::{Angle, AngularVelocity, Radian, Ratio, Voltage};
use crate::solver::Context;
use std::f64::consts::PI;

impl TransientAnalysis for VoltageSource {
    fn update_transient(
        &mut self,
        _: &CircuitState<f64>,
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
                let omega: AngularVelocity = (frequency * Angle::new::<Radian>(2.0 * PI)).into();
                let phase_ratio: Ratio = phase.into();

                amplitude * (omega * t + phase_ratio).value.sin()
            }
            Waveform::Step {
                initial,
                final_value,
                delay,
                rise_time,
            } => {
                let t = transient_analysis_context.time.value;
                if t < delay {
                    initial
                } else if t >= delay && t < delay + rise_time {
                    let slope = (final_value - initial).value / rise_time;
                    Voltage::new::<uom::si::electric_potential::volt>(
                        initial.value + slope * (t - delay),
                    )
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
        circuit_states: &CircuitState<f64>,
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
