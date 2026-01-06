use crate::devices::diode::Diode;
use crate::devices::diode::model::DiodeShockleyModel;
use crate::devices::{Component, ComponentSpec, ModelResolver};
use crate::math::param::{OptionalParameter, SampleOptional};
use crate::math::unit::{Current, Ratio, UnitExt};
use crate::netlist::{IntoNodeIdentifier, Netlist, NodeIdentifier};
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
