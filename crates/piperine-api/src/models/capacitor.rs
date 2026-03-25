use crate::devices::Capacitor;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{
    Celsius, Dimensionless, Farad, FaradPerMeter, FaradPerMeterSquared, Meter, UnitExt,
};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait CapacitorModel: Model<ComponentType = Capacitor> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn CapacitorModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

/// Semiconductor capacitor model parameters.
///
/// All parameters from ngspice manual §3.3.8.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
    /// CAP: Default capacitance (F). Default: 0.0.
    pub cap: Farad,
    /// CJ: Bottom junction capacitance per area (F/m²). Default: 0.0.
    pub cj: FaradPerMeterSquared,
    /// CJSW: Sidewall junction capacitance per length (F/m). Default: 0.0.
    pub cjsw: FaradPerMeter,
    /// DEFW: Default width (m). Default: 1e-6.
    pub defw: Meter,
    /// DEFL: Default length (m). Default: 0.0.
    pub defl: Meter,
    /// NARROW: Narrowing due to side etching (m). Default: 0.0.
    pub narrow: Meter,
    /// SHORT: Shortening due to side etching (m). Default: 0.0.
    pub short: Meter,
    /// TC1: First order temperature coefficient (1/°C). Default: 0.0.
    pub tc1: Dimensionless,
    /// TC2: Second order temperature coefficient (1/°C²). Default: 0.0.
    pub tc2: Dimensionless,
    /// DI: Relative dielectric constant. Default: 0.0.
    pub di: Dimensionless,
    /// THICK: Insulator thickness (m). Default: 0.0.
    pub thick: Meter,
    /// VC1: First order voltage coefficient. Default: 0.0.
    pub vc1: Dimensionless,
    /// VC2: Second order voltage coefficient. Default: 0.0.
    pub vc2: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tnom: 27.0.deg_C(),
            cap: 0.0.F(),
            cj: 0.0,
            cjsw: 0.0,
            defw: 1.0.um(),
            defl: 0.0.m(),
            narrow: 0.0.m(),
            short: 0.0.m(),
            tc1: 0.0.inv_C(),
            tc2: 0.0.inv_C2(),
            di: 0.0,
            thick: 0.0.m(),
            vc1: 0.0,
            vc2: 0.0,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn tnom(&self) -> &Celsius {
        &self.tnom
    }
    pub fn cap(&self) -> &Farad {
        &self.cap
    }
    pub fn cj(&self) -> &FaradPerMeterSquared {
        &self.cj
    }
    pub fn cjsw(&self) -> &FaradPerMeter {
        &self.cjsw
    }
    pub fn defw(&self) -> &Meter {
        &self.defw
    }
    pub fn defl(&self) -> &Meter {
        &self.defl
    }
    pub fn narrow(&self) -> &Meter {
        &self.narrow
    }
    pub fn short(&self) -> &Meter {
        &self.short
    }
    pub fn tc1(&self) -> &Dimensionless {
        &self.tc1
    }
    pub fn tc2(&self) -> &Dimensionless {
        &self.tc2
    }
    pub fn di(&self) -> &Dimensionless {
        &self.di
    }
    pub fn thick(&self) -> &Meter {
        &self.thick
    }
    pub fn vc1(&self) -> &Dimensionless {
        &self.vc1
    }
    pub fn vc2(&self) -> &Dimensionless {
        &self.vc2
    }

    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self {
        self.tnom = tnom;
        self
    }
    pub fn with_cap(&mut self, cap: Farad) -> &mut Self {
        self.cap = cap;
        self
    }
    pub fn with_cj(&mut self, cj: FaradPerMeterSquared) -> &mut Self {
        self.cj = cj;
        self
    }
    pub fn with_cjsw(&mut self, cjsw: FaradPerMeter) -> &mut Self {
        self.cjsw = cjsw;
        self
    }
    pub fn with_defw(&mut self, defw: Meter) -> &mut Self {
        self.defw = defw;
        self
    }
    pub fn with_defl(&mut self, defl: Meter) -> &mut Self {
        self.defl = defl;
        self
    }
    pub fn with_narrow(&mut self, narrow: Meter) -> &mut Self {
        self.narrow = narrow;
        self
    }
    pub fn with_short(&mut self, short: Meter) -> &mut Self {
        self.short = short;
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
    pub fn with_di(&mut self, di: Dimensionless) -> &mut Self {
        self.di = di;
        self
    }
    pub fn with_thick(&mut self, thick: Meter) -> &mut Self {
        self.thick = thick;
        self
    }
    pub fn with_voltage_coefficients(
        &mut self,
        vc1: Dimensionless,
        vc2: Dimensionless,
    ) -> &mut Self {
        self.vc1 = vc1;
        self.vc2 = vc2;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Capacitor;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        format!(".MODEL {} C", self.name)
    }
}

impl CapacitorModel for DefaultModel {}
