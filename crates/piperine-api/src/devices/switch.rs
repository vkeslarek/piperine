use crate::devices::Component;
use crate::node::Node;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use std::sync::Arc;

/// Initial state of a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchState {
    On,
    Off,
}

/// Voltage-controlled switch (`S`).
///
/// `SXXXX n+ n- nc+ nc- models <ON|OFF>`
/// See ngspice manual §3.3.15.
#[derive(Debug)]
pub struct VoltageSwitch {
    name: String,
    node_plus: Node,
    node_minus: Node,
    ctrl_plus: Node,
    ctrl_minus: Node,
    model: Arc<dyn crate::models::switch::VoltageSwitchModel + Send + Sync>,
    initial_state: Option<SwitchState>,
}

impl Clone for VoltageSwitch {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            node_plus: self.node_plus.clone(),
            node_minus: self.node_minus.clone(),
            ctrl_plus: self.ctrl_plus.clone(),
            ctrl_minus: self.ctrl_minus.clone(),
            model: Arc::clone(&self.model),
            initial_state: self.initial_state,
        }
    }
}

impl VoltageSwitch {
    pub const SYMBOL: &str = "S";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        ctrl_plus: impl Into<Node>,
        ctrl_minus: impl Into<Node>,
        model: Arc<dyn crate::models::switch::VoltageSwitchModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            ctrl_plus: ctrl_plus.into(),
            ctrl_minus: ctrl_minus.into(),
            model,
            initial_state: None,
        }
    }

    pub fn with_initial_state(&mut self, state: SwitchState) -> &mut Self {
        self.initial_state = Some(state);
        self
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn model_name(&self) -> &str { self.model.model_name() }
}

impl Component for VoltageSwitch {}

impl SpiceElement for VoltageSwitch {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        Some(Arc::clone(&self.model) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for VoltageSwitch {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL, self.name(),
            self.node_plus, self.node_minus,
            self.ctrl_plus, self.ctrl_minus,
            model_name
        );
        if let Some(state) = &self.initial_state {
            match state {
                SwitchState::On => s.push_str(" ON"),
                SwitchState::Off => s.push_str(" OFF"),
            }
        }
        s
    }
}

/// Current-controlled switch (`W`).
///
/// `WXXXX n+ n- vname models <ON|OFF>`
/// See ngspice manual §3.3.15.
#[derive(Debug)]
pub struct CurrentSwitch {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// Name of controlling voltage source.
    v_source: String,
    model: Arc<dyn crate::models::switch::VoltageSwitchModel + Send + Sync>,
    initial_state: Option<SwitchState>,
}

impl Clone for CurrentSwitch {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            node_plus: self.node_plus.clone(),
            node_minus: self.node_minus.clone(),
            v_source: self.v_source.clone(),
            model: Arc::clone(&self.model),
            initial_state: self.initial_state,
        }
    }
}

impl CurrentSwitch {
    pub const SYMBOL: &str = "W";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        v_source: impl Into<String>,
        model: Arc<dyn crate::models::switch::VoltageSwitchModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            v_source: v_source.into(),
            model,
            initial_state: None,
        }
    }

    pub fn with_initial_state(&mut self, state: SwitchState) -> &mut Self {
        self.initial_state = Some(state);
        self
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn model_name(&self) -> &str { self.model.model_name() }
    pub fn v_source(&self) -> &str { &self.v_source }
}

impl Component for CurrentSwitch {}

impl SpiceElement for CurrentSwitch {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }

    fn spice_model(&self) -> Option<Arc<dyn crate::spice::SpiceModel>> {
        Some(Arc::clone(&self.model) as Arc<dyn crate::spice::SpiceModel>)
    }
}

impl SpiceComponent for CurrentSwitch {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL, self.name(),
            self.node_plus, self.node_minus,
            self.v_source(), model_name
        );
        if let Some(state) = &self.initial_state {
            match state {
                SwitchState::On => s.push_str(" ON"),
                SwitchState::Off => s.push_str(" OFF"),
            }
        }
        s
    }
}
