use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Ampere, Volt};
use std::sync::Arc;

/// Lossy transmission line (`O`) using an LTRA model.
///
/// `OXXXX N1 N2 N3 N4 MNAME <IC=V1,I1,V2,I2>`
#[derive(Debug)]
pub struct LossyTransmissionLine {
    name: String,
    port1_plus: Node,
    port1_minus: Node,
    port2_plus: Node,
    port2_minus: Node,
    model: Arc<dyn crate::models::ltra::LtraModel + Send + Sync>,
    ic_v1: Option<Dynamic<Volt>>,
    ic_i1: Option<Dynamic<Ampere>>,
    ic_v2: Option<Dynamic<Volt>>,
    ic_i2: Option<Dynamic<Ampere>>,
}

impl Clone for LossyTransmissionLine {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            port1_plus: self.port1_plus.clone(),
            port1_minus: self.port1_minus.clone(),
            port2_plus: self.port2_plus.clone(),
            port2_minus: self.port2_minus.clone(),
            model: Arc::clone(&self.model),
            ic_v1: self.ic_v1.clone(),
            ic_i1: self.ic_i1.clone(),
            ic_v2: self.ic_v2.clone(),
            ic_i2: self.ic_i2.clone(),
        }
    }
}

impl LossyTransmissionLine {
    pub const SYMBOL: &str = "O";

    pub fn new(
        name: impl Into<String>,
        port1_plus: impl Into<Node>,
        port1_minus: impl Into<Node>,
        port2_plus: impl Into<Node>,
        port2_minus: impl Into<Node>,
        model: Arc<dyn crate::models::ltra::LtraModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            port1_plus: port1_plus.into(),
            port1_minus: port1_minus.into(),
            port2_plus: port2_plus.into(),
            port2_minus: port2_minus.into(),
            model,
            ic_v1: None,
            ic_i1: None,
            ic_v2: None,
            ic_i2: None,
        }
    }

    pub fn with_ic(
        &mut self,
        v1: impl Into<Dynamic<Volt>>,
        i1: impl Into<Dynamic<Ampere>>,
        v2: impl Into<Dynamic<Volt>>,
        i2: impl Into<Dynamic<Ampere>>,
    ) -> &mut Self {
        self.ic_v1 = Some(v1.into());
        self.ic_i1 = Some(i1.into());
        self.ic_v2 = Some(v2.into());
        self.ic_i2 = Some(i2.into());
        self
    }
}

impl Component for LossyTransmissionLine {}

impl SpiceElement for LossyTransmissionLine {
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

impl SpiceComponent for LossyTransmissionLine {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL,
            self.name,
            self.port1_plus,
            self.port1_minus,
            self.port2_plus,
            self.port2_minus,
            self.model.model_name()
        );

        if let (Some(v1), Some(i1), Some(v2), Some(i2)) =
            (&self.ic_v1, &self.ic_i1, &self.ic_v2, &self.ic_i2)
        {
            s.push_str(&format!(" IC={v1},{i1},{v2},{i2}"));
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ltra::DefaultModel;

    #[test]
    fn serializes_ltra_instance_without_ic() {
        let model = Arc::new(DefaultModel::new("LLINE"));
        let o = LossyTransmissionLine::new("1", "n1", "0", "n2", "0", model);
        assert_eq!(o.into_spice(), "O1 n1 0 n2 0 LLINE");
    }

    #[test]
    fn serializes_ltra_instance_with_ic() {
        let model = Arc::new(DefaultModel::new("LLINE"));
        let mut o = LossyTransmissionLine::new("1", "n1", "0", "n2", "0", model);
        o.with_ic(1.0, 0.1, 2.0, 0.2);
        assert_eq!(o.into_spice(), "O1 n1 0 n2 0 LLINE IC=1,0.1,2,0.2");
    }
}
