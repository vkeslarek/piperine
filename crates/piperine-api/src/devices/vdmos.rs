use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Celsius, Dimensionless, Volt};
use std::sync::Arc;

/// Vertical DMOS / power MOS (`M`, VDMOS model).
///
/// Common forms in ngspice:
/// `MXXXX D G S MNAME [instance params...]`
/// `MXXXX D G S TJ TC MNAME THERMAL [instance params...]`
#[derive(Debug)]
pub struct Vdmos {
    name: String,
    drain: Node,
    gate: Node,
    source: Node,
    temp_node: Option<Node>,
    tcase_node: Option<Node>,
    thermal: bool,
    model: Arc<dyn crate::models::vdmos::VdmosModel + Send + Sync>,
    multiplier: Option<Dynamic<Dimensionless>>,
    off: bool,
    ic_vds: Option<Dynamic<Volt>>,
    ic_vgs: Option<Dynamic<Volt>>,
    temp: Option<Dynamic<Celsius>>,
    dtemp: Option<Dynamic<Celsius>>,
}

impl Clone for Vdmos {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            drain: self.drain.clone(),
            gate: self.gate.clone(),
            source: self.source.clone(),
            temp_node: self.temp_node,
            tcase_node: self.tcase_node,
            thermal: self.thermal,
            model: Arc::clone(&self.model),
            multiplier: self.multiplier.clone(),
            off: self.off,
            ic_vds: self.ic_vds.clone(),
            ic_vgs: self.ic_vgs.clone(),
            temp: self.temp.clone(),
            dtemp: self.dtemp.clone(),
        }
    }
}

impl Vdmos {
    pub const SYMBOL: &str = "M";

    pub fn new(
        name: impl Into<String>,
        drain: impl Into<Node>,
        gate: impl Into<Node>,
        source: impl Into<Node>,
        model: Arc<dyn crate::models::vdmos::VdmosModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            temp_node: None,
            tcase_node: None,
            thermal: false,
            model,
            multiplier: None,
            off: false,
            ic_vds: None,
            ic_vgs: None,
            temp: None,
            dtemp: None,
        }
    }

    pub fn with_thermal_nodes(
        &mut self,
        temp_node: impl Into<Node>,
        tcase_node: impl Into<Node>,
    ) -> &mut Self {
        self.temp_node = Some(temp_node.into());
        self.tcase_node = Some(tcase_node.into());
        self.thermal = true;
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
    pub fn with_temp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self {
        self.temp = Some(v.into());
        self
    }
    pub fn with_dtemp(&mut self, v: impl Into<Dynamic<Celsius>>) -> &mut Self {
        self.dtemp = Some(v.into());
        self
    }
}

impl Component for Vdmos {}

impl SpiceElement for Vdmos {
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

impl SpiceComponent for Vdmos {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {}",
            Self::SYMBOL,
            self.name,
            self.drain,
            self.gate,
            self.source
        );

        let has_thermal_nodes = self.temp_node.is_some() && self.tcase_node.is_some();
        if let (Some(tj), Some(tc)) = (&self.temp_node, &self.tcase_node) {
            s.push_str(&format!(" {tj} {tc}"));
        }

        s.push_str(&format!(" {}", self.model.model_name()));

        if self.thermal && has_thermal_nodes {
            s.push_str(" THERMAL");
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
        if let Some(v) = &self.temp {
            s.push_str(&format!(" TEMP={v}"));
        }
        if let Some(v) = &self.dtemp {
            s.push_str(&format!(" DTEMP={v}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::vdmos::{DefaultModel, VdmosType};

    #[test]
    fn serializes_vdmos_three_terminal() {
        let model = Arc::new(DefaultModel::new("IRFP240", VdmosType::Nchan));
        let mut m = Vdmos::new("1", "d", "g", "s", model);
        m.with_multiplier(2.0)
            .with_off()
            .with_ic(3.0, 4.0)
            .with_temp(30.0)
            .with_dtemp(5.0);

        assert_eq!(
            m.into_spice(),
            "M1 d g s IRFP240 M=2 OFF IC=3,4 TEMP=30 DTEMP=5"
        );
    }

    #[test]
    fn serializes_vdmos_thermal_form() {
        let model = Arc::new(DefaultModel::new("IRFP240", VdmosType::Nchan));
        let mut m = Vdmos::new("1", "d", "g", "s", model);
        m.with_thermal_nodes("tj", "tc");

        assert_eq!(m.into_spice(), "M1 d g s tj tc IRFP240 THERMAL");
    }
}
