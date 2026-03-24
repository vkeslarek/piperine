use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Celsius, Dimensionless, Meter, Volt};
use std::sync::Arc;

/// Junction diode (`D`).
///
/// `DXXXX n+ n- mname <area=val> <pj=val> <off> <ic=vd> <temp=val> <dtemp=val> <m=val>`
/// See ngspice manual §7.1.
#[derive(Debug)]
pub struct Diode {
    name: String,
    /// Anode (n+).
    node_plus: Node,
    /// Cathode (n-).
    node_minus: Node,
    /// Model (required).
    model: Arc<dyn crate::models::diode::DiodeModel + Send + Sync>,
    /// AREA: Area factor (scales IS, RS, CJ0, IBV). Default: 1.0.
    area: Option<Dynamic<Dimensionless>>,
    /// PJ: Perimeter factor (scales CJP). Default: 1.0.
    pj: Option<Dynamic<Meter>>,
    /// OFF: Indicates initial condition hint for DC analysis.
    off: bool,
    /// IC: Initial condition voltage across diode.
    ic: Option<Dynamic<Volt>>,
    /// TEMP: Instance temperature.
    temp: Option<Dynamic<Celsius>>,
    /// DTEMP: Temperature offset from circuit temperature.
    delta_temp: Option<Dynamic<Celsius>>,
    /// M: Multiplier (parallel instances).
    multiplier: Option<Dynamic<Dimensionless>>,
}

impl Clone for Diode {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            node_plus: self.node_plus.clone(),
            node_minus: self.node_minus.clone(),
            model: Arc::clone(&self.model),
            area: self.area.clone(),
            pj: self.pj.clone(),
            off: self.off,
            ic: self.ic.clone(),
            temp: self.temp.clone(),
            delta_temp: self.delta_temp.clone(),
            multiplier: self.multiplier.clone(),
        }
    }
}

impl Diode {
    pub const SYMBOL: &str = "D";

    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        model: Arc<dyn crate::models::diode::DiodeModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            model,
            area: None, pj: None, off: false, ic: None,
            temp: None, delta_temp: None, multiplier: None,
        }
    }

    pub fn with_area(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.area = Some(v.into()); self }
    pub fn with_pj(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self { self.pj = Some(v.into()); self }
    pub fn with_off(&mut self) -> &mut Self { self.off = true; self }
    pub fn with_ic(&mut self, v: impl Into<Dynamic<Volt>>) -> &mut Self { self.ic = Some(v.into()); self }
    pub fn with_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self { self.temp = Some(v.into()); self }
    pub fn with_delta_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self { self.delta_temp = Some(v.into()); self }
    pub fn with_multiplier(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.multiplier = Some(v.into()); self }

    pub fn name(&self) -> &str { &self.name }
    pub fn node_plus(&self) -> &Node { &self.node_plus }
    pub fn node_minus(&self) -> &Node { &self.node_minus }
    pub fn model_name(&self) -> &str { self.model.model_name() }
    pub fn area(&self) -> Option<&Dynamic<Dimensionless>> { self.area.as_ref() }
    pub fn pj(&self) -> Option<&Dynamic<Meter>> { self.pj.as_ref() }
    pub fn is_off(&self) -> bool { self.off }
    pub fn ic(&self) -> Option<&Dynamic<Volt>> { self.ic.as_ref() }
    pub fn temp(&self) -> Option<&Dynamic<Celsius>> { self.temp.as_ref() }
    pub fn delta_temp(&self) -> Option<&Dynamic<Celsius>> { self.delta_temp.as_ref() }
    pub fn multiplier(&self) -> Option<&Dynamic<Dimensionless>> { self.multiplier.as_ref() }
}

impl Component for Diode {}

impl SpiceElement for Diode {
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

impl SpiceComponent for Diode {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {}",
            Self::SYMBOL, self.name(), self.node_plus(), self.node_minus(), model_name
        );
        if let Some(a) = &self.area { s.push_str(&format!(" AREA={}", a)); }
        if let Some(pj) = &self.pj { s.push_str(&format!(" PJ={}", pj)); }
        if self.off { s.push_str(" OFF"); }
        if let Some(ic) = &self.ic { s.push_str(&format!(" IC={}", ic)); }
        if let Some(t) = &self.temp { s.push_str(&format!(" TEMP={}", t)); }
        if let Some(dt) = &self.delta_temp { s.push_str(&format!(" DTEMP={}", dt)); }
        if let Some(m) = &self.multiplier { s.push_str(&format!(" M={}", m)); }
        s
    }
}
