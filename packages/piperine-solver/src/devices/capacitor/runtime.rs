use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::analysis::truncation::{IntegrationMethod, TruncationError};
use crate::circuit::netlist::{CircuitReference, Netlist};
use crate::devices::capacitor::Capacitor;
use crate::devices::soa::SoaCheck;
use crate::devices::Runtime;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::{Farad, Second};
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct CapacitorRuntime {
    component: Arc<Capacitor>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    capacitance: Farad,
}

impl Runtime for CapacitorRuntime {
    type ComponentType = Capacitor;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());
        let capacitance = component.capacitance;

        Self {
            component,
            node_plus,
            node_minus,
            capacitance,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, _: &Context) {
        // Do nothing
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

    fn as_truncation_error(&self) -> Option<&dyn TruncationError> {
        Some(self)
    }
}

impl DcAnalysis for CapacitorRuntime {
    fn load_dc(
        &self,
        _dc_circuit_state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // In DC analysis, capacitor is open circuit
        // Add gmin to prevent floating nodes
        let g = context.gmin;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
        ]
    }
}

impl AcAnalysis for CapacitorRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;
        let cap_val = self.capacitance;

        let admittance = Complex::new(0.0, omega * cap_val);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), admittance),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -admittance),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -admittance),
        ]
    }
}

impl TransientAnalysis for CapacitorRuntime {
    fn load_transient(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // For capacitor: i = C * dV/dt
        // The dynamic part (C matrix) is returned by load_transient_dynamic()
        // The solver will apply integration coefficients automatically
        // No additional static stamps needed
        vec![]
    }

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // For capacitor: i = C * dV/dt
        // This returns the C matrix that multiplies the derivative vector
        let c = self.capacitance;

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), c),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), c),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -c),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -c),
        ]
    }
}

impl TruncationError for CapacitorRuntime {
    fn suggest_timestep(
        &self,
        state_history: &TransientAnalysisState,
        time_history: &[f64],
        method: IntegrationMethod,
        context: &Context,
    ) -> Option<Second> {
        // Need at least order+2 points for divided differences
        let order = method.order();
        if state_history.len() < order + 2 {
            return None;
        }

        // Calculate charge history: q = C * V
        // V = V_plus - V_minus
        let mut charge_history = Vec::with_capacity(order + 2);
        for lookback in (0..=order + 1).rev() {
            let state = state_history.view(lookback)?;

            let v_plus = if let Some(idx) = self.node_plus.idx() {
                state[idx]
            } else {
                0.0 // Ground
            };

            let v_minus = if let Some(idx) = self.node_minus.idx() {
                state[idx]
            } else {
                0.0 // Ground
            };

            let voltage = v_plus - v_minus;
            let charge = self.capacitance * voltage;
            charge_history.push(charge);
        }

        // Calculate voltage tolerance
        let v_current = charge_history[0] / self.capacitance;
        let v_prev = charge_history[1] / self.capacitance;
        let volttol = context.abstol + context.reltol * v_current.abs().max(v_prev.abs());

        // Calculate charge tolerance (for current)
        let q_current = charge_history[0].abs();
        let q_prev = charge_history[1].abs();
        let current_tol = context.reltol * q_current.max(q_prev).max(context.chgtol);

        // Time difference for current derivative approximation
        let dt_current = if time_history.len() > 1 {
            time_history[0] - time_history[1]
        } else {
            return None;
        };

        let chargetol = if dt_current > 0.0 {
            current_tol / dt_current
        } else {
            return None;
        };

        let tol = volttol.max(chargetol);

        // Calculate divided differences
        let mut diff = charge_history.clone();
        let mut deltmp: Vec<f64> = time_history[0..=order]
            .iter()
            .zip(time_history[1..=order + 1].iter())
            .map(|(t0, t1)| t0 - t1)
            .collect();

        // Divided difference algorithm from ngSpice cktterr.c
        for j in (0..=order).rev() {
            for i in 0..=j {
                if deltmp[i].abs() < 1e-20 {
                    // Avoid division by very small numbers
                    return None;
                }
                diff[i] = (diff[i] - diff[i + 1]) / deltmp[i];
            }

            if j > 0 {
                for i in 0..j {
                    deltmp[i] = deltmp[i + 1]
                        + (if i < time_history.len() - 1 {
                            time_history[i] - time_history[i + 1]
                        } else {
                            0.0
                        });
                }
            }
        }

        // Calculate timestep suggestion
        let factor = method.truncation_coefficient();
        let error_magnitude = factor * diff[0].abs();

        if error_magnitude < context.abstol {
            // Error too small to be meaningful, return large timestep
            return Some((tol / context.abstol).into());
        }

        let ratio = context.trtol * tol / error_magnitude.max(context.abstol);

        // Apply order-dependent scaling: dt_new = ratio^(1/order)
        let dt_new = match order {
            1 => ratio,
            2 => ratio.sqrt(),
            3 => ratio.cbrt(),
            _ => ratio.powf(1.0 / order as f64),
        };

        Some(dt_new.into())
    }
}
