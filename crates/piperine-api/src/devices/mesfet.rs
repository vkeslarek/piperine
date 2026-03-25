use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Dimensionless, Volt};
use std::sync::Arc;

/// MESFET (`Z`).
///
/// `ZXXXX ND NG NS MNAME <AREA=val> <OFF> <IC=VDS,VGS> <M=val>`
#[derive(Debug)]
pub struct Mesfet {
    name: String,
    drain: Node,
    gate: Node,
    source: Node,
    model: Arc<dyn crate::models::mesfet::MesfetModel + Send + Sync>,
    area: Option<Dynamic<Dimensionless>>,
    multiplier: Option<Dynamic<Dimensionless>>,
    off: bool,
    ic_vds: Option<Dynamic<Volt>>,
    ic_vgs: Option<Dynamic<Volt>>,
}

impl Clone for Mesfet {
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
        }
    }
}

impl Mesfet {
    pub const SYMBOL: &str = "Z";

    pub fn new(
        name: impl Into<String>,
        drain: impl Into<Node>,
        gate: impl Into<Node>,
        source: impl Into<Node>,
        model: Arc<dyn crate::models::mesfet::MesfetModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            model,
            area: None,
            multiplier: None,
            off: false,
            ic_vds: None,
            ic_vgs: None,
        }
    }

    pub fn with_area(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.area = Some(v.into());
        self
    }
    pub fn with_multiplier(&mut self, v: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.multiplier = Some(v.into());
        self
    }
    pub fn with_off(&mut self) -> &mut Self {
        self.off = true;
        self
    }
    pub fn with_ic(
        &mut self,
        vds: impl Into<Dynamic<Volt>>,
        vgs: impl Into<Dynamic<Volt>>,
    ) -> &mut Self {
        self.ic_vds = Some(vds.into());
        self.ic_vgs = Some(vgs.into());
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Component for Mesfet {}

impl SpiceElement for Mesfet {
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

impl SpiceComponent for Mesfet {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL,
            self.name,
            self.drain,
            self.gate,
            self.source,
            self.model.model_name()
        );
        if let Some(v) = &self.area {
            s.push_str(&format!(" AREA={v}"));
        }
        if let Some(v) = &self.multiplier {
            s.push_str(&format!(" M={v}"));
        }
        if self.off {
            s.push_str(" OFF");
        }
        if let (Some(vds), Some(vgs)) = (&self.ic_vds, &self.ic_vgs) {
            s.push_str(&format!(" IC={vds},{vgs}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mesfet::{DefaultModel, MesfetType};

    #[test]
    fn serializes_mesfet() {
        let model = Arc::new(DefaultModel::new("ZM1", MesfetType::Nmf));
        let mut z = Mesfet::new("1", "d", "g", "s", model);
        z.with_area(2.0)
            .with_off()
            .with_ic(1.0, -0.2)
            .with_multiplier(4.0);
        assert_eq!(z.into_spice(), "Z1 d g s ZM1 AREA=2 M=4 OFF IC=1,-0.2");
    }
}
