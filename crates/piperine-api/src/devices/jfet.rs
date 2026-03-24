use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Celsius, Dimensionless, Volt};
use std::sync::Arc;

/// Junction Field-Effect Transistor (`J`).
///
/// `JXXXX nd ng ns mname <area=val> <off> <ic=vds,vgs> <temp=val> <m=val>`
/// See ngspice manual §10.1.
#[derive(Debug)]
pub struct Jfet {
    name: String,
    drain: Node,
    gate: Node,
    source: Node,
    /// Model (required).
    model: Arc<dyn crate::models::jfet::JfetModel + Send + Sync>,
    /// AREA: Area factor.
    area: Option<Dynamic<Dimensionless>>,
    /// M: Multiplier.
    multiplier: Option<Dynamic<Dimensionless>>,
    /// OFF: Initial condition hint.
    off: bool,
    /// IC: Initial VDS.
    ic_vds: Option<Dynamic<Volt>>,
    /// IC: Initial VGS.
    ic_vgs: Option<Dynamic<Volt>>,
    /// TEMP: Instance temperature.
    temp: Option<Dynamic<Celsius>>,
}

impl Clone for Jfet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            drain: self.drain.clone(),
            gate: self.gate.clone(),
            source: self.source.clone(),
            model: Arc::clone(&self.model),
            area: self.area.clone(),
            multiplier: self.multiplier.clone(),
            off: self.off,
            ic_vds: self.ic_vds.clone(),
            ic_vgs: self.ic_vgs.clone(),
            temp: self.temp.clone(),
        }
    }
}

impl Jfet {
    pub const SYMBOL: &str = "J";

    pub fn new(
        name: impl Into<String>,
        drain: impl Into<Node>,
        gate: impl Into<Node>,
        source: impl Into<Node>,
        model: Arc<dyn crate::models::jfet::JfetModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            model,
            area: None, multiplier: None, off: false,
            ic_vds: None, ic_vgs: None, temp: None,
        }
    }

    pub fn with_area(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.area = Some(v.into()); self }
    pub fn with_multiplier(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self { self.multiplier = Some(v.into()); self }
    pub fn with_off(&mut self) -> &mut Self { self.off = true; self }
    pub fn with_ic(&mut self, vds: impl Into<Dynamic<Volt>>, vgs: impl Into<Dynamic<Volt>>) -> &mut Self {
        self.ic_vds = Some(vds.into());
        self.ic_vgs = Some(vgs.into());
        self
    }
    pub fn with_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self { self.temp = Some(v.into()); self }

    pub fn name(&self) -> &str { &self.name }
    pub fn drain(&self) -> &Node { &self.drain }
    pub fn gate(&self) -> &Node { &self.gate }
    pub fn source(&self) -> &Node { &self.source }
    pub fn model_name(&self) -> &str { self.model.model_name() }
}

impl Component for Jfet {}

impl SpiceElement for Jfet {
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

impl SpiceComponent for Jfet {
    fn into_spice(&self) -> String {
        let model_name = self.model.model_name();
        let mut s = format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL, self.name(),
            self.drain(), self.gate(), self.source(),
            model_name
        );
        if let Some(a) = &self.area { s.push_str(&format!(" AREA={}", a)); }
        if let Some(m) = &self.multiplier { s.push_str(&format!(" M={}", m)); }
        if self.off { s.push_str(" OFF"); }
        if let (Some(vds), Some(vgs)) = (&self.ic_vds, &self.ic_vgs) {
            s.push_str(&format!(" IC={},{}", vds, vgs));
        }
        if let Some(t) = &self.temp { s.push_str(&format!(" TEMP={}", t)); }
        s
    }
}
