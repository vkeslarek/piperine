use crate::devices::LossyTransmissionLine;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{FaradPerMeter, Henry, Meter, Ohm, Siemens};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait LtraModel: Model<ComponentType = LossyTransmissionLine> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn LtraModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub r: Option<Ohm>,
    pub l: Option<Henry>,
    pub g: Option<Siemens>,
    pub c: Option<FaradPerMeter>,
    pub len: Option<Meter>,
    pub rel: Option<f64>,
    pub abs: Option<f64>,
    pub nocontrol: bool,
    pub steplimit: bool,
    pub nosteplimit: bool,
    pub lininterp: bool,
    pub quadinterp: bool,
    pub mixedinterp: bool,
    pub truncnr: bool,
    pub truncdontcut: bool,
    pub compactrel: Option<f64>,
    pub compactabs: Option<f64>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            r: None,
            l: None,
            g: None,
            c: None,
            len: None,
            rel: None,
            abs: None,
            nocontrol: false,
            steplimit: false,
            nosteplimit: false,
            lininterp: false,
            quadinterp: false,
            mixedinterp: false,
            truncnr: false,
            truncdontcut: false,
            compactrel: None,
            compactabs: None,
        }
    }

    pub fn with_r(&mut self, v: Ohm) -> &mut Self {
        self.r = Some(v);
        self
    }
    pub fn with_l(&mut self, v: Henry) -> &mut Self {
        self.l = Some(v);
        self
    }
    pub fn with_g(&mut self, v: Siemens) -> &mut Self {
        self.g = Some(v);
        self
    }
    pub fn with_c(&mut self, v: FaradPerMeter) -> &mut Self {
        self.c = Some(v);
        self
    }
    pub fn with_len(&mut self, v: Meter) -> &mut Self {
        self.len = Some(v);
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = LossyTransmissionLine;
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
        if let Some(v) = self.len {
            params.push(format!("LEN={v}"));
        }
        if let Some(v) = self.rel {
            params.push(format!("REL={v}"));
        }
        if let Some(v) = self.abs {
            params.push(format!("ABS={v}"));
        }
        if self.nocontrol {
            params.push("NOCONTROL".to_string());
        }
        if self.steplimit {
            params.push("STEPLIMIT".to_string());
        }
        if self.nosteplimit {
            params.push("NOSTEPLIMIT".to_string());
        }
        if self.lininterp {
            params.push("LININTERP".to_string());
        }
        if self.quadinterp {
            params.push("QUADINTERP".to_string());
        }
        if self.mixedinterp {
            params.push("MIXEDINTERP".to_string());
        }
        if self.truncnr {
            params.push("TRUNCNR".to_string());
        }
        if self.truncdontcut {
            params.push("TRUNCDONTCUT".to_string());
        }
        if let Some(v) = self.compactrel {
            params.push(format!("COMPACTREL={v}"));
        }
        if let Some(v) = self.compactabs {
            params.push(format!("COMPACTABS={v}"));
        }

        if params.is_empty() {
            format!(".MODEL {} LTRA", self.name)
        } else {
            format!(".MODEL {} LTRA ({})", self.name, params.join(" "))
        }
    }
}

impl LtraModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ltra_model_spice_line() {
        let mut m = DefaultModel::new("LLINE");
        m.with_r(12.45)
            .with_l(8.972e-9)
            .with_c(0.468e-12)
            .with_len(16.0);
        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL LLINE LTRA (R=12.45 L=0.000000008972 C=0.000000000000468 LEN=16)"
        );
    }
}
