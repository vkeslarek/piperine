use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Dynamic, Model};
use crate::unit::{Celsius, Dimensionless, Meter, Ohm};
use std::sync::Arc;

#[derive(Clone)]
pub struct Resistor {
    name: String,
    model: Arc<ResistorModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,

    resistance: Option<Dynamic<Ohm>>,
    ac: Option<Ohm>,
    multiplier: Option<Dimensionless>,
    scale: Option<Dimensionless>,
    temp: Option<Celsius>,
    delta_temp: Option<Celsius>,
    tc1: Option<Dimensionless>,
    tc2: Option<Dimensionless>,
    tce: Option<Dimensionless>,
    noisy: Option<bool>,

    length: Option<Meter>,
    width: Option<Meter>,
}

impl Resistor {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        resistance: impl Into<Option<Dynamic<Ohm>>>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(ResistorModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            resistance: resistance.into(),
            ac: None,
            multiplier: None,
            scale: None,
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            tce: None,
            noisy: None,
            length: None,
            width: None,
        }
    }

    pub fn with_model(&mut self, model: Arc<ResistorModel>) -> &mut Resistor {
        self.model = model;
        self
    }

    pub fn with_ac_resistance(&mut self, ac: impl Into<Ohm>) -> &mut Resistor {
        self.ac = Some(ac.into());
        self
    }

    pub fn with_width(&mut self, width: impl Into<Meter>) -> &mut Resistor {
        self.width = Some(width.into());
        self
    }

    pub fn with_length(&mut self, length: impl Into<Meter>) -> &mut Resistor {
        self.length = Some(length.into());
        self
    }

    pub fn with_scale(&mut self, scale: impl Into<Dimensionless>) -> &mut Resistor {
        self.scale = Some(scale.into());
        self
    }

    pub fn with_multiplier(&mut self, multiplier: impl Into<Dimensionless>) -> &mut Resistor {
        self.multiplier = Some(multiplier.into());
        self
    }

    pub fn with_temp(&mut self, temp: impl Into<Celsius>) -> &mut Resistor {
        self.temp = Some(temp.into());
        self
    }

    pub fn with_delta_temp(&mut self, delta_temp: impl Into<Celsius>) -> &mut Resistor {
        self.delta_temp = Some(delta_temp.into());
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Resistor {
        self.tc1 = Some(tc1.into());
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Resistor {
        self.tc2 = Some(tc2.into());
        self
    }

    pub fn with_tce(&mut self, tce: impl Into<Dimensionless>) -> &mut Resistor {
        self.tce = Some(tce.into());
        self
    }

    pub fn with_noise(&mut self, enable: impl Into<bool>) -> &mut Resistor {
        self.noisy = Some(enable.into());
        self
    }

    pub fn model(&self) -> &Arc<ResistorModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn resistance(&self) -> Option<&Dynamic<Ohm>> {
        self.resistance.as_ref()
    }

    pub fn ac(&self) -> Option<Ohm> {
        self.ac
    }

    pub fn multiplier(&self) -> Option<Dimensionless> {
        self.multiplier
    }

    pub fn scale(&self) -> Option<Dimensionless> {
        self.scale
    }

    pub fn width(&self) -> Option<Meter> {
        self.width
    }

    pub fn length(&self) -> Option<Meter> {
        self.length
    }

    pub fn temp(&self) -> Option<Celsius> {
        self.temp
    }

    pub fn delta_temp(&self) -> Option<Celsius> {
        self.delta_temp
    }

    pub fn tc1(&self) -> Option<Dimensionless> {
        self.tc1
    }

    pub fn tc2(&self) -> Option<Dimensionless> {
        self.tc2
    }

    pub fn tce(&self) -> Option<Dimensionless> {
        self.tce
    }

    pub fn noisy(&self) -> Option<bool> {
        self.noisy
    }
}

impl Component for Resistor {
    fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug)]
