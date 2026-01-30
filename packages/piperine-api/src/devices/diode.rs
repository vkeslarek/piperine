use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Model};
use crate::unit::{Ampere, Celsius, Dimensionless, Farad, Meter, Ohm, Second, Volt};
use std::sync::Arc;

#[derive(Clone)]
pub struct Diode {
    name: String,
    model: Arc<DiodeModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    area: Option<Dimensionless>,
    multiplier: Option<Dimensionless>,
    pj: Option<Meter>,
    off: Option<bool>,
    ic: Option<Volt>,
    temp: Option<Celsius>,
    dtemp: Option<Celsius>,
    lm: Option<Meter>,
    wm: Option<Meter>,
    lp: Option<Meter>,
    wp: Option<Meter>,
}

impl Diode {
    pub fn new(
        name: impl Into<String>,
        node_anode: impl Into<NodeIdentifier>,
        node_cathode: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(DiodeModel::default()),
            node_plus: node_anode.into(),
            node_minus: node_cathode.into(),
            area: None,
            multiplier: None,
            pj: None,
            off: None,
            ic: None,
            temp: None,
            dtemp: None,
            lm: None,
            wm: None,
            lp: None,
            wp: None,
        }
    }

    pub fn with_model(&mut self, model: Arc<DiodeModel>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_area(&mut self, area: impl Into<Dimensionless>) -> &mut Self {
        self.area = Some(area.into());
        self
    }

    pub fn with_multiplier(&mut self, m: impl Into<Dimensionless>) -> &mut Self {
        self.multiplier = Some(m.into());
        self
    }

    pub fn with_perimeter_junction(&mut self, pj: impl Into<Meter>) -> &mut Self {
        self.pj = Some(pj.into());
        self
    }

    pub fn with_off(&mut self, off: impl Into<bool>) -> &mut Self {
        self.off = Some(off.into());
        self
    }

    pub fn with_initial_condition(&mut self, ic: impl Into<Volt>) -> &mut Self {
        self.ic = Some(ic.into());
        self
    }

    pub fn with_temp(&mut self, temp: impl Into<Celsius>) -> &mut Self {
        self.temp = Some(temp.into());
        self
    }

    pub fn with_dtemp(&mut self, dtemp: impl Into<Celsius>) -> &mut Self {
        self.dtemp = Some(dtemp.into());
        self
    }

    pub fn with_lm(&mut self, lm: impl Into<Meter>) -> &mut Self {
        self.lm = Some(lm.into());
        self
    }

    pub fn with_wm(&mut self, wm: impl Into<Meter>) -> &mut Self {
        self.wm = Some(wm.into());
        self
    }

    pub fn with_lp(&mut self, lp: impl Into<Meter>) -> &mut Self {
        self.lp = Some(lp.into());
        self
    }

    pub fn with_wp(&mut self, wp: impl Into<Meter>) -> &mut Self {
        self.wp = Some(wp.into());
        self
    }

    pub fn model(&self) -> &Arc<DiodeModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn area(&self) -> Dimensionless {
        self.area.unwrap_or(1.0.into())
    }

    pub fn multiplier(&self) -> Dimensionless {
        self.multiplier.unwrap_or(1.0.into())
    }

    pub fn perimeter_junction(&self) -> Meter {
        self.pj.unwrap_or(0.0.into())
    }

    pub fn off(&self) -> bool {
        self.off.unwrap_or(false)
    }

    pub fn initial_condition(&self) -> Option<Volt> {
        self.ic
    }

    pub fn temp(&self) -> Option<Celsius> {
        self.temp
    }

    pub fn dtemp(&self) -> Option<Celsius> {
        self.dtemp
    }

    pub fn lm(&self) -> Option<Meter> {
        self.lm
    }

    pub fn wm(&self) -> Option<Meter> {
        self.wm
    }

    pub fn lp(&self) -> Option<Meter> {
        self.lp
    }

    pub fn wp(&self) -> Option<Meter> {
        self.wp
    }
}

impl Component for Diode {
    fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug)]
