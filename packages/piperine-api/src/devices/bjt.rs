use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Model};
use crate::unit::{Ampere, Celsius, Dimensionless, Farad, Ohm, Radian, Second, Volt};
use std::sync::Arc;

#[derive(Clone)]
pub enum BipolarModelVariant {
    GummelPoon(Arc<GummelPoonModel>),
    Vbic(Arc<VbicModel>),
    HicumL2(Arc<HicumL2Model>),
}

impl Default for BipolarModelVariant {
    fn default() -> Self {
        BipolarModelVariant::GummelPoon(Arc::new(GummelPoonModel::default()))
    }
}

impl Model for BipolarModelVariant {
    type ComponentType = BJT;
}

#[derive(Clone)]
pub struct BJT {
    name: String,
    model: BipolarModelVariant,

    node_collector: NodeIdentifier,
    node_base: NodeIdentifier,
    node_emitter: NodeIdentifier,
    node_s: Option<NodeIdentifier>,
    node_t: Option<NodeIdentifier>,

    area: Option<Dimensionless>,
    areab: Option<Dimensionless>,
    areac: Option<Dimensionless>,
    multiplier: Option<Dimensionless>,
    off: Option<bool>,
    ic_vbe: Option<Volt>,
    ic_vce: Option<Volt>,
    temp: Option<Celsius>,
    dtemp: Option<Celsius>,
}

impl Component for BJT {
    fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct GummelPoonModel {
    pub name: String,
    pub pnp: bool,

    pub subs: Dimensionless,
    pub is: Ampere,
    pub ibe: Ampere,
    pub ibc: Ampere,
    pub iss: Ampere,
    pub bf: Dimensionless,
    pub nf: Dimensionless,
    pub vaf: Volt,
    pub ikf: Ampere,
    pub nkf: Dimensionless,
    pub ise: Ampere,
    pub ne: Dimensionless,
    pub br: Dimensionless,
    pub nr: Dimensionless,
    pub var: Volt,
    pub ikr: Ampere,
    pub isc: Ampere,
    pub nc: Dimensionless,
    pub rb: Ohm,
    pub irb: Ampere,
    pub rbm: Ohm,
    pub re: Ohm,
    pub rc: Ohm,
    pub cje: Farad,
    pub vje: Volt,
    pub mje: Dimensionless,
    pub tf: Second,
    pub xtf: Dimensionless,
    pub vtf: Volt,
    pub itf: Ampere,
    pub ptf: Radian,
    pub cjc: Farad,
    pub vjc: Volt,
    pub mjc: Dimensionless,
    pub xcjc: Dimensionless,
    pub tr: Second,
    pub cjs: Farad,
    pub vjs: Volt,
    pub mjs: Dimensionless,
    pub xtb: Dimensionless,
    pub eg: Volt,
    pub xti: Dimensionless,
    pub kf: Dimensionless,
    pub af: Dimensionless,
    pub fc: Dimensionless,
    pub tnom: Celsius,
    
    pub rco: Ohm,
    pub vo: Volt,
    pub gamma: Dimensionless,
    pub qco: Dimensionless,
    pub vg: Volt,
    pub cn: Dimensionless,
    pub d: Dimensionless,

    pub tlev: Dimensionless,
    pub tlevc: Dimensionless,
    pub tre1: Dimensionless,
    pub tre2: Dimensionless,
    pub trc1: Dimensionless,
    pub trc2: Dimensionless,
    pub trb1: Dimensionless,
    pub trb2: Dimensionless,
    pub trbm1: Dimensionless,
    pub trbm2: Dimensionless,
    pub tbf1: Dimensionless,
    pub tbf2: Dimensionless,
    pub tbr1: Dimensionless,
    pub tbr2: Dimensionless,
    pub tikf1: Dimensionless,
    pub tikf2: Dimensionless,
    pub tikr1: Dimensionless,
    pub tikr2: Dimensionless,
    pub tirb1: Dimensionless,
    pub tirb2: Dimensionless,
    pub tnc1: Dimensionless,
    pub tnc2: Dimensionless,
    pub tne1: Dimensionless,
    pub tne2: Dimensionless,
    pub tnf1: Dimensionless,
    pub tnf2: Dimensionless,
    pub tnr1: Dimensionless,
    pub tnr2: Dimensionless,
    pub tvaf1: Dimensionless,
    pub tvaf2: Dimensionless,
    pub tvar1: Dimensionless,
    pub tvar2: Dimensionless,
    pub ctc: Dimensionless,
    pub cte: Dimensionless,
    pub cts: Dimensionless,
    pub tvjc: Dimensionless,
    pub tvje: Dimensionless,
    pub titf1: Dimensionless,
    pub titf2: Dimensionless,
    pub ttf1: Dimensionless,
    pub ttf2: Dimensionless,
    pub ttr1: Dimensionless,
    pub ttr2: Dimensionless,
    pub tmje1: Dimensionless,
    pub tmje2: Dimensionless,
    pub tmjc1: Dimensionless,
    pub tmjc2: Dimensionless,
}

impl Default for GummelPoonModel {
    fn default() -> Self {
        Self {
            name: "DefaultGummelPoon".to_string(),
            pnp: false,
            subs: 1.0.into(),
            is: 1.0e-16.into(),
            ibe: 0.0.into(),
            ibc: 0.0.into(),
            iss: 0.0.into(),
            bf: 100.0.into(),
            nf: 1.0.into(),
            vaf: f64::INFINITY.into(),
            ikf: f64::INFINITY.into(),
            nkf: 0.5.into(),
            ise: 0.0.into(),
            ne: 1.5.into(),
            br: 1.0.into(),
            nr: 1.0.into(),
            var: f64::INFINITY.into(),
            ikr: f64::INFINITY.into(),
            isc: 0.0.into(),
            nc: 2.0.into(),
            rb: 0.0.into(),
            irb: f64::INFINITY.into(),
            rbm: 0.0.into(),
            re: 0.0.into(),
            rc: 0.0.into(),
            cje: 0.0.into(),
            vje: 0.75.into(),
            mje: 0.33.into(),
            tf: 0.0.into(),
            xtf: 0.0.into(),
            vtf: f64::INFINITY.into(),
            itf: 0.0.into(),
            ptf: 0.0.into(),
            cjc: 0.0.into(),
            vjc: 0.75.into(),
            mjc: 0.33.into(),
            xcjc: 1.0.into(),
            tr: 0.0.into(),
            cjs: 0.0.into(),
            vjs: 0.75.into(),
            mjs: 0.0.into(),
            xtb: 0.0.into(),
            eg: 1.11.into(),
            xti: 3.0.into(),
            kf: 0.0.into(),
            af: 1.0.into(),
            fc: 0.5.into(),
            tnom: 27.0.into(),
            rco: 0.0.into(),
            vo: 10.0.into(),
            gamma: 1e-11.into(),
            qco: 0.0.into(),
            vg: 1.206.into(),
            cn: 2.42.into(),
            d: 0.87.into(),
            tlev: 0.0.into(),
            tlevc: 0.0.into(),
            tre1: 0.0.into(),
            tre2: 0.0.into(),
            trc1: 0.0.into(),
            trc2: 0.0.into(),
            trb1: 0.0.into(),
            trb2: 0.0.into(),
            trbm1: 0.0.into(),
            trbm2: 0.0.into(),
            tbf1: 0.0.into(),
            tbf2: 0.0.into(),
            tbr1: 0.0.into(),
            tbr2: 0.0.into(),
            tikf1: 0.0.into(),
            tikf2: 0.0.into(),
            tikr1: 0.0.into(),
            tikr2: 0.0.into(),
            tirb1: 0.0.into(),
            tirb2: 0.0.into(),
            tnc1: 0.0.into(),
            tnc2: 0.0.into(),
            tne1: 0.0.into(),
            tne2: 0.0.into(),
            tnf1: 0.0.into(),
            tnf2: 0.0.into(),
            tnr1: 0.0.into(),
            tnr2: 0.0.into(),
            tvaf1: 0.0.into(),
            tvaf2: 0.0.into(),
            tvar1: 0.0.into(),
            tvar2: 0.0.into(),
            ctc: 0.0.into(),
            cte: 0.0.into(),
            cts: 0.0.into(),
            tvjc: 0.0.into(),
            tvje: 0.0.into(),
            titf1: 0.0.into(),
            titf2: 0.0.into(),
            ttf1: 0.0.into(),
            ttf2: 0.0.into(),
            ttr1: 0.0.into(),
            ttr2: 0.0.into(),
            tmje1: 0.0.into(),
            tmje2: 0.0.into(),
            tmjc1: 0.0.into(),
            tmjc2: 0.0.into(),
        }
    }
}

impl Model for GummelPoonModel {
    type ComponentType = BJT;
}

#[derive(Debug, Clone)]
pub struct VbicModel {
    pub name: String,
    pub pnp: bool,

