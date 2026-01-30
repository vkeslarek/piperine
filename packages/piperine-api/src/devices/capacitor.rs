use crate::circuit::netlist::NodeIdentifier;
use crate::devices::{Component, Dynamic, Model};
use crate::unit::{
    Celsius, Dimensionless, Farad, FaradPerMeter, FaradPerMeterSquared, Meter, UnitExt, Volt,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct Capacitor {
    name: String,
    model: Arc<CapacitorModel>,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    value: Option<Dynamic<Farad>>,
    multiplier: Option<Dimensionless>,
    scale: Option<Dimensionless>,
    width: Option<Meter>,
    length: Option<Meter>,
    temp: Option<Celsius>,
    delta_temp: Option<Celsius>,
    tc1: Option<Dimensionless>,
    tc2: Option<Dimensionless>,
    ic: Option<Volt>,
}

impl Capacitor {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        value: impl Into<Option<Dynamic<Farad>>>,
    ) -> Self {
        Self {
            name: name.into(),
            model: Arc::new(CapacitorModel::default()),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            value: value.into(),
            multiplier: None,
            scale: None,
            width: None,
            length: None,
            temp: None,
            delta_temp: None,
            tc1: None,
            tc2: None,
            ic: None,
        }
    }
    pub fn with_model(&mut self, model: Arc<CapacitorModel>) -> &mut Self {
        self.model = model;
        self
    }

    pub fn with_value(&mut self, value: impl Into<Dynamic<Farad>>) -> &mut Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_width(&mut self, width: impl Into<Meter>) -> &mut Self {
        self.width = Some(width.into());
        self
    }

    pub fn with_length(&mut self, length: impl Into<Meter>) -> &mut Self {
        self.length = Some(length.into());
        self
    }

    pub fn with_multiplier(&mut self, m: impl Into<Dimensionless>) -> &mut Self {
        self.multiplier = Some(m.into());
        self
    }

    pub fn with_scale(&mut self, scale: impl Into<Dimensionless>) -> &mut Self {
        self.scale = Some(scale.into());
        self
    }

    pub fn with_temp(&mut self, temp: impl Into<Celsius>) -> &mut Self {
        self.temp = Some(temp.into());
        self
    }

    pub fn with_delta_temp(&mut self, dtemp: impl Into<Celsius>) -> &mut Self {
        self.delta_temp = Some(dtemp.into());
        self
    }

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = Some(tc1.into());
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = Some(tc2.into());
        self
    }

    pub fn with_initial_condition(&mut self, ic: impl Into<Volt>) -> &mut Self {
        self.ic = Some(ic.into());
        self
    }

    pub fn model(&self) -> &Arc<CapacitorModel> {
        &self.model
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn value(&self) -> Option<&Dynamic<Farad>> {
        self.value.as_ref()
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

    pub fn initial_condition(&self) -> Option<Volt> {
        self.ic
    }
}

impl Component for Capacitor {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CapacitorModel {
    name: String,
    cap: Farad,
    cj: FaradPerMeterSquared,
    cjsw: FaradPerMeterSquared,
    default_width: Meter,
    default_length: Meter,
    narrow: Meter,
    short: Meter,
    tc1: Dimensionless,
    tc2: Dimensionless,
    tnom: Celsius,
    dielectric_constant: FaradPerMeter,
    thickness: Meter,
}

impl Default for CapacitorModel {
    fn default() -> Self {
        Self {
            name: "DefaultCapacitorModel".to_string(),
            cap: 0.0,
            cj: 0.0,
            cjsw: 0.0,
            default_width: 1.0.um(),
            default_length: 0.0,
            narrow: 0.0,
            short: 0.0,
            tc1: 0.0,
            tc2: 0.0,
            tnom: 27.0.deg_C(),
            dielectric_constant: 0.0,
            thickness: 0.0,
        }
    }
}

impl CapacitorModel {
    pub fn with_cap(&mut self, cap: impl Into<Farad>) -> &mut Self {
        self.cap = cap.into();
        self
    }

    pub fn with_cj(&mut self, cj: impl Into<FaradPerMeterSquared>) -> &mut Self {
        self.cj = cj.into();
        self
    }

    pub fn with_cjsw(&mut self, cjsw: impl Into<FaradPerMeter>) -> &mut Self {
        self.cjsw = cjsw.into();
        self
    }

    pub fn with_default_width(&mut self, w: impl Into<Meter>) -> &mut Self {
        self.default_width = w.into();
        self
    }

    pub fn with_default_length(&mut self, l: impl Into<Meter>) -> &mut Self {
        self.default_length = l.into();
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

    pub fn with_tc1(&mut self, tc1: impl Into<Dimensionless>) -> &mut Self {
        self.tc1 = tc1.into();
        self
    }

    pub fn with_tc2(&mut self, tc2: impl Into<Dimensionless>) -> &mut Self {
        self.tc2 = tc2.into();
        self
    }

    pub fn with_tnom(&mut self, tnom: impl Into<Celsius>) -> &mut Self {
        self.tnom = tnom.into();
        self
    }

    pub fn with_dielectric_constant(&mut self, di: impl Into<Dimensionless>) -> &mut Self {
        self.dielectric_constant = di.into();
        self
    }

    pub fn with_thickness(&mut self, thick: impl Into<Meter>) -> &mut Self {
        self.thickness = thick.into();
        self
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn cap(&self) -> Farad {
        self.cap
    }

    pub fn cj(&self) -> FaradPerMeterSquared {
        self.cj
    }

    pub fn cjsw(&self) -> FaradPerMeterSquared {
        self.cjsw
    }

    pub fn default_width(&self) -> Meter {
        self.default_width
    }

    pub fn default_length(&self) -> Meter {
        self.default_length
    }

    pub fn narrow(&self) -> Meter {
        self.narrow
    }

    pub fn short(&self) -> Meter {
        self.short
    }

    pub fn tc1(&self) -> Dimensionless {
        self.tc1
    }

    pub fn tc2(&self) -> Dimensionless {
        self.tc2
    }

    pub fn tnom(&self) -> Celsius {
        self.tnom
    }

    pub fn dielectric_constant(&self) -> FaradPerMeter {
        self.dielectric_constant
    }

    pub fn thickness(&self) -> Meter {
        self.thickness
    }
}

impl Model for CapacitorModel {
    type ComponentType = Capacitor;
}
