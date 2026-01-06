use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::component::{Component, ComponentSpec, Context};
use crate::math::linear::Stamp;
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::{AdmittanceConvert, Capacitance, ReactanceConvert};
use crate::model::ModelResolver;
use crate::model::cap::{CapacitorIdealModel, CapacitorModel};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier};
use crate::state::CircuitState;
use num_complex::Complex;
use std::any::Any;
use std::sync::Arc;

pub struct CapacitorSpec {
    pub name: String,
    pub model: Option<String>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub capacitance: Parameter<Capacitance>,
}

impl CapacitorSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_m: impl IntoNodeIdentifier,
        capacitance: impl IntoParameter<Capacitance>,
    ) -> CapacitorSpec {
        CapacitorSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_m.into(),
            capacitance: capacitance.into_parameter(),
        }
    }
    pub fn with_model(&mut self, name: &str) -> &mut Self {
        self.model = Some(name.to_string());
        self
    }
}

impl ComponentSpec for CapacitorSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(Capacitor {
            name: self.name.clone(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(CapacitorIdealModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            capacitance: self.capacitance.sample(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct Capacitor {
    pub name: String,
    pub model: Arc<CapacitorModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub capacitance: Capacitance,
}

impl Component for Capacitor {
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

impl DcAnalysis for Capacitor {
    fn load_dc(&self, _: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        vec![]
    }
}

impl TransientAnalysis for Capacitor {
    fn load_transient(
        &self,
        states: &CircuitState<f64>,
        _trans_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        // states.derivative_coefficients already returns values in units of 1/s
        let (alpha_0, history_sum_p) = states.derivative_coefficients(&self.node_plus);
        let (_, history_sum_m) = states.derivative_coefficients(&self.node_minus);

        // Geq = C * alpha_0
        let g_eq = self.capacitance.value * alpha_0;
        // Ieq = C * (hist_p - hist_m)
        let i_hist = self.capacitance.value * (history_sum_p - history_sum_m);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), g_eq),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), g_eq),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -g_eq),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -g_eq),
            Stamp::Rhs(self.node_plus.clone(), -i_hist),
            Stamp::Rhs(self.node_minus.clone(), i_hist),
        ]
    }
}

impl AcAnalysis for Capacitor {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        ac_analysis_context: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        // Convert Hz to rad/s
        let omega = 2.0 * std::f64::consts::PI * ac_analysis_context.frequency;

        // Y = j * omega * C
        // We create a Complex number with 0.0 real part and (omega * C) imaginary part
        let y = Complex::new(0.0, omega.value * self.capacitance.value);

        vec![
            Stamp::Matrix(self.node_plus.clone(), self.node_plus.clone(), y),
            Stamp::Matrix(self.node_minus.clone(), self.node_minus.clone(), y),
            Stamp::Matrix(self.node_plus.clone(), self.node_minus.clone(), -y),
            Stamp::Matrix(self.node_minus.clone(), self.node_plus.clone(), -y),
        ]
    }
}
