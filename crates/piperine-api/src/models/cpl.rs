use crate::devices::CoupledMultiline;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{FaradPerMeter, Henry, Meter, Ohm, Siemens};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait CplModel: Model<ComponentType = CoupledMultiline> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn CplModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub r: Vec<Ohm>,
    pub l: Vec<Henry>,
    pub c: Vec<FaradPerMeter>,
    pub g: Vec<Siemens>,
    pub length: Option<Meter>,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            r: Vec::new(),
            l: Vec::new(),
            c: Vec::new(),
            g: Vec::new(),
            length: None,
        }
    }

    pub fn with_r_matrix(&mut self, vals: Vec<Ohm>) -> &mut Self {
        self.r = vals;
        self
    }
    pub fn with_l_matrix(&mut self, vals: Vec<Henry>) -> &mut Self {
        self.l = vals;
        self
    }
    pub fn with_c_matrix(&mut self, vals: Vec<FaradPerMeter>) -> &mut Self {
        self.c = vals;
        self
    }
    pub fn with_g_matrix(&mut self, vals: Vec<Siemens>) -> &mut Self {
        self.g = vals;
        self
    }
    pub fn with_length(&mut self, v: Meter) -> &mut Self {
        self.length = Some(v);
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = CoupledMultiline;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let mut params = Vec::new();
        if !self.r.is_empty() {
            params.push(format!(
                "R={}",
                self.r
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        if !self.l.is_empty() {
            params.push(format!(
                "L={}",
                self.l
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        if !self.g.is_empty() {
            params.push(format!(
                "G={}",
                self.g
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        if !self.c.is_empty() {
            params.push(format!(
                "C={}",
                self.c
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        if let Some(v) = self.length {
            params.push(format!("LENGTH={v}"));
        }

        if params.is_empty() {
            format!(".MODEL {} CPL", self.name)
        } else {
            format!(".MODEL {} CPL ({})", self.name, params.join(" "))
        }
    }
}

impl CplModel for DefaultModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_cpl_model_minimal() {
        let m = DefaultModel::new("PLINE");
        assert_eq!(m.to_spice_model_line(), ".MODEL PLINE CPL");
    }

    #[test]
    fn serializes_cpl_model_with_matrices() {
        let mut m = DefaultModel::new("PLINE");
        m.with_r_matrix(vec![0.2, 0.0, 0.2])
            .with_l_matrix(vec![9.13e-9, 3.3e-9, 9.13e-9])
            .with_g_matrix(vec![0.0, 0.0, 0.0])
            .with_c_matrix(vec![3.65e-13, -9e-14, 3.65e-13])
            .with_length(24.0);

        assert_eq!(
            m.to_spice_model_line(),
            ".MODEL PLINE CPL (R=0.2 0 0.2 L=0.00000000913 0.0000000033 0.00000000913 G=0 0 0 C=0.000000000000365 -0.00000000000009 0.000000000000365 LENGTH=24)"
        );
    }
}
