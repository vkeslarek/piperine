use crate::analysis::ac::{AcAnalysis, AcAnalysisContext};
use crate::analysis::dc::DcAnalysis;
use crate::analysis::transient::{TransientAnalysis, TransientAnalysisContext};
use crate::component::{Component, ComponentSpec, Context};
use crate::math::linear::Stamp;
use crate::math::param::{IntoParameter, Parameter};
use crate::math::unit::Voltage;
use crate::model::ModelResolver;
use crate::model::vsrc::{VoltageSourceIdealModel, VoltageSourceModel};
use crate::netlist::{
    BranchIdentifier, CircuitReference, IntoNodeIdentifier, Netlist, NodeIdentifier,
};
use crate::state::CircuitState;
use num_complex::Complex;
use num_traits::One;
use std::any::Any;
use std::sync::Arc;

pub struct VoltageSourceSpec {
    pub name: String,
    pub model: Option<String>,
    pub node_plus: NodeIdentifier,
    pub node_minus: NodeIdentifier,
    pub voltage: Parameter<Voltage>,
}

impl VoltageSourceSpec {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        voltage: impl IntoParameter<Voltage>,
    ) -> VoltageSourceSpec {
        VoltageSourceSpec {
            name: name.to_string(),
            model: None,
            node_plus: node_p.into(),
            node_minus: node_n.into(),
            voltage: voltage.into_parameter(),
        }
    }

    pub fn with_model(&mut self, name: &str) -> &mut Self {
        self.model = Some(name.to_string());
        self
    }
}

impl ComponentSpec for VoltageSourceSpec {
    fn instantiate(
        &self,
        netlist: &mut Netlist,
        model_resolver: &ModelResolver,
    ) -> crate::error::Result<Box<dyn Component>> {
        Ok(Box::new(VoltageSource {
            name: self.name.to_string(),
            model: model_resolver
                .resolve(self.model.clone())
                .unwrap_or_else(|| Arc::new(VoltageSourceIdealModel::new())),
            node_plus: netlist.connect_node(self.node_plus.clone()),
            node_minus: netlist.connect_node(self.node_minus.clone()),
            branch: netlist.connect_branch(BranchIdentifier {
                component: self.name.to_string(),
                name: None,
            }),
            voltage: self.voltage.sample(),
        }))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub struct VoltageSource {
    pub name: String,
    pub model: Arc<VoltageSourceModel>,
    pub node_plus: CircuitReference,
    pub node_minus: CircuitReference,
    pub branch: CircuitReference,
    pub voltage: Voltage,
}

impl Component for VoltageSource {
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

impl DcAnalysis for VoltageSource {
    fn load_dc(&self, context: &Context) -> Vec<Stamp<CircuitReference, f64>> {
        // let voltage = self.voltage.value.re;
        let voltage = 0.0;
        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), voltage),
        ]
    }
}

impl TransientAnalysis for VoltageSource {
    fn load_transient(
        &self,
        _: &CircuitState<f64>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, f64>> {
        let voltage = if transient_analysis_context.time > 2.0 * transient_analysis_context.dt {
            5.0
        } else {
            0.0
        };
        vec![
            Stamp::Matrix(self.branch.clone(), self.node_plus.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.node_minus.clone(), -1.0),
            Stamp::Matrix(self.node_plus.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.node_minus.clone(), self.branch.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), voltage),
        ]
    }
}

impl AcAnalysis for VoltageSource {
    fn load_ac(
        &self,
        _circuit_states: &CircuitState<Complex<f64>>,
        _: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<CircuitReference, Complex<f64>>> {
        let ac_volt = Complex::new(1.0, 0.0);

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
            Stamp::Rhs(self.branch.clone(), ac_volt),
        ]
    }
}
