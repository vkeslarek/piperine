use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Model};
use crate::unit::{Ampere, Celsius, Dimensionless, Farad, Henry, Ohm, Second, Volt};
use std::sync::Arc;

#[derive(Clone)]
pub enum JfetModelVariant {
    Level1(Arc<JfetLevel1Model>),
    ParkerSkellern(Arc<JfetParkerSkellernModel>),
}

impl Default for JfetModelVariant {
    fn default() -> Self {
        JfetModelVariant::Level1(Arc::new(JfetLevel1Model::default()))
    }
}

impl Model for JfetModelVariant {
    type ComponentType = JFET;
}

#[derive(Clone)]
pub struct JFET {
    name: String,
    model: JfetModelVariant,

    node_drain: NodeIdentifier,
    node_gate: NodeIdentifier,
    node_source: NodeIdentifier,

    area: Option<Dimensionless>,
    off: Option<bool>,
    ic_vds: Option<Volt>,
    ic_vgs: Option<Volt>,
    temp: Option<Celsius>,
}

impl Component for JFET {
    fn name(&self) -> &String {
        &self.name
    }
}


#[derive(Debug, Clone)]
pub struct JfetLevel1Model {
    pub name: String,
    pub pjf: bool,

    pub vto: Volt,
    pub beta: Dimensionless,
    pub lambda: Dimensionless,
    pub rd: Ohm,
    pub rs: Ohm,
    pub is: Ampere,
    pub b: Dimensionless,

    pub cgs: Farad,
    pub cgd: Farad,
    pub pb: Volt,
    pub fc: Dimensionless,

    pub kf: Dimensionless,
    pub af: Dimensionless,
    pub nlev: Dimensionless,
    pub gdsnoi: Dimensionless,

    pub tnom: Celsius,
    pub tcv: Dimensionless,
    pub vtotc: Dimensionless,
    pub bex: Dimensionless,
    pub betatce: Dimensionless,
    pub xti: Dimensionless,
    pub eg: Volt,
}

impl Default for JfetLevel1Model {
    fn default() -> Self {
        Self {
            name: "DefaultJfetLevel1".to_string(),
            pjf: false,
            vto: (-2.0).into(),
            beta: 1.0e-4.into(),
            lambda: 0.0.into(),
            rd: 0.0.into(),
            rs: 0.0.into(),
            is: 1.0e-14.into(),
            b: 1.0.into(),
            cgs: 0.0.into(),
            cgd: 0.0.into(),
            pb: 1.0.into(),
            fc: 0.5.into(),
            kf: 0.0.into(),
            af: 1.0.into(),
            nlev: 2.0.into(),
            gdsnoi: 1.0.into(),
            tnom: 27.0.into(),
            tcv: 0.0.into(),
            vtotc: 0.0.into(),
            bex: 0.0.into(),
            betatce: 0.0.into(),
            xti: 3.0.into(),
            eg: 1.11.into(),
        }
    }
}

impl Model for JfetLevel1Model {
    type ComponentType = JFET;
}

#[derive(Debug, Clone)]
pub struct JfetParkerSkellernModel {
    pub name: String,
    pub pjf: bool,

    pub id: String,

    pub acgam: Dimensionless,
    pub beta: Dimensionless,
    pub delta: Dimensionless,
    pub mvst: Dimensionless,
    pub n: Dimensionless,
    pub p: Dimensionless,
    pub q: Dimensionless,
    pub vto: Volt,
    pub xc: Dimensionless,
    pub xi: Dimensionless,
    pub z: Dimensionless,

    pub hfeta: Dimensionless,
    pub hfe1: Dimensionless,
    pub hfe2: Dimensionless,
    pub hfgam: Dimensionless,
    pub hfg1: Dimensionless,
    pub hfg2: Dimensionless,
    pub lfgam: Dimensionless,
    pub lfg1: Dimensionless,
    pub lfg2: Dimensionless,

    pub ibd: Ampere,
    pub is: Ampere,
    pub vbd: Volt,
    pub vbi: Volt,
    pub vst: Volt,

    pub rs: Ohm,
    pub rd: Ohm,
    pub rg: Ohm,
    pub lg: Henry,
    pub ls: Henry,
    pub ld: Henry,

    pub cgd: Farad,
    pub cgs: Farad,
    pub fc: Dimensionless,
    pub cdss: Farad,
    pub taud: Second,
    pub taug: Second,

    pub afac: Dimensionless,
    pub nfing: Dimensionless,

    pub tnom: Celsius,
}

impl Default for JfetParkerSkellernModel {
    fn default() -> Self {
        Self {
            name: "DefaultJfetParkerSkellern".to_string(),
            pjf: false,
            id: "PF1".to_string(),
            acgam: 0.0.into(),
            beta: 1.0e-4.into(),
            delta: 0.0.into(),
            mvst: 0.0.into(),
            n: 1.0.into(),
            p: 2.0.into(),
            q: 2.0.into(),
            vto: (-2.0).into(),
            xc: 0.0.into(),
            xi: 1000.0.into(),
            z: 0.5.into(),
            hfeta: 0.0.into(),
            hfe1: 0.0.into(),
            hfe2: 0.0.into(),
            hfgam: 0.0.into(),
            hfg1: 0.0.into(),
            hfg2: 0.0.into(),
            lfgam: 0.0.into(),
            lfg1: 0.0.into(),
            lfg2: 0.0.into(),
            ibd: 0.0.into(),
            is: 1.0e-14.into(),
            vbd: 0.0.into(),
            vbi: 1.0.into(),
            vst: 0.0.into(),
            rs: 0.0.into(),
            rd: 0.0.into(),
            rg: 0.0.into(),
            lg: 0.0.into(),
            ls: 0.0.into(),
            ld: 0.0.into(),
            cgd: 0.0.into(),
            cgs: 0.0.into(),
            fc: 0.5.into(),
            cdss: 0.0.into(),
            taud: 0.0.into(),
            taug: 0.0.into(),
            afac: 1.0.into(),
            nfing: 1.0.into(),
            tnom: 26.85.into(),
        }
    }
}

impl Model for JfetParkerSkellernModel {
    type ComponentType = JFET;
}