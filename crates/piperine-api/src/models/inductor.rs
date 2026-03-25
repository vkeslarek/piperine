use crate::devices::Inductor;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{Celsius, Dimensionless, Henry, Meter, MeterSquared, UnitExt};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait InductorModel: Model<ComponentType = Inductor> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn InductorModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

/// Semiconductor inductor model parameters.
///
/// All parameters from ngspice manual §3.3.11.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
    /// IND: Default inductance (H). Default: 0.0.
    pub ind: Henry,
    /// CSECT: Cross-section area (m²). Default: 0.0.
    pub csect: MeterSquared,
    /// DIA: Coil diameter (m). Default: 0.0.
    pub dia: Meter,
    /// LENGTH: Coil length (m). Default: 0.0.
    pub length: Meter,
    /// TC1: First order temperature coefficient (1/°C). Default: 0.0.
    pub tc1: Dimensionless,
    /// TC2: Second order temperature coefficient (1/°C²). Default: 0.0.
    pub tc2: Dimensionless,
    /// NT: Number of turns. Default: 0.0.
    pub nt: Dimensionless,
    /// MU: Relative magnetic permeability. Default: 0.0.
    pub mu: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tnom: 27.0.deg_C(),
            ind: 0.0.H(),
            csect: 0.0,
            dia: 0.0.m(),
            length: 0.0.m(),
            tc1: 0.0.inv_C(),
            tc2: 0.0.inv_C2(),
            nt: 0.0,
            mu: 0.0,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn tnom(&self) -> &Celsius {
        &self.tnom
    }
    pub fn ind(&self) -> &Henry {
        &self.ind
    }
    pub fn csect(&self) -> &MeterSquared {
        &self.csect
    }
    pub fn dia(&self) -> &Meter {
        &self.dia
    }
    pub fn length(&self) -> &Meter {
        &self.length
    }
    pub fn tc1(&self) -> &Dimensionless {
        &self.tc1
    }
    pub fn tc2(&self) -> &Dimensionless {
        &self.tc2
    }
    pub fn nt(&self) -> &Dimensionless {
        &self.nt
    }
    pub fn mu(&self) -> &Dimensionless {
        &self.mu
    }

    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self {
        self.tnom = tnom;
        self
    }
    pub fn with_ind(&mut self, ind: Henry) -> &mut Self {
        self.ind = ind;
        self
    }
    pub fn with_csect(&mut self, csect: MeterSquared) -> &mut Self {
        self.csect = csect;
        self
    }
    pub fn with_dia(&mut self, dia: Meter) -> &mut Self {
        self.dia = dia;
        self
    }
    pub fn with_length(&mut self, length: Meter) -> &mut Self {
        self.length = length;
        self
    }
    pub fn with_temperature_coefficients(
        &mut self,
        tc1: Dimensionless,
        tc2: Dimensionless,
    ) -> &mut Self {
        self.tc1 = tc1;
        self.tc2 = tc2;
        self
    }
    pub fn with_nt(&mut self, nt: Dimensionless) -> &mut Self {
        self.nt = nt;
        self
    }
    pub fn with_mu(&mut self, mu: Dimensionless) -> &mut Self {
        self.mu = mu;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Inductor;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        format!(".MODEL {} L", self.name)
    }
}

impl InductorModel for DefaultModel {}
