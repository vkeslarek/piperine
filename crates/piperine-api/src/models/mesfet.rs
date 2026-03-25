use crate::devices::Mesfet;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Dimensionless, Farad, Ohm, Volt};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait MesfetModel: Model<ComponentType = Mesfet> + SpiceModel + Debug {}

pub static DEFAULT_NMF: LazyLock<Arc<dyn MesfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", MesfetType::Nmf)));

pub static DEFAULT_PMF: LazyLock<Arc<dyn MesfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", MesfetType::Pmf)));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesfetType {
    Nmf,
    Pmf,
}

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub mesfet_type: MesfetType,
    pub vto: Option<Volt>,
    pub alpha: Option<Dimensionless>,
    pub beta: Option<Dimensionless>,
    pub lambda: Option<Dimensionless>,
    pub b: Option<Dimensionless>,
    pub rd: Option<Ohm>,
    pub rs: Option<Ohm>,
    pub cgs: Option<Farad>,
    pub cgd: Option<Farad>,
    pub pb: Option<Volt>,
    pub is: Option<Dimensionless>,
    pub fc: Option<Dimensionless>,
    pub kf: Option<Dimensionless>,
    pub af: Option<Dimensionless>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, mesfet_type: MesfetType) -> Self {
        Self {
            name: name.into(),
            mesfet_type,
            vto: None,
            alpha: None,
            beta: None,
            lambda: None,
            b: None,
            rd: None,
            rs: None,
            cgs: None,
            cgd: None,
            pb: None,
            is: None,
            fc: None,
            kf: None,
            af: None,
        }
    }

    pub fn with_vto(&mut self, v: Volt) -> &mut Self {
        self.vto = Some(v);
        self
    }
    pub fn with_alpha(&mut self, v: Dimensionless) -> &mut Self {
        self.alpha = Some(v);
        self
    }
    pub fn with_beta(&mut self, v: Dimensionless) -> &mut Self {
        self.beta = Some(v);
        self
    }
    pub fn with_lambda(&mut self, v: Dimensionless) -> &mut Self {
        self.lambda = Some(v);
        self
    }
    pub fn with_b(&mut self, v: Dimensionless) -> &mut Self {
        self.b = Some(v);
        self
    }
    pub fn with_rd(&mut self, v: Ohm) -> &mut Self {
        self.rd = Some(v);
        self
    }
    pub fn with_rs(&mut self, v: Ohm) -> &mut Self {
        self.rs = Some(v);
        self
    }
    pub fn with_cgs(&mut self, v: Farad) -> &mut Self {
        self.cgs = Some(v);
        self
    }
    pub fn with_cgd(&mut self, v: Farad) -> &mut Self {
        self.cgd = Some(v);
        self
    }
    pub fn with_pb(&mut self, v: Volt) -> &mut Self {
        self.pb = Some(v);
        self
    }
    pub fn with_is(&mut self, v: Dimensionless) -> &mut Self {
        self.is = Some(v);
        self
    }
    pub fn with_fc(&mut self, v: Dimensionless) -> &mut Self {
        self.fc = Some(v);
        self
    }
    pub fn with_kf(&mut self, v: Dimensionless) -> &mut Self {
        self.kf = Some(v);
        self
    }
    pub fn with_af(&mut self, v: Dimensionless) -> &mut Self {
        self.af = Some(v);
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Mesfet;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let model_type = match self.mesfet_type {
            MesfetType::Nmf => "NMF",
            MesfetType::Pmf => "PMF",
        };

        let mut params = Vec::new();
        if let Some(v) = self.vto {
            params.push(format!("VTO={v}"));
        }
        if let Some(v) = self.alpha {
            params.push(format!("ALPHA={v}"));
        }
        if let Some(v) = self.beta {
            params.push(format!("BETA={v}"));
        }
        if let Some(v) = self.lambda {
            params.push(format!("LAMBDA={v}"));
        }
        if let Some(v) = self.b {
            params.push(format!("B={v}"));
        }
        if let Some(v) = self.rd {
            params.push(format!("RD={v}"));
        }
        if let Some(v) = self.rs {
            params.push(format!("RS={v}"));
        }
        if let Some(v) = self.cgs {
            params.push(format!("CGS={v}"));
        }
        if let Some(v) = self.cgd {
            params.push(format!("CGD={v}"));
        }
        if let Some(v) = self.pb {
            params.push(format!("PB={v}"));
        }
        if let Some(v) = self.is {
            params.push(format!("IS={v}"));
        }
        if let Some(v) = self.fc {
            params.push(format!("FC={v}"));
        }
        if let Some(v) = self.kf {
            params.push(format!("KF={v}"));
        }
        if let Some(v) = self.af {
            params.push(format!("AF={v}"));
        }

        if params.is_empty() {
            format!(".MODEL {} {}", self.name, model_type)
        } else {
            format!(".MODEL {} {} ({})", self.name, model_type, params.join(" "))
        }
    }
}

impl MesfetModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesfet_model_spice_line_minimal() {
        let m = DefaultModel::new("ZM1", MesfetType::Pmf);
        assert_eq!(m.to_spice_model_line(), ".MODEL ZM1 PMF");
    }

    #[test]
    fn mesfet_model_spice_line() {
        let mut m = DefaultModel::new("ZM1", MesfetType::Nmf);
        m.with_vto(-2.0).with_beta(1.0e-3);
        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL ZM1 NMF (VTO=-2 BETA=0.001)"
        );
    }
}
