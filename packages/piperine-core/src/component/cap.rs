use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{CircuitReference, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::model::cap::CapacitorModel;
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use piperine_macros::stamps;
use std::sync::Arc;

pub struct CapacitorParameters {
    pub name: String,
    pub model: Arc<CapacitorModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub capacitance: f64,
}

pub struct Capacitor {
    pub name: String,
    pub model: Arc<CapacitorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub capacitance: f64,
}

impl Component for Capacitor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_dc(&self) -> Option<&dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient(&self) -> Option<&dyn TransientAnalysis> {
        Some(self)
    }

    fn as_ac(&self) -> Option<&dyn AcAnalysis> {
        Some(self)
    }
}

impl Capacitor {
    pub fn new(
        netlist: &mut Netlist,
        parameters: CapacitorParameters,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name,
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            capacitance: parameters.capacitance,
        })
    }
}

impl DcAnalysis for Capacitor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<f64>> {
        vec![]
    }
}

impl TransientAnalysis for Capacitor {
    fn load_transient(
        &self,
        states: &CircuitStates,
        trans_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<f64>> {
        let method = &context.numerical_method; // The NumericalMethod trait object
        let dt = trans_context.dt;

        // 1. Get differentiation coefficients for both nodes
        // Ic = C * d(Vp - Vm)/dt
        let (alpha_0, history_sum_p) = method.get_differentiation_coeffs(states, &self.node_plus);
        let (_, history_sum_m) = method.get_differentiation_coeffs(states, &self.node_minus);

        // 2. Linearize the derivative:
        // dv/dt = (alpha_0 * V_now + (history_sum_p - history_sum_m)) / dt
        let mut g_eq = self.capacitance * (alpha_0 / dt);
        let i_hist = self.capacitance * ((history_sum_p - history_sum_m) / dt);

        // 3. Apply Gmin for numerical stability (prevents singular matrices)
        if g_eq.abs() < context.gmin {
            g_eq = g_eq.signum() * context.gmin;
        }

        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => g_eq,
                self.node_minus => -g_eq,
                RHS             => -i_hist
            },
            KCL(self.node_minus): {
                self.node_plus  => -g_eq,
                self.node_minus => g_eq,
                RHS             => i_hist
            }
        )
    }
}

impl AcAnalysis for Capacitor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitStates,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Complex<f64>>> {
        // Y = j * omega * C
        let y = Complex::new(0.0, ac_analysis_context.omega * self.capacitance);
        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => y,
                self.node_minus => -y
            },
            KCL(self.node_minus): {
                self.node_plus  => -y,
                self.node_minus => y
            }
        )
    }
}
