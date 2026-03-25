use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::Meter;
use std::sync::Arc;

/// Single lossy transmission line (`Y`) using a TXL model.
///
/// `YXXXX N1 G1 N2 G2 MNAME <LEN=val>`
#[derive(Debug)]
pub struct SingleLossyTransmissionLine {
    name: String,
    port1: Node,
    return1: Node,
    port2: Node,
    return2: Node,
    model: Arc<dyn crate::models::txl::TxlModel + Send + Sync>,
    len: Option<Dynamic<Meter>>,
}

impl Clone for SingleLossyTransmissionLine {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            port1: self.port1.clone(),
            return1: self.return1.clone(),
            port2: self.port2.clone(),
            return2: self.return2.clone(),
            model: Arc::clone(&self.model),
            len: self.len.clone(),
        }
    }
}

impl SingleLossyTransmissionLine {
    pub const SYMBOL: &str = "Y";

    pub fn new(
        name: impl Into<String>,
        port1: impl Into<Node>,
        return1: impl Into<Node>,
        port2: impl Into<Node>,
        return2: impl Into<Node>,
        model: Arc<dyn crate::models::txl::TxlModel + Send + Sync>,
    ) -> Self {
        Self {
            name: name.into(),
            port1: port1.into(),
            return1: return1.into(),
            port2: port2.into(),
            return2: return2.into(),
            model,
            len: None,
        }
    }

    pub fn with_len(&mut self, v: impl Into<Dynamic<Meter>>) -> &mut Self {
        self.len = Some(v.into());
        self
    }
}

impl Component for SingleLossyTransmissionLine {}

impl SpiceElement for SingleLossyTransmissionLine {
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

impl SpiceComponent for SingleLossyTransmissionLine {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {} {}",
            Self::SYMBOL,
            self.name,
            self.port1,
            self.return1,
            self.port2,
            self.return2,
            self.model.model_name()
        );
        if let Some(v) = &self.len {
            s.push_str(&format!(" LEN={v}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::txl::DefaultModel;

    #[test]
    fn serializes_txl_instance_minimal() {
        let model = Arc::new(DefaultModel::new("YMOD"));
        let y = SingleLossyTransmissionLine::new("1", "2", "0", "3", "0", model);
        assert_eq!(y.into_spice(), "Y1 2 0 3 0 YMOD");
    }

    #[test]
    fn serializes_txl_instance_with_len() {
        let model = Arc::new(DefaultModel::new("YMOD"));
        let mut y = SingleLossyTransmissionLine::new("1", "2", "0", "3", "0", model);
        y.with_len(16.0);
        assert_eq!(y.into_spice(), "Y1 2 0 3 0 YMOD LEN=16");
    }
}