    pub tnom: Celsius,
    pub rc: Ohm,
    pub rb: Ohm,
    pub re: Ohm,

    pub selft: Dimensionless,
    pub rth: Ohm,
    pub cth: Farad,
}

impl Default for VbicModel {
    fn default() -> Self {
        Self {
            name: "DefaultVbic".to_string(),
            pnp: false,
            tnom: 27.0.into(),
            rc: 0.0.into(),
            rb: 0.0.into(),
            re: 0.0.into(),
            selft: 0.0.into(),
            rth: 0.0.into(),
            cth: 0.0.into(),
        }
    }
}

impl Model for VbicModel {
    type ComponentType = BJT;
}

#[derive(Debug, Clone)]
pub struct HicumL2Model {
    pub name: String,
    pub pnp: bool,

    pub tnom: Celsius,
    pub c10: Farad,
    pub qp0: Ampere,
    pub ich: Ampere,

    pub flsh: Dimensionless,
    pub rth: Ohm,
    pub cth: Farad,
}

impl Default for HicumL2Model {
    fn default() -> Self {
        Self {
            name: "DefaultHicumL2".to_string(),
            pnp: false,
            tnom: 27.0.into(),
            c10: 0.0.into(),
            qp0: 0.0.into(),
            ich: 0.0.into(),
            flsh: 0.0.into(),
            rth: 0.0.into(),
            cth: 0.0.into(),
        }
    }
}

impl Model for HicumL2Model {
    type ComponentType = BJT;
}
