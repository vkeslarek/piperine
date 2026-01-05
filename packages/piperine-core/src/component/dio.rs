use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::component::{Component, ComponentSpec, Context};
use crate::math::linear::Stamp;
use crate::math::param::{IntoParameter, OptionalParameter, SampleOptional};
use crate::math::unit::{Conductance, Current, Ratio, UnitExt, Voltage};
use crate::model::ModelResolver;
use crate::model::dio::{DiodeModel, DiodeShockleyModel};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::state::CircuitState;
use num_complex::{Complex, ComplexFloat};
use num_traits::Zero;
use std::any::Any;
use std::sync::Arc;

pub struct DiodeSpec {
    name: String,
    model: Option<String>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    saturation_current: OptionalParameter<Current>,
    emission_coefficient: OptionalParameter<Ratio>,
}

impl DiodeSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
    ) -> DiodeSpec {
        DiodeSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            saturation_current: None,
            emission_coefficient: None,
        }
    }
}

impl ComponentSpec for DiodeSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Diode {
            name: self.name.clone(),
            model: resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(DiodeShockleyModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            saturation_current: self.saturation_current.sample_opt().unwrap_or(1e-12.A()),
            emission_coefficient: self
                .emission_coefficient
                .sample_opt()
                .unwrap_or(1.3.ratio()),
            g_eq: 0.0.S(),
            i_eq: 0.0.A(),
            v_new: 0.0.V(),
            v_old: 0.0.V(),
            v_guess: 0.0.V(),
            v_linearized: 0.0.V(),
        }))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Debug)]
pub struct Diode {
    pub name: String,
    pub model: Arc<DiodeModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub saturation_current: Current,
    pub emission_coefficient: Ratio,
    pub g_eq: Conductance,
    pub i_eq: Current,

    // CHANGE HERE: Separate new guess from old linearization point
    pub v_new: Voltage,        // The raw guess from the matrix (k)
    pub v_old: Voltage,        // The limited voltage from the previous iteration (k-1)
    pub v_guess: Voltage,      // The raw input from the matrix solver (Iteration K)
    pub v_linearized: Voltage, // The safe, limited voltage we used last time (Iteration K-1)
}

impl Component for Diode {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn update(&mut self) -> crate::error::Result<()> {
        self.model.clone().update(self)
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

impl DcAnalysis for Diode {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        let g_leak = 1e-9; // 1 Giga-Ohm leakage path (Conductance)

        vec![
            // Place the fixed conductance in the matrix
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_leak),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_leak),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_leak),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_leak),
            // NO RHS: Passive components have no current source I_eq in a fixed-G model.
            // This effectively sets the initial guess for the diode current to 0.
        ]
    }
}

impl TransientAnalysis for Diode {
    fn update_transient(
        &mut self,
        circuit_states: &CircuitState<f64>,
        _: &TransientAnalysisContext,
        _: &Context,
    ) -> crate::error::Result<()> {
        let v_plus = circuit_states
            .get_guess_value(&self.node_plus)
            .unwrap_or(0.0);
        let v_minus = circuit_states
            .get_guess_value(&self.node_minus)
            .unwrap_or(0.0);

        // STORE THE RAW GUESS. Do not touch v_linearized!
        self.v_guess = (v_plus - v_minus).V();
        Ok(())
    }

    fn load_transient(
        &self,
        _states: &CircuitState<f64>,
        _ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let g = self.g_eq.value.max(1e-9);
        let i = self.i_eq.value.re.min(1e3);
        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
            // RHS: Current Id flows PLUS -> MINUS
            // So we subtract from the PLUS node and add to the MINUS node
            Stamp::Rhs(self.node_plus.clone(), -i),
            Stamp::Rhs(self.node_minus.clone(), i),
        ]
    }
    fn check_convergence(
        &self,
        circuit_states: &CircuitState<f64>,
        _ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> bool {
        let v_now = circuit_states
            .get_guess_value(&self.node_plus)
            .unwrap_or(0.0)
            - circuit_states
                .get_guess_value(&self.node_minus)
                .unwrap_or(0.0);

        // Compare the Solver's Result (v_now) against the Voltage we Linearized Around (v_linearized)
        let v_lin = self.v_linearized.value.re;

        (v_now - v_lin).abs() < (context.reltol * v_now.abs().max(v_lin.abs()) + context.vntol)
    }
}

impl AcAnalysis for Diode {
    fn load_ac(
        &self,
        _: &CircuitState<Complex<f64>>,
        _: &AcAnalysisContext,
        _: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        // Use the g_eq calculated during the final OP/Transient step
        let g = Complex::new(self.g_eq.value, 0.0);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g),
        ]
    }
}