pub struct DiodeModel {
    pub name: String,
    pub is: Ampere,
    pub jsw: Ampere,
    pub n: Dimensionless,
    pub rs: Ohm,
    pub bv: Volt,
    pub ibv: Ampere,
    pub nbv: Dimensionless,
    pub ikf: Ampere,
    pub ikr: Ampere,
    pub jtun: Ampere,
    pub jtunsw: Ampere,
    pub ntun: Dimensionless,
    pub xtitun: Dimensionless,
    pub keg: Dimensionless,
    pub isr: Ampere,
    pub nr: Dimensionless,
    pub cjo: Farad,
    pub cjp: Farad,
    pub fc: Dimensionless,
    pub fcs: Dimensionless,
    pub m: Dimensionless,
    pub mjsw: Dimensionless,
    pub vj: Volt,
    pub php: Volt,
    pub tt: Second,
    pub eg: Volt,
    pub xti: Dimensionless,
    pub tnom: Celsius,
    pub trs1: Dimensionless,
    pub trs2: Dimensionless,
    pub tm1: Dimensionless,
    pub tm2: Dimensionless,
    pub ttt1: Dimensionless,
    pub ttt2: Dimensionless,
    pub kf: Dimensionless,
    pub af: Dimensionless,
}

impl Default for DiodeModel {
    fn default() -> Self {
        Self {
            name: "DefaultDiodeModel".to_string(),
            is: 1.0e-14.into(),
            jsw: 0.0.into(),
            n: 1.0.into(),
            rs: 0.0.into(),
            bv: f64::INFINITY.into(),
            ibv: 1.0e-3.into(),
            nbv: 1.0.into(),
            ikf: f64::INFINITY.into(),
            ikr: f64::INFINITY.into(),
            jtun: 0.0.into(),
            jtunsw: 0.0.into(),
            ntun: 30.0.into(),
            xtitun: 3.0.into(),
            keg: 1.0.into(),
            isr: 0.0.into(),
            nr: 2.0.into(),
            cjo: 0.0.into(),
            cjp: 0.0.into(),
            fc: 0.5.into(),
            fcs: 0.5.into(),
            m: 0.5.into(),
            mjsw: 0.33.into(),
            vj: 1.0.into(),
            php: 1.0.into(),
            tt: 0.0.into(),
            eg: 1.11.into(),
            xti: 3.0.into(),
            tnom: 27.0.into(),
            trs1: 0.0.into(),
            trs2: 0.0.into(),
            tm1: 0.0.into(),
            tm2: 0.0.into(),
            ttt1: 0.0.into(),
            ttt2: 0.0.into(),
            kf: 0.0.into(),
            af: 1.0.into(),
        }
    }
}

