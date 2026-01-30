use crate::circuit::netlist::{BranchIdentifier, NodeIdentifier};
use crate::devices::Component;
use crate::unit::{Dimensionless, Ohm, Siemens};

pub struct VoltageControlledCurrentSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    nc_plus: NodeIdentifier,
    nc_minus: NodeIdentifier,
    transconductance: Siemens,
    multiplier: Option<Dimensionless>,
}

impl VoltageControlledCurrentSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        nc_plus: impl Into<NodeIdentifier>,
        nc_minus: impl Into<NodeIdentifier>,
        transconductance: impl Into<Siemens>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            nc_plus: nc_plus.into(),
            nc_minus: nc_minus.into(),
            transconductance: transconductance.into(),
            multiplier: None,
        }
    }

    pub fn with_multiplier(&mut self, multiplier: impl Into<Dimensionless>) -> &mut Self {
        self.multiplier = Some(multiplier.into());
        self
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn node_control_plus(&self) -> &NodeIdentifier {
        &self.nc_plus
    }

    pub fn node_control_minus(&self) -> &NodeIdentifier {
        &self.nc_minus
    }

    pub fn transconductance(&self) -> Siemens {
        self.transconductance
    }

    pub fn multiplier(&self) -> Option<Dimensionless> {
        self.multiplier
    }
}

impl Component for VoltageControlledCurrentSource {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct VoltageControlledVoltageSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    nc_plus: NodeIdentifier,
    nc_minus: NodeIdentifier,
    gain: Dimensionless,
}

impl VoltageControlledVoltageSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        nc_plus: impl Into<NodeIdentifier>,
        nc_minus: impl Into<NodeIdentifier>,
        gain: impl Into<Dimensionless>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            nc_plus: nc_plus.into(),
            nc_minus: nc_minus.into(),
            gain: gain.into(),
        }
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn node_control_plus(&self) -> &NodeIdentifier {
        &self.nc_plus
    }

    pub fn node_control_minus(&self) -> &NodeIdentifier {
        &self.nc_minus
    }

    pub fn gain(&self) -> Siemens {
        self.gain
    }
}

impl Component for VoltageControlledVoltageSource {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CurrentControlledCurrentSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    controlling_source: BranchIdentifier,
    gain: Dimensionless,
    multiplier: Option<Dimensionless>,
}

impl CurrentControlledCurrentSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        controlling_source: impl Into<BranchIdentifier>,
        gain: impl Into<Dimensionless>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            controlling_source: controlling_source.into(),
            gain: gain.into(),
            multiplier: None,
        }
    }

    pub fn with_multiplier(&mut self, multiplier: impl Into<Dimensionless>) -> &mut Self {
        self.multiplier = Some(multiplier.into());
        self
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn branch_control(&self) -> &BranchIdentifier {
        &self.controlling_source
    }

    pub fn gain(&self) -> Dimensionless {
        self.gain
    }

    pub fn multiplier(&self) -> Option<Dimensionless> {
        self.multiplier
    }
}

impl Component for CurrentControlledCurrentSource {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CurrentControlledVoltageSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    controlling_source: BranchIdentifier,
    transresistance: Ohm,
}

impl CurrentControlledVoltageSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        controlling_source: impl Into<BranchIdentifier>,
        transresistance: impl Into<Ohm>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            controlling_source: controlling_source.into(),
            transresistance: transresistance.into(),
        }
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn branch_control(&self) -> &BranchIdentifier {
        &self.controlling_source
    }

    pub fn transresistance(&self) -> Dimensionless {
        self.transresistance
    }
}

impl Component for CurrentControlledVoltageSource {
    fn name(&self) -> &String {
        &self.name
    }
}
