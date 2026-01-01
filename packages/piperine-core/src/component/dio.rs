use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::circuit::{CircuitReference, Netlist, NodeIdentifier};
use crate::component::{Component, Context};
use crate::model::dio::DiodeModel;
use crate::numerical_method::History;
use crate::solver::Stamp;
use crate::state::CircuitStates;
use num_complex::Complex;
use piperine_macros::stamps;
use std::sync::Arc;

pub struct DiodeParameters {
    pub name: String,
    pub model: Arc<DiodeModel>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    // Typical defaults: Is = 1e-14, n = 1.0
    pub saturation_current: f64,
    pub emission_coefficient: f64,
}

pub struct Diode {
    pub name: String,
    pub model: Arc<DiodeModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub saturation_current: f64,
    pub emission_coefficient: f64,
    pub g_eq: f64,
    pub i_eq: f64,
    pub i_d: f64,
    pub v_prev_iter: f64,
}

impl Component for Diode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(&mut self, circuit_states: &CircuitStates, _: &Context) -> crate::error::Result<()> {
        self.model.clone().update(self, circuit_states)
    }

    // fn ask(&self, measure: &Measure, circuit_states: &CircuitStates) -> crate::error::Result<f64> {
    //     if measure.is_for_current(BranchIdentifier { component: self.name, name: None }) {
    //         let v_plus = circuit_states.get_value(&self.node_plus, 0).unwrap_or(0.0);
    //         let v_minus = circuit_states.get_value(&self.node_minus, 0).unwrap_or(0.0);
    //         let vd = v_plus - v_minus;
    //
    //         Ok(self.g_eq * vd - self.i_eq)
    //     } else {
    //         Component::ask(self, measure, circuit_states)
    //     }
    // }

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

impl Diode {
    pub fn new(netlist: &mut Netlist, parameters: DiodeParameters) -> crate::error::Result<Self> {
        Ok(Self {
            name: parameters.name,
            model: parameters.model,
            node_plus: netlist.connect_node(parameters.node_plus),
            node_minus: netlist.connect_node(parameters.node_minus),
            saturation_current: parameters.saturation_current,
            emission_coefficient: parameters.emission_coefficient,
            g_eq: 0.0, // Initial guess
            i_eq: 0.0,
            i_d: 0.0,
            v_prev_iter: 0.0,
        })
    }

    fn calculate_shockley(&self, vd: f64, vt: f64) -> (f64, f64) {
        let nvt = self.emission_coefficient * vt;
        let exp_term = (vd / nvt).exp();

        let id = self.saturation_current * (exp_term - 1.0);
        let gd = (self.saturation_current / nvt) * exp_term;

        (id, gd)
    }

    pub fn linearize(&mut self, vd_new: f64, vt: f64) -> crate::error::Result<()> {
        let is = self.saturation_current;
        let nvt = self.emission_coefficient * vt;

        // 1. Voltage Limiting
        let vd_old = self.v_prev_iter;
        let mut vd = vd_new;

        let v_crit = nvt * (nvt / (2.0f64.sqrt() * is)).ln();
        if vd > v_crit && (vd - vd_old).abs() > (2.0 * nvt) {
            vd = if vd > vd_old {
                vd_old + 2.0 * nvt
            } else {
                // Logarithmic limiting is more stable for downward swings
                vd_old - 2.0 * nvt * ((vd_old - vd) / (2.0 * nvt)).ln()
            };
        }

        // 2. Physics calculation
        let exp_term = (vd / nvt).exp();
        let id = is * (exp_term - 1.0);
        let gd = (is / nvt) * exp_term;

        // 3. Update internal state
        self.g_eq = gd;
        self.i_eq = -(id - gd * vd);
        self.v_prev_iter = vd; // Save for the next iteration

        Ok(())
    }
}

impl DcAnalysis for Diode {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<f64>> {
        // Stamp the linearized companion model
        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => self.g_eq,
                self.node_minus => -self.g_eq,
                RHS             => self.i_eq
            },
            KCL(self.node_minus): {
                self.node_plus  => -self.g_eq,
                self.node_minus => self.g_eq,
                RHS             => -self.i_eq
            }
        )
    }
}

impl TransientAnalysis for Diode {
    fn load_transient(
        &self,
        _: &CircuitStates,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<f64>> {
        self.load_dc(context)
    }

    fn check_convergence(
        &self,
        state: &CircuitStates,
        _: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        // Current solution (Iteration k)
        let v_now = state.get_value(&self.node_plus, 0).unwrap_or(0.0)
            - state.get_value(&self.node_minus, 0).unwrap_or(0.0);

        // Previous guess (Iteration k-1)
        let v_prev = state.get_value(&self.node_plus, 1).unwrap_or(0.0)
            - state.get_value(&self.node_minus, 1).unwrap_or(0.0);

        // 1. Voltage Convergence
        let v_diff = (v_now - v_prev).abs();
        let v_tol = context.reltol * v_now.abs().max(v_prev.abs()) + context.vntol;

        if v_diff > v_tol {
            return false;
        }

        // 2. Current Convergence
        // We calculate current for both points to see if the physics settled
        let vt = 0.02585;
        let (i_now, _) = self.calculate_shockley(v_now, vt);
        let (i_prev, _) = self.calculate_shockley(v_prev, vt);

        let i_diff = (i_now - i_prev).abs();
        let i_tol = context.reltol * i_now.abs().max(i_prev.abs()) + context.abstol;

        i_diff < i_tol
    }
}

impl AcAnalysis for Diode {
    fn load_ac(
        &self,
        circuit_states: &CircuitStates,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<Complex<f64>>> {
        // 1. Get the DC operating point voltage from history (lookback 0)
        let v_plus = circuit_states.get_value(&self.node_plus, 0).unwrap_or(0.0);
        let v_minus = circuit_states.get_value(&self.node_minus, 0).unwrap_or(0.0);
        let vd_dc = v_plus - v_minus;

        // 2. Calculate small-signal conductance gd
        let vt = 0.02585; // Thermal voltage
        let nvt = self.emission_coefficient * vt;
        let gd_val = (self.saturation_current / nvt) * (vd_dc / nvt).exp();

        let gd = Complex::new(gd_val, 0.0);

        // 3. Stamp as a linear conductance
        stamps!(
            KCL(self.node_plus): {
                self.node_plus  => gd,
                self.node_minus => -gd
            },
            KCL(self.node_minus): {
                self.node_plus  => -gd,
                self.node_minus => gd
            }
        )
    }
}
