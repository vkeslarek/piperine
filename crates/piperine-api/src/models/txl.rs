use crate::devices::SingleLossyTransmissionLine;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{FaradPerMeter, Henry, Meter, Ohm, Siemens};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait TxlModel:
    Model<ComponentType = SingleLossyTransmissionLine> + SpiceModel + Debug
{
}

pub static DEFAULT: LazyLock<Arc<dyn TxlModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub r: Option<Ohm>,
    pub l: Option<Henry>,
    pub c: Option<FaradPerMeter>,
    pub g: Option<Siemens>,
    pub length: Option<Meter>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            r: None,
            l: None,
            c: None,
            g: None,
            length: None,
        }
    }
}

impl Model for DefaultModel {
    type ComponentType = SingleLossyTransmissionLine;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let mut params = Vec::new();
        if let Some(v) = self.r {
            params.push(format!("R={v}"));
        }
        if let Some(v) = self.l {
            params.push(format!("L={v}"));
        }
        if let Some(v) = self.g {
            params.push(format!("G={v}"));
        }
        if let Some(v) = self.c {
            params.push(format!("C={v}"));
        }
        if let Some(v) = self.length {
            params.push(format!("LENGTH={v}"));
        }

        if params.is_empty() {
            format!(".MODEL {} TXL", self.name)
        } else {
            format!(".MODEL {} TXL ({})", self.name, params.join(" "))
        }
    }
}

impl TxlModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_txl_model_minimal() {
        let m = DefaultModel::new("YMOD");
        assert_eq!(m.to_spice_model_line(), ".MODEL YMOD TXL");
    }

    #[test]
    fn serializes_txl_model_with_parameters() {
        let mut m = DefaultModel::new("YMOD");
        m.r = Some(12.45);
        m.l = Some(8.972e-9);
        m.g = Some(0.0);
        m.c = Some(0.468e-12);
        m.length = Some(16.0);

        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL YMOD TXL (R=12.45 L=0.000000008972 G=0 C=0.000000000000468 LENGTH=16)"
        );
    }
}
