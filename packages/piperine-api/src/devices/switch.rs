use crate::circuit::netlist::{BranchIdentifier, NodeIdentifier};
use crate::devices::{Component, Model};
use crate::unit::{Ampere, Ohm, Volt};
use std::sync::Arc;

pub struct VoltageControlledSwitch {
    name: String,
    model: Arc<VoltageControlledSwitchModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    nsense_plus: NodeIdentifier,
    nsense_minus: NodeIdentifier,
    initial_state: Option<bool>,
}

impl VoltageControlledSwitch {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        nsense_plus: impl Into<NodeIdentifier>,
        nsense_minus: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(VoltageControlledSwitchModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            nsense_plus: nsense_plus.into(),
            nsense_minus: nsense_minus.into(),
            initial_state: None,
        }
    }

    pub fn with_model(&mut self, model: Arc<VoltageControlledSwitchModel>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_initial_state(&mut self, state: impl Into<bool>) -> &mut Self {
        self.initial_state = Some(state.into());
        self
    }

    pub fn model(&self) -> &Arc<VoltageControlledSwitchModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn nsense_plus(&self) -> &NodeIdentifier {
        &self.nsense_plus
    }

    pub fn nsense_minus(&self) -> &NodeIdentifier {
        &self.nsense_minus
    }

    pub fn initial_state(&self) -> Option<bool> {
        self.initial_state
    }
}

impl Component for VoltageControlledSwitch {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CurrentControlledSwitch {
    name: String,
    model: Arc<CurrentControlledSwitchModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    bsense: BranchIdentifier,
    initial_state: Option<bool>,
}

impl CurrentControlledSwitch {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        bsense: impl Into<BranchIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(CurrentControlledSwitchModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            bsense: bsense.into(),
            initial_state: None,
        }
    }
    pub fn with_model(&mut self, model: Arc<CurrentControlledSwitchModel>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_initial_state(&mut self, state: impl Into<bool>) -> &mut Self {
        self.initial_state = Some(state.into());
        self
    }

    pub fn model(&self) -> &Arc<CurrentControlledSwitchModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn bsense(&self) -> &BranchIdentifier {
        &self.bsense
    }

    pub fn initial_state(&self) -> Option<bool> {
        self.initial_state
    }
}

impl Component for CurrentControlledSwitch {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct VoltageControlledSwitchModel {
    threshold: Volt,
    hysteresis: Volt,
    resistance_on: Ohm,
    resistance_off: Ohm,
}

impl Default for VoltageControlledSwitchModel {
    fn default() -> Self {
        Self {
            threshold: 0.0,
            hysteresis: 0.0,
            resistance_on: 0.0,
            resistance_off: 1e12,
        }
    }
}

impl VoltageControlledSwitchModel {
    pub fn with_threshold(&mut self, threshold: impl Into<Volt>) -> &mut Self {
        self.threshold = threshold.into();
        self
    }

    pub fn with_hysteresis(&mut self, hysteresis: impl Into<Volt>) -> &mut Self {
        self.hysteresis = hysteresis.into();
        self
    }

    pub fn with_resistance_on(&mut self, resistance_on: impl Into<Ohm>) -> &mut Self {
        self.resistance_on = resistance_on.into();
        self
    }

    pub fn with_resistance_off(&mut self, resistance_off: impl Into<Ohm>) -> &mut Self {
        self.resistance_off = resistance_off.into();
        self
    }

    pub fn threshold(&self) -> Volt {
        self.threshold
    }

    pub fn hysteresis(&self) -> Volt {
        self.hysteresis
    }

    pub fn resistance_on(&self) -> Ohm {
        self.resistance_on
    }

    pub fn resistance_off(&self) -> Ohm {
        self.resistance_off
    }
}

impl Model for VoltageControlledSwitchModel {
    type ComponentType = VoltageControlledSwitch;
}

pub struct CurrentControlledSwitchModel {
    threshold: Ampere,
    hysteresis: Ampere,
    resistance_on: Ohm,
    resistance_off: Ohm,
}

impl Default for CurrentControlledSwitchModel {
    fn default() -> Self {
        Self {
            threshold: 0.0,
            hysteresis: 0.0,
            resistance_on: 0.0,
            resistance_off: 1e12,
        }
    }
}

impl CurrentControlledSwitchModel {
    pub fn with_threshold(&mut self, threshold: impl Into<Ampere>) -> &mut Self {
        self.threshold = threshold.into();
        self
    }

    pub fn with_hysteresis(&mut self, hysteresis: impl Into<Ampere>) -> &mut Self {
        self.hysteresis = hysteresis.into();
        self
    }

    pub fn with_resistance_on(&mut self, resistance_on: impl Into<Ohm>) -> &mut Self {
        self.resistance_on = resistance_on.into();
        self
    }

    pub fn with_resistance_off(&mut self, resistance_off: impl Into<Ohm>) -> &mut Self {
        self.resistance_off = resistance_off.into();
        self
    }

    pub fn threshold(&self) -> Ampere {
        self.threshold
    }

    pub fn hysteresis(&self) -> Ampere {
        self.hysteresis
    }

    pub fn resistance_on(&self) -> Ohm {
        self.resistance_on
    }

    pub fn resistance_off(&self) -> Ohm {
        self.resistance_off
    }
}

impl Model for CurrentControlledSwitchModel {
    type ComponentType = CurrentControlledSwitch;
}
