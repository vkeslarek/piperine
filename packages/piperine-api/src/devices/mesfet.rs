use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Model};
use crate::unit::{Dimensionless, Farad, Ohm, Volt};
use std::sync::Arc;

#[derive(Clone)]
pub enum MesfetModelVariant {
    Statz(Arc<MesfetStatzModel>),
    Ytterdal(Arc<MesfetYtterdalModel>),
    Hfet(Arc<MesfetHfetModel>),
}

impl Default for MesfetModelVariant {
    fn default() -> Self {
        MesfetModelVariant::Statz(Arc::new(MesfetStatzModel::default()))
    }
}

impl Model for MesfetModelVariant {
    type ComponentType = MetalSemiconductorFieldEffectTransistor;
}

#[derive(Clone)]
pub struct MetalSemiconductorFieldEffectTransistor {
    name: String,
    model: MesfetModelVariant,
    node_d: NodeIdentifier,
    node_g: NodeIdentifier,
    node_s: NodeIdentifier,
    area: Option<Dimensionless>,
    off: Option<bool>,
    ic_vds: Option<Volt>,
    ic_vgs: Option<Volt>,
}

impl MetalSemiconductorFieldEffectTransistor {
    pub fn new(
        name: impl Into<String>,
        node_d: impl Into<NodeIdentifier>,
        node_g: impl Into<NodeIdentifier>,
        node_s: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            model: MesfetModelVariant::default(),
            node_d: node_d.into(),
            node_g: node_g.into(),
            node_s: node_s.into(),
            area: None,
            off: None,
            ic_vds: None,
            ic_vgs: None,
        }
    }

    pub fn with_model(&mut self, model: MesfetModelVariant) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_area(&mut self, area: impl Into<Dimensionless>) -> &mut Self {
        self.area = Some(area.into());
        self
    }

    pub fn with_off(&mut self, off: impl Into<bool>) -> &mut Self {
        self.off = Some(off.into());
        self
    }

    pub fn with_ic(&mut self, vds: impl Into<Volt>, vgs: impl Into<Volt>) -> &mut Self {
        self.ic_vds = Some(vds.into());
        self.ic_vgs = Some(vgs.into());
        self
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn model(&self) -> &MesfetModelVariant {
        &self.model
    }

    pub fn node_d(&self) -> &NodeIdentifier {
        &self.node_d
    }

    pub fn node_g(&self) -> &NodeIdentifier {
        &self.node_g
    }

    pub fn node_s(&self) -> &NodeIdentifier {
        &self.node_s
    }

    pub fn area(&self) -> Dimensionless {
        self.area.unwrap_or(1.0.into())
    }

    pub fn off(&self) -> bool {
        self.off.unwrap_or(false)
    }

    pub fn ic_vds(&self) -> Option<Volt> {
        self.ic_vds
    }

    pub fn ic_vgs(&self) -> Option<Volt> {
        self.ic_vgs
    }
}

impl Component for MetalSemiconductorFieldEffectTransistor {
    fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct MesfetStatzModel {
    pub name: String,
    pub nmf: bool,
    pub vto: Volt,
    pub beta: Dimensionless,
    pub b: Dimensionless,
    pub alpha: Dimensionless,
    pub lambda: Dimensionless,
    pub rd: Ohm,
    pub rs: Ohm,
    pub cgs: Farad,
    pub cgd: Farad,
    pub pb: Volt,
    pub kf: Dimensionless,
    pub af: Dimensionless,
    pub fc: Dimensionless,
}

impl Default for MesfetStatzModel {
    fn default() -> Self {
        Self {
            name: "DefaultMesfetStatz".to_string(),
            nmf: true,
            vto: (-2.0).into(),
            beta: 1.0e-4.into(),
            b: 0.3.into(),
            alpha: 2.0.into(),
            lambda: 0.0.into(),
            rd: 0.0.into(),
            rs: 0.0.into(),
            cgs: 0.0.into(),
            cgd: 0.0.into(),
            pb: 1.0.into(),
            kf: 0.0.into(),
            af: 1.0.into(),
            fc: 0.5.into(),
        }
    }
}

impl Model for MesfetStatzModel {
    type ComponentType = MetalSemiconductorFieldEffectTransistor;
}

#[derive(Debug, Clone)]
pub struct MesfetYtterdalModel {
    pub name: String,
    pub nmf: bool,
}

impl Default for MesfetYtterdalModel {
    fn default() -> Self {
        Self {
            name: "DefaultMesfetYtterdal".to_string(),
            nmf: true,
        }
    }
}

impl Model for MesfetYtterdalModel {
    type ComponentType = MetalSemiconductorFieldEffectTransistor;
}

#[derive(Debug, Clone)]
pub struct MesfetHfetModel {
    pub name: String,
    pub nmf: bool,
}

impl Default for MesfetHfetModel {
    fn default() -> Self {
        Self {
            name: "DefaultMesfetHfet".to_string(),
            nmf: true,
        }
    }
}

impl Model for MesfetHfetModel {
    type ComponentType = MetalSemiconductorFieldEffectTransistor;
}