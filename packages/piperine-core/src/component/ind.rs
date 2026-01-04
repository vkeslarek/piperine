use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::component::{Component, Context};
use crate::math::linear::Stamp;
use crate::math::unit::{Inductance, ReactanceConvert};
use crate::model::ind::InductorModel;
use crate::netlist::{BranchIdentifier, CircuitReference, Netlist, NodeIdentifier};
use crate::state::CircuitState;
use num_complex::Complex;
use num_traits::One;
use std::sync::Arc;

pub struct InductorParameters {
    pub name: String,
    pub model: Arc<InductorModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub inductance: Inductance,
}

pub struct Inductor {
    pub name: String,
    pub model: Arc<InductorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub inductance: Inductance,
}

impl Component for Inductor {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn as_dc_mut(&mut self) -> Option<&mut dyn DcAnalysis> {
        Some(self)
    }

    fn as_transient_mut(&mut self) -> Option<&mut dyn TransientAnalysis> {
        Some(self)
    }

    fn as_ac_mut(&mut self) -> Option<&mut dyn AcAnalysis> {
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
                name: None,
            }),
            inductance: parameters.inductance,
        })
    }
}

impl DcAnalysis for Inductor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), 0.0),
        ]
    }
}

impl TransientAnalysis for Inductor {
    fn load_transient(
        &self,
        circuit_states: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let dt = transient_analysis_context.dt;

        // 1. Get differentiation coefficients for the branch current
        // V = L * di/dt
        // di/dt = (alpha_0 * i_now + history_sum) / dt
        let (alpha_0, history_sum) = circuit_states.derivative_coefficients(&self.branch);

        // 2. Linearize the relationship:
        // V_now = (L/dt) * (alpha_0 * i_now + history_sum)
        // V_now = [(L * alpha_0) / dt] * i_now + [L * history_sum / dt]

        let req = (self.inductance * alpha_0) / dt;
        let v_hist = (self.inductance * history_sum) / dt;

        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.branch.clone(), -req.value),
            Stamp::Rhs(self.branch.clone(), v_hist.value),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
        ]
    }
    fn check_convergence(
        &self,
        state: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        // Lookback 0: The solution the solver just produced
        // Lookback 1: The guess we used for this specific NR iteration
        let i_now = state.get_commited_value(&self.branch, 0).unwrap_or(0.0);
        let i_prev = state.get_commited_value(&self.branch, 1).unwrap_or(0.0);

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
        _circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let z = self.inductance.to_impedance(ac_analysis_context.frequency);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), Complex::one()),
            Stamp::Matrix(
                self.node_minus.clone(),
                self.branch.clone(),
                -Complex::one(),
            ),
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), Complex::one()),
            Stamp::Matrix(
                self.branch.clone(),
                self.node_minus.clone(),
                -Complex::one(),
            ),
            Stamp::Matrix(self.branch.clone(), self.branch.clone(), -z.value),
        ]
    }
}
