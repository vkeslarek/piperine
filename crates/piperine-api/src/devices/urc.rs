use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Meter;
use std::sync::Arc;

/// Uniform distributed RC line (`U`).
///
/// `UXXXX N1 N2 N3 MNAME L=LEN <N=LUMPS>`
#[derive(Debug)]
pub struct UrcLine {
    name: String,
    pos: Node,
    neg: Node,
    gnd: Node,
    model: Arc<dyn crate::models::urc::UrcModel + Send + Sync>,
    length: Option<Dynamic<Meter>>,
    lumps: Option<u32>,
}

impl Clone for UrcLine {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            pos: self.pos.clone(),
            neg: self.neg.clone(),
            gnd: self.gnd.clone(),
            model: Arc::clone(&self.model),
            length: self.length.clone(),
            lumps: self.lumps,
        }
    }
}

impl UrcLine {
    pub const SYMBOL: &str = "U";

    pub fn new(
        name: impl Into<String>,
        pos: impl Into<Node>,
        neg: impl Into<Node>,
        gnd: impl Into<Node>,
        model: Arc<dyn crate::models::urc::UrcModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            pos: pos.into(),
            neg: neg.into(),
            gnd: gnd.into(),
            model,
            length: None,
            lumps: None,
        }
    }

    pub fn with_length(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.length = Some(v.into());
        self
    }
    pub fn with_lumps(&mut self, n: u32) -> &mut Self {
        self.lumps = Some(n);
        self
    }
}

impl Component for UrcLine {}

impl SpiceElement for UrcLine {
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

impl SpiceComponent for UrcLine {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {}",
            Self::SYMBOL,
            self.name,
            self.pos,
            self.neg,
            self.gnd,
            self.model.model_name()
        );
        if let Some(v) = &self.length {
            s.push_str(&format!(" L={v}"));
        }
        if let Some(v) = self.lumps {
            s.push_str(&format!(" N={v}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::urc::DefaultModel;

    #[test]
    fn serializes_urc_instance_minimal() {
        let model = Arc::new(DefaultModel::new("URCMOD"));
        let u = UrcLine::new("1", "n1", "n2", "0", model);
        assert_eq!(u.into_spice(), "U1 n1 n2 0 URCMOD");
    }

    #[test]
    fn serializes_urc_instance_with_length_and_lumps() {
        let model = Arc::new(DefaultModel::new("URCMOD"));
        let mut u = UrcLine::new("1", "n1", "n2", "0", model);
        u.with_length(20e-3).with_lumps(6);
        assert_eq!(u.into_spice(), "U1 n1 n2 0 URCMOD L=0.02 N=6");
    }
}
