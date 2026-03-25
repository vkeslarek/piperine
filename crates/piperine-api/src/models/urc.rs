use crate::devices::UrcLine;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Ampere, FaradPerMeter, Hertz, Ohm};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait UrcModel: Model<ComponentType = UrcLine> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn UrcModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub k: Option<f64>,
    pub fmax: Option<Hertz>,
    pub rperl: Option<Ohm>,
    pub cperl: Option<FaradPerMeter>,
    pub isperl: Option<Ampere>,
    pub rsperl: Option<Ohm>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            k: None,
            fmax: None,
            rperl: None,
            cperl: None,
            isperl: None,
            rsperl: None,
        }
    }
}

impl Model for DefaultModel {
    type ComponentType = UrcLine;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let mut params = Vec::new();
        if let Some(v) = self.k {
            params.push(format!("K={v}"));
        }
        if let Some(v) = self.fmax {
            params.push(format!("FMAX={v}"));
        }
        if let Some(v) = self.rperl {
            params.push(format!("RPERL={v}"));
        }
        if let Some(v) = self.cperl {
            params.push(format!("CPERL={v}"));
        }
        if let Some(v) = self.isperl {
            params.push(format!("ISPERL={v}"));
        }
        if let Some(v) = self.rsperl {
            params.push(format!("RSPERL={v}"));
        }

        if params.is_empty() {
            format!(".MODEL {} URC", self.name)
        } else {
            format!(".MODEL {} URC ({})", self.name, params.join(" "))
        }
    }
}

impl UrcModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_urc_model_minimal() {
        let m = DefaultModel::new("URCMOD");
        assert_eq!(m.to_spice_model_line(), ".MODEL URCMOD URC");
    }

    #[test]
    fn serializes_urc_model_with_parameters() {
        let mut m = DefaultModel::new("URCMOD");
        m.k = Some(2.0);
        m.fmax = Some(10e9);
        m.rperl = Some(100e3);
        m.cperl = Some(100e-12);
        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL URCMOD URC (K=2 FMAX=10000000000 RPERL=100000 CPERL=0.0000000001)"
        );
    }
}
