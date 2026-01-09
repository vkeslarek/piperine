use std::any::Any;
use crate::analysis::dc::DcAnalysis;
use crate::devices::Component;
use crate::devices::diode::model::{DiodeModel, DiodeModelType};
use crate::math::unit::{Conductance, Current, Temperature, UnitExt};
use crate::netlist::{CircuitReference, IntoNodeIdentifier, Netlist};
use crate::util::AsAny;
use std::sync::Arc;

mod dc;
mod model;

#[derive(Clone)]
pub struct Diode {
    name: String,
    model: Arc<dyn DiodeModelType>,
    node_plus: CircuitReference,
    node_minus: CircuitReference,

    pub temp: Option<Temperature>,

    // Runtime State (Calculated during linearization)
    pub g_eq: Conductance, // Dynamic Conductance (gd = dI/dV)
    pub i_eq: Current,     // Equivalent Current Source offset
}

impl Diode {
    pub fn new(
        name: &str,
        node_p: impl IntoNodeIdentifier,
        node_n: impl IntoNodeIdentifier,
        netlist: &mut Netlist,
    ) -> Self {
        Self {
            name: name.to_string(),
            model: Arc::new(DiodeModel::default()),
            node_plus: netlist.connect_node(node_p.into()),
            node_minus: netlist.connect_node(node_n.into()),
            temp: None,
            // Initial guess: Start as a very small conductance (almost open)
            g_eq: 0.0.pS(),
            i_eq: 0.0.A(),
        }
    }

    pub fn with_model(&mut self, model: Arc<dyn DiodeModelType>) -> &mut Self {
        self.model = model;
        self
    }
}

// Standard Boilerplate
impl AsAny for Diode {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Component for Diode {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn as_dc(&mut self) -> Option<&mut dyn DcAnalysis> {
        Some(self)
    }
    fn as_ac(&mut self) -> Option<&mut dyn crate::analysis::ac::AcAnalysis> {
        None
    }
    fn as_transient(&mut self) -> Option<&mut dyn crate::analysis::transient::TransientAnalysis> {
        None
    }
}
