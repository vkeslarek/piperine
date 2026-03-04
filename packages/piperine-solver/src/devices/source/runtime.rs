use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::analysis::truncation::BreakpointProvider;
use crate::circuit::netlist::{BranchIdentifier, CircuitReference, Netlist};
use crate::devices::soa::SoaCheck;
use crate::devices::source::{VoltageSource, Waveform};
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::{Second, UnitExt, Volt};
use crate::solver::Context;
use num_complex::Complex;
use num_traits::One;
use std::f64::consts::PI;
use std::sync::Arc;

pub struct VoltageSourceRuntime {
    component: Arc<VoltageSource>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    branch: CircuitReference,
    voltage: Volt,
}

impl Runtime for VoltageSourceRuntime {
    type ComponentType = VoltageSource;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());
        let branch =
            netlist.connect_branch(BranchIdentifier::from_component(component.name.clone()));

        Self {
            component,
            node_plus,
            node_minus,
            branch,
            voltage: 0.0,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, context: &Context) {
        let voltage = match self.component.waveform {
            Waveform::DC(value) => value,
            Waveform::Sine {
                amplitude,
                frequency,
                phase,
            } => {
                let t = context.time;
                let omega = 2.0 * PI * frequency;

                amplitude * (omega * t + phase).sin()
            }
            Waveform::Step {
                initial,
                final_value,
                delay,
                rise_time,
            } => {
                let t = context.time;
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
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_noise_source(&self) -> Option<&dyn NoiseSource> {
        None
    }

    fn as_soa_check(&self) -> Option<&dyn SoaCheck> {
        None
    }

    fn as_breakpoint_provider(&self) -> Option<&dyn BreakpointProvider> {
        Some(self)
    }
}

impl DcAnalysis for VoltageSourceRuntime {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        let dc_value = match self.component.waveform {
            Waveform::DC(v) => v,
            Waveform::Sine { amplitude: _, .. } => 0.0.V(),
            Waveform::Step { initial, delay, .. } => {
                if delay > 0.0 {
                    initial
                } else {
                    0.0.V()
                }
            }
        };

        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), dc_value),
        ]
    }
}

impl AcAnalysis for VoltageSourceRuntime {
    fn load_ac(
        &self,
        dc_analysis_result: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let (mag, phase_rad) = match &self.component.waveform {
            Waveform::Sine {
                amplitude, phase, ..
            } => (*amplitude, *phase),
            Waveform::Step { final_value, .. } => (*final_value, 0.0),
            _ => (0.0, 0.0),
        };

        let phasor = Complex::from_polar(mag, phase_rad);

        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), Complex::one()),
            Stamp::Matrix(
                self.branch.clone(),
                self.node_minus.clone(),
                -Complex::one(),
            ),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), Complex::one()),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.branch.clone(),
                -Complex::one(),
            ),
            Stamp::Rhs(self.branch.clone(), phasor),
        ]
    }
}

impl TransientAnalysis for VoltageSourceRuntime {
    fn load_transient(
        &self,
        circuit_states: &TransientAnalysisState,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.voltage),
        ]
    }
}

impl BreakpointProvider for VoltageSourceRuntime {
    fn get_breakpoints(&self, start_time: Second, stop_time: Second) -> Vec<Second> {
        let mut breakpoints = Vec::new();
        let start: f64 = start_time.into();
        let stop: f64 = stop_time.into();

        match self.component.waveform {
            Waveform::DC(_) => {
                // DC sources don't need breakpoints
            }
            Waveform::Sine { .. } => {
                // Sine waves are smooth, don't need breakpoints
                // (could add period-based breakpoints for very long simulations, but not critical)
            }
            Waveform::Step {
                delay, rise_time, ..
            } => {
                // Add breakpoints at the beginning, middle, and end of the step transition
                // This ensures we capture the edge properly with at least 3 points

                let t_start = delay;
                let t_end = delay + rise_time;

                // Only add breakpoints within the simulation time window
                if t_start >= start && t_start <= stop {
                    breakpoints.push(t_start.into());
                }

                // Add a breakpoint in the middle of the rise for better accuracy
                let t_mid = delay + rise_time / 2.0;
                if t_mid >= start && t_mid <= stop {
                    breakpoints.push(t_mid.into());
                }

                if t_end >= start && t_end <= stop {
                    breakpoints.push(t_end.into());
                }
            }
        }

        breakpoints
    }
}