impl DiodeModel {
    pub fn with_is(&mut self, is: impl Into<Ampere>) -> &mut Self {
        self.is = is.into();
        self
    }
    pub fn with_jsw(&mut self, jsw: impl Into<Ampere>) -> &mut Self {
        self.jsw = jsw.into();
        self
    }
    pub fn with_n(&mut self, n: impl Into<Dimensionless>) -> &mut Self {
        self.n = n.into();
        self
    }
    pub fn with_rs(&mut self, rs: impl Into<Ohm>) -> &mut Self {
        self.rs = rs.into();
        self
    }
    pub fn with_bv(&mut self, bv: impl Into<Volt>) -> &mut Self {
        self.bv = bv.into();
        self
    }
    pub fn with_ibv(&mut self, ibv: impl Into<Ampere>) -> &mut Self {
        self.ibv = ibv.into();
        self
    }
    pub fn with_nbv(&mut self, nbv: impl Into<Dimensionless>) -> &mut Self {
        self.nbv = nbv.into();
        self
    }
    pub fn with_ikf(&mut self, ikf: impl Into<Ampere>) -> &mut Self {
        self.ikf = ikf.into();
        self
    }
    pub fn with_ikr(&mut self, ikr: impl Into<Ampere>) -> &mut Self {
        self.ikr = ikr.into();
        self
    }
    pub fn with_isr(&mut self, isr: impl Into<Ampere>) -> &mut Self {
        self.isr = isr.into();
        self
    }
    pub fn with_nr(&mut self, nr: impl Into<Dimensionless>) -> &mut Self {
        self.nr = nr.into();
        self
    }
    pub fn with_cjo(&mut self, cjo: impl Into<Farad>) -> &mut Self {
        self.cjo = cjo.into();
        self
    }
    pub fn with_cjp(&mut self, cjp: impl Into<Farad>) -> &mut Self {
        self.cjp = cjp.into();
        self
    }
    pub fn with_fc(&mut self, fc: impl Into<Dimensionless>) -> &mut Self {
        self.fc = fc.into();
        self
    }
    pub fn with_fcs(&mut self, fcs: impl Into<Dimensionless>) -> &mut Self {
        self.fcs = fcs.into();
        self
    }
    pub fn with_m(&mut self, m: impl Into<Dimensionless>) -> &mut Self {
        self.m = m.into();
        self
    }
    pub fn with_mjsw(&mut self, mjsw: impl Into<Dimensionless>) -> &mut Self {
        self.mjsw = mjsw.into();
        self
    }
    pub fn with_vj(&mut self, vj: impl Into<Volt>) -> &mut Self {
        self.vj = vj.into();
        self
    }
    pub fn with_php(&mut self, php: impl Into<Volt>) -> &mut Self {
        self.php = php.into();
        self
    }
    pub fn with_tt(&mut self, tt: impl Into<Second>) -> &mut Self {
        self.tt = tt.into();
        self
    }
    pub fn with_eg(&mut self, eg: impl Into<Volt>) -> &mut Self {
        self.eg = eg.into();
        self
    }
    pub fn with_xti(&mut self, xti: impl Into<Dimensionless>) -> &mut Self {
        self.xti = xti.into();
        self
    }
    pub fn with_tnom(&mut self, tnom: impl Into<Celsius>) -> &mut Self {
        self.tnom = tnom.into();
        self
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn is(&self) -> Ampere {
        self.is
    }
    pub fn jsw(&self) -> Ampere {
        self.jsw
    }
    pub fn n(&self) -> Dimensionless {
        self.n
    }
    pub fn rs(&self) -> Ohm {
        self.rs
    }
    pub fn bv(&self) -> Volt {
        self.bv
    }
    pub fn ibv(&self) -> Ampere {
        self.ibv
    }
    pub fn nbv(&self) -> Dimensionless {
        self.nbv
    }
    pub fn ikf(&self) -> Ampere {
        self.ikf
    }
    pub fn ikr(&self) -> Ampere {
        self.ikr
    }
    pub fn jtun(&self) -> Ampere {
        self.jtun
    }
    pub fn jtunsw(&self) -> Ampere {
        self.jtunsw
    }
    pub fn ntun(&self) -> Dimensionless {
        self.ntun
    }
    pub fn xtitun(&self) -> Dimensionless {
        self.xtitun
    }
    pub fn keg(&self) -> Dimensionless {
        self.keg
    }
    pub fn isr(&self) -> Ampere {
        self.isr
    }
    pub fn nr(&self) -> Dimensionless {
        self.nr
    }
    pub fn cjo(&self) -> Farad {
        self.cjo
    }
    pub fn cjp(&self) -> Farad {
        self.cjp
    }
    pub fn fc(&self) -> Dimensionless {
        self.fc
    }
    pub fn fcs(&self) -> Dimensionless {
        self.fcs
    }
    pub fn m(&self) -> Dimensionless {
        self.m
    }
    pub fn mjsw(&self) -> Dimensionless {
        self.mjsw
    }
    pub fn vj(&self) -> Volt {
        self.vj
    }
    pub fn php(&self) -> Volt {
        self.php
    }
    pub fn tt(&self) -> Second {
        self.tt
    }
    pub fn eg(&self) -> Volt {
        self.eg
    }
    pub fn xti(&self) -> Dimensionless {
        self.xti
    }
    pub fn tnom(&self) -> Celsius {
        self.tnom
    }
    pub fn trs1(&self) -> Dimensionless {
        self.trs1
    }
    pub fn trs2(&self) -> Dimensionless {
        self.trs2
    }
    pub fn tm1(&self) -> Dimensionless {
        self.tm1
    }
    pub fn tm2(&self) -> Dimensionless {
        self.tm2
    }
    pub fn ttt1(&self) -> Dimensionless {
        self.ttt1
    }
    pub fn ttt2(&self) -> Dimensionless {
        self.ttt2
    }
    pub fn kf(&self) -> Dimensionless {
        self.kf
    }
    pub fn af(&self) -> Dimensionless {
        self.af
    }
}

impl Model for DiodeModel {
    type ComponentType = Diode;
}