pub struct ResistorModel {
    pub name: String,
    pub tc1: Dimensionless,
    pub tc2: Dimensionless,
    pub tce: Dimensionless,
    pub sheet_res: Ohm,
    pub def_width: Meter,
    pub def_length: Meter,
    pub narrow: Meter,
    pub short: Meter,
    pub tnom: Celsius,

    pub kf: Dimensionless,
    pub af: Dimensionless,
    pub wf: Dimensionless,
    pub lf: Dimensionless,
    pub ef: Dimensionless,
}

impl Default for ResistorModel {
    fn default() -> Self {
        Self {
            name: "DefaultResistorModel".to_string(),
            tnom: 27.0.into(),
            tc1: 0.0.into(),
            tc2: 0.0.into(),
            tce: 0.0.into(),
            sheet_res: 0.0.into(),
            def_width: 10.0.into(),
            def_length: 10.0.into(),
            narrow: 0.0.into(),
            short: 0.0.into(),
            lf: 1.0.into(),
            wf: 1.0.into(),
            ef: 1.0.into(),
            kf: 1.0.into(),
            af: 1.0.into(),
        }
    }
}

impl ResistorModel {
    pub fn with_tnom(&mut self, tnom: impl Into<Celsius>) -> &mut Self {
        self.tnom = tnom.into();
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = tc1.into();
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = tc2.into();
        self
    }

    pub fn with_tce(&mut self, tce: impl Into<Dimensionless>) -> &mut Self {
        self.tce = tce.into();
        self
    }

    pub fn with_sheet_resistivity(&mut self, sheet_res: impl Into<Ohm>) -> &mut Self {
        self.sheet_res = sheet_res.into();
        self
    }

    pub fn with_default_width(&mut self, def_width: impl Into<Meter>) -> &mut Self {
        self.def_width = def_width.into();
        self
    }

    pub fn with_default_length(&mut self, def_length: impl Into<Meter>) -> &mut Self {
        self.def_length = def_length.into();
        self
    }

    pub fn with_narrow(&mut self, narrow: impl Into<Meter>) -> &mut Self {
        self.narrow = narrow.into();
        self
    }

    pub fn with_short(&mut self, short: impl Into<Meter>) -> &mut Self {
        self.short = short.into();
        self
    }

    pub fn with_kf(&mut self, kf: impl Into<Dimensionless>) -> &mut Self {
        self.kf = kf.into();
        self
    }

    pub fn with_af(&mut self, af: impl Into<Dimensionless>) -> &mut Self {
        self.af = af.into();
        self
    }

    pub fn with_wf(&mut self, wf: impl Into<Dimensionless>) -> &mut Self {
        self.wf = wf.into();
        self
    }

    pub fn with_lf(&mut self, lf: impl Into<Dimensionless>) -> &mut Self {
        self.lf = lf.into();
        self
    }

    pub fn with_ef(&mut self, ef: impl Into<Dimensionless>) -> &mut Self {
        self.ef = ef.into();
        self
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn tc1(&self) -> Dimensionless {
        self.tc1
    }

    pub fn tc2(&self) -> Dimensionless {
        self.tc2
    }

    pub fn tce(&self) -> Dimensionless {
        self.tce
    }

    pub fn sheet_res(&self) -> Ohm {
        self.sheet_res
    }

    pub fn def_width(&self) -> Meter {
        self.def_width
    }

    pub fn def_length(&self) -> Meter {
        self.def_length
    }

    pub fn narrow(&self) -> Meter {
        self.narrow
    }

    pub fn short(&self) -> Meter {
        self.short
    }

    pub fn tnom(&self) -> Celsius {
        self.tnom
    }

    pub fn kf(&self) -> Dimensionless {
        self.kf
    }

    pub fn af(&self) -> Dimensionless {
        self.af
    }

    pub fn wf(&self) -> Dimensionless {
        self.wf
    }

    pub fn lf(&self) -> Dimensionless {
        self.lf
    }

    pub fn ef(&self) -> Dimensionless {
        self.ef
    }
}

impl Model for ResistorModel {
    type ComponentType = Resistor;
}
