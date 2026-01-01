use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{BranchIdentifier, CircuitReference, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::model::ind::InductorModel;
use crate::numerical_method::History;
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use piperine_macros::stamps;
use std::sync::Arc;

pub struct InductorParameters {
    pub name: String,
    pub model: Arc<InductorModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub inductance: f64,
}

pub struct Inductor {
    pub name: String,
    pub model: Arc<InductorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference, // Inductors need a branch current index
    pub inductance: f64,
}

impl Component for Inductor {
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

impl Inductor {
    pub fn new(
        netlist: &mut Netlist,
        parameters: InductorParameters,
    ) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name.clone(),
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            branch: netlist.connect_branch(BranchIdentifier {
                component: parameters.name,
                name: Some("branch".to_string()),
            }),
            inductance: parameters.inductance,
        })
    }
}

impl DcAnalysis for Inductor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<f64>> {
        let one = 1.0;
        stamps!(
            KCL(self.node_plus): { self.branch => one },
            KCL(self.node_minus): { self.branch => -one },
            KVL(self.branch): {
                self.node_plus => one,
                self.node_minus => -one,
                RHS => 0.0 // Force 0V drop across the inductor
            }
        )
    }
}

impl TransientAnalysis for Inductor {
    fn load_transient(
        &self,
        circuit_states: &CircuitStates,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<f64>> {
        let method = &context.numerical_method;
        let dt = transient_analysis_context.dt;

        // 1. Get differentiation coefficients for the branch current
        // V = L * di/dt
        // di/dt = (alpha_0 * i_now + history_sum) / dt
        let (alpha_0, history_sum) =
            method.get_differentiation_coeffs(circuit_states, &self.branch);

        // 2. Linearize the relationship:
        // V_now = (L/dt) * (alpha_0 * i_now + history_sum)
        // V_now = [(L * alpha_0) / dt] * i_now + [L * history_sum / dt]

        let req = (self.inductance * alpha_0) / dt;
        let v_hist = (self.inductance * history_sum) / dt;

        stamps!(
            // MNA Equation for the branch: V+ - V- - V_L = 0
            // V+ - V- - (req * i_now + v_hist) = 0
            // (1.0)V+ + (-1.0)V- + (-req)i_now = v_hist
            KVL(self.branch): {
                self.node_plus  => 1.0,
                self.node_minus => -1.0,
                self.branch     => -req,
                RHS             => v_hist
            },
            // Current KCL stamps: current leaves plus, enters minus
            KCL(self.node_plus): {
                self.branch     => 1.0
            },
            KCL(self.node_minus): {
                self.branch     => -1.0
            }
        )
    }
    fn check_convergence(
        &self,
        state: &CircuitStates,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        // Lookback 0: The solution the solver just produced
        // Lookback 1: The guess we used for this specific NR iteration
        let i_now = state.get_value(&self.branch, 0).unwrap_or(0.0);
        let i_prev = state.get_value(&self.branch, 1).unwrap_or(0.0);

        let diff = (i_now - i_prev).abs();

        // We use abstol (absolute tolerance) and reltol (relative tolerance)
        // Standard SPICE values: abstol = 1pA, reltol = 1e-3
        let rel_tol = context.reltol * i_now.abs().max(i_prev.abs());
        let abs_tol = context.abstol;

        // Converged if the change is below the absolute threshold
        // OR below the relative threshold
        diff < abs_tol || diff < rel_tol
    }
}

impl AcAnalysis for Inductor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitStates,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Complex<f64>>> {
        let z = Complex::new(0.0, ac_analysis_context.omega * self.inductance);
        let one = Complex::new(1.0, 0.0);

        stamps!(
            KCL(self.node_plus): {
                self.branch => one
            },
            KCL(self.node_minus): {
                self.branch => -one
            },
            Equation(self.branch): {
                self.node_plus        => one,
                self.node_minus       => -one,
                self.branch => -z
            }
        )
    }
}
