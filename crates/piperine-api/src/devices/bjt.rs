use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Celsius, Dimensionless, Volt};
use std::sync::Arc;

/// Bipolar Junction Transistor (`Q`).
///
/// `QXXXX nc nb ne <ns> mname <area=val> <areac=val> <areab=val> <m=val>
///  <off> <ic=vbe,vce> <temp=val> <dtemp=val>`
/// See ngspice manual §8.1.
#[derive(Debug)]
pub struct Bjt {
    name: String,
    /// Collector.
    collector: Node,
    /// Base.
    base: Node,
    /// Emitter.
    emitter: Node,
    /// Substrate (optional, 4th terminal).
    substrate: Option<Node>,
    /// Model (required).
    model: Arc<dyn crate::models::bjt::BjtModel + Send + Sync>,
    /// AREA: Emitter area factor.
    area: Option<Dynamic<Dimensionless>>,
    /// AREAC: Collector area factor.
    areac: Option<Dynamic<Dimensionless>>,
    /// AREAB: Base area factor.
    areab: Option<Dynamic<Dimensionless>>,
    /// M: Multiplier.
    multiplier: Option<Dynamic<Dimensionless>>,
    /// OFF: Initial condition hint.
    off: bool,
    /// IC: Initial condition VBE.
    ic_vbe: Option<Dynamic<Volt>>,
    /// IC: Initial condition VCE.
    ic_vce: Option<Dynamic<Volt>>,
    /// TEMP: Instance temperature.
    temp: Option<Dynamic<Celsius>>,
    /// DTEMP: Temperature offset.
    delta_temp: Option<Dynamic<Celsius>>,
}

impl Clone for Bjt {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            collector: self.collector.clone(),
            base: self.base.clone(),
            emitter: self.emitter.clone(),
            substrate: self.substrate.clone(),
            model: Arc::clone(&self.model),
            area: self.area.clone(),
            areac: self.areac.clone(),
            areab: self.areab.clone(),
            multiplier: self.multiplier.clone(),
            off: self.off,
            ic_vbe: self.ic_vbe.clone(),
            ic_vce: self.ic_vce.clone(),
            temp: self.temp.clone(),
            delta_temp: self.delta_temp.clone(),
        }
    }
}

impl Bjt {
    pub const SYMBOL: &str = "Q";

    pub fn new(
        name: impl Into<String>,
        collector: impl Into<Node>,
        base: impl Into<Node>,
        emitter: impl Into<Node>,
        model: Arc<dyn crate::models::bjt::BjtModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            collector: collector.into(),
            base: base.into(),
            emitter: emitter.into(),
            substrate: None,
            model,
            area: None, areac: None, areab: None, multiplier: None,
            off: false, ic_vbe: None, ic_vce: None, temp: None, delta_temp: None,
        }
    }

    pub fn with_substrate(&mut self, node: impl Into<Node>) -> &mut Self { self.substrate = Some(node.into()); self }
    pub fn with_area(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.area = Some(v.into()); self }
    pub fn with_areac(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.areac = Some(v.into()); self }
    pub fn with_areab(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.areab = Some(v.into()); self }
    pub fn with_multiplier(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.multiplier = Some(v.into()); self }
    pub fn with_off(&mut self) -> &mut Self { self.off = true; self }
    pub fn with_ic(&mut self, vbe: impl Into<Dynamic<Volt>>, vce: impl Into<Dynamic<Volt>>) -> &mut Self {
        self.ic_vbe = Some(vbe.into());
        self.ic_vce = Some(vce.into());
        self
    }
    pub fn with_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self { self.temp = Some(v.into()); self }
    pub fn with_delta_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self { self.delta_temp = Some(v.into()); self }

    pub fn name(&self) -> &str { &self.name }
    pub fn collector(&self) -> &Node { &self.collector }
    pub fn base(&self) -> &Node { &self.base }
    pub fn emitter(&self) -> &Node { &self.emitter }
    pub fn substrate(&self) -> Option<&Node> { self.substrate.as_ref() }
    pub fn model_name(&self) -> &str { self.model.model_name() }
}

impl Component for Bjt {}

impl SpiceElement for Bjt {
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

impl SpiceComponent for Bjt {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {}",
            Self::SYMBOL, self.name(),
            self.collector(), self.base(), self.emitter()
        );
        if let Some(sub) = &self.substrate {
            s.push_str(&format!(" {}", sub));
        }
        s.push_str(&format!(" {}", model_name));
        if let Some(a) = &self.area { s.push_str(&format!(" AREA={}", a)); }
        if let Some(a) = &self.areac { s.push_str(&format!(" AREAC={}", a)); }
        if let Some(a) = &self.areab { s.push_str(&format!(" AREAB={}", a)); }
        if let Some(m) = &self.multiplier { s.push_str(&format!(" M={}", m)); }
        if self.off { s.push_str(" OFF"); }
        if let (Some(vbe), Some(vce)) = (&self.ic_vbe, &self.ic_vce) {
            s.push_str(&format!(" IC={},{}", vbe, vce));
        }
        if let Some(t) = &self.temp { s.push_str(&format!(" TEMP={}", t)); }
        if let Some(dt) = &self.delta_temp { s.push_str(&format!(" DTEMP={}", dt)); }
        s
    }
}
