use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::{DcAnalysis, DcAnalysisResult, DcAnalysisState};
use crate::analysis::noise::NoiseSource;
use crate::analysis::transient::{
    TransientAnalysis, TransientAnalysisContext, TransientAnalysisState,
};
use crate::analysis::truncation::{IntegrationMethod, TruncationError};
use crate::circuit::netlist::{BranchIdentifier, CircuitReference, Netlist};
use crate::devices::Runtime;
use crate::devices::inductor::Inductor;
use crate::devices::soa::SoaCheck;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::{Henry, Second};
use crate::solver::Context;
use num_complex::Complex;
use std::sync::Arc;

pub struct InductorRuntime {
    component: Arc<Inductor>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,
    current_ref: CircuitReference,
    inductance: Henry,
}

impl Runtime for InductorRuntime {
    type ComponentType = Inductor;

    fn allocate(component: Arc<Self::ComponentType>, netlist: &mut Netlist) -> Self
    where
        Self: Sized,
    {
        let current_ref = BranchIdentifier::from_component(&component.name);

        let node_plus = netlist.connect_node(component.node_plus.clone());
        let node_minus = netlist.connect_node(component.node_minus.clone());
        let current_ref = netlist.connect_branch(current_ref);
        let inductance = component.inductance;

        Self {
            component,
            node_plus,
            node_minus,
            current_ref,
            inductance,
        }
    }

    fn update(&mut self, _: &CircularArrayBuffer2<f64>, _: &Context)
    where
        Self: Sized,
    {
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

impl DcAnalysis for InductorRuntime {
    fn load_dc(&self, _: &DcAnalysisState, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
        ]
    }
}

impl AcAnalysis for InductorRuntime {
    fn load_ac(
        &self,
        _: &DcAnalysisResult,
        ac_analysis_context: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;
        let impedance = Complex::new(0.0, omega * self.inductance);

        vec![
            Stamp::Matrix(
                self.node_plus.clone(),
                self.current_ref.clone(),
                Complex::new(1.0, 0.0),
            ),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.current_ref.clone(),
                Complex::new(-1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.node_plus.clone(),
                Complex::new(1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.node_minus.clone(),
                Complex::new(-1.0, 0.0),
            ),
            Stamp::Matrix(
                self.current_ref.clone(),
                self.current_ref.clone(),
                -impedance,
            ),
        ]
    }
}

impl TransientAnalysis for InductorRuntime {
    fn load_transient(
        &self,
        _: &TransientAnalysisState,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.current_ref.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.current_ref.clone(), -1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.current_ref.clone(), self.node_minus.clone(), -1.0),
        ]
    }

    fn load_transient_dynamic(
        &self,
        _circuit_states: &TransientAnalysisState,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let l = self.inductance;

        vec![Stamp::Matrix(
            self.current_ref.clone(),
            self.current_ref.clone(),
            -l,
        )]
    }
}

impl TruncationError for InductorRuntime {
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

        // Calculate flux history: φ = L * I
        // Current flows through the branch current variable
        let mut flux_history = Vec::with_capacity(order + 2);
        for lookback in (0..=order + 1).rev() {
            let state = state_history.view(lookback)?;

            let current = if let Some(idx) = self.current_ref.idx() {
                state[idx]
            } else {
                // Should not happen - inductors always have a current branch
                return None;
            };

            let flux = self.inductance * current;
            flux_history.push(flux);
        }

        // Calculate current tolerance (analogous to voltage tolerance for capacitor)
        let i_current = flux_history[0] / self.inductance;
        let i_prev = flux_history[1] / self.inductance;
        let currenttol = context.abstol + context.reltol * i_current.abs().max(i_prev.abs());

        // Calculate flux tolerance (analogous to charge tolerance)
        // For inductors, we use flux instead of charge
        let flux_current = flux_history[0].abs();
        let flux_prev = flux_history[1].abs();

        // Use chgtol as flux tolerance (both are in Weber = Volt·second)
        let flux_tol = context.reltol * flux_current.max(flux_prev).max(context.chgtol);

        // Time difference for derivative approximation
        let dt_current = if time_history.len() > 1 {
            time_history[0] - time_history[1]
        } else {
            return None;
        };

        let fluxtol = if dt_current > 0.0 {
            flux_tol / dt_current
        } else {
            return None;
        };

        let tol = currenttol.max(fluxtol);

        // Calculate divided differences
        let mut diff = flux_history.clone();
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
