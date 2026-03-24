use crate::devices::Jfet;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{
    Ampere, Celsius, Dimensionless, ElectronVolt, Farad, Ohm, UnitExt, Volt,
};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait JfetModel: Model<ComponentType = Jfet> + SpiceModel + Debug {}

pub static DEFAULT_NJF: LazyLock<Arc<dyn JfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", JfetType::Njf)));

pub static DEFAULT_PJF: LazyLock<Arc<dyn JfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", JfetType::Pjf)));

/// JFET polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JfetType {
    Njf,
    Pjf,
}

/// JFET Level 1 model parameters (`.MODEL name NJF/PJF`).
///
/// All parameters from ngspice manual §7.4, p. 151.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub jfet_type: JfetType,

    // --- DC ---
    /// VTO: Threshold voltage (V). Default: -2.0.
    pub vto: Volt,
    /// BETA: Transconductance parameter (A/V²). Default: 1.0e-4.
    pub beta: Dimensionless,
    /// LAMBDA: Channel-length modulation (1/V). Default: 0.0.
    pub lambda: Dimensionless,
    /// RD: Drain ohmic resistance (Ω). Default: 0.0.
    pub rd: Ohm,
    /// RS: Source ohmic resistance (Ω). Default: 0.0.
    pub rs: Ohm,
    /// IS: Gate saturation current (A). Default: 1.0e-14.
    pub is: Ampere,
    /// B: Doping tail parameter. Default: 1.0.
    pub b: Dimensionless,

    // --- Capacitance ---
    /// CGS: Zero-bias G-S junction capacitance (F). Default: 0.0.
    pub cgs: Farad,
    /// CGD: Zero-bias G-D junction capacitance (F). Default: 0.0.
    pub cgd: Farad,
    /// PB: Gate junction potential (V). Default: 1.0.
    pub pb: Volt,
    /// FC: Forward-bias depletion cap coefficient. Default: 0.5.
    pub fc: Dimensionless,

    // --- Temperature ---
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
    /// TCV: Threshold voltage temperature coefficient (1/°C). Default: 0.0.
    pub tcv: Dimensionless,
    /// VTOTC: Threshold voltage temperature coefficient (alternative) (1/°C). Default: 0.0.
    pub vtotc: Dimensionless,
    /// BEX: Mobility temperature exponent. Default: 0.0.
    pub bex: Dimensionless,
    /// BETATCE: Mobility temperature coefficient (%/°C, alternative). Default: 0.0.
    pub betatce: Dimensionless,
    /// XTI: Gate saturation current temperature coefficient. Default: 3.0.
    pub xti: Dimensionless,
    /// EG: Bandgap voltage (eV). Default: 1.11.
    pub eg: ElectronVolt,

    // --- Noise ---
    /// KF: Flicker noise coefficient. Default: 0.0.
    pub kf: Dimensionless,
    /// AF: Flicker noise exponent. Default: 1.0.
    pub af: Dimensionless,
    /// NLEV: Noise equation selector. Default: 1.
    pub nlev: u32,
    /// GDSNOI: Channel noise coefficient for nlev=3. Default: 1.0.
    pub gdsnoi: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, jfet_type: JfetType) -> Self {
        Self {
            name: name.into(),
            jfet_type,
            // DC
            vto: -2.0.V(),
            beta: 1.0e-4,
            lambda: 0.0,
            rd: 0.0.Ohms(),
            rs: 0.0.Ohms(),
            is: 1.0e-14.A(),
            b: 1.0,
            // Capacitance
            cgs: 0.0.F(),
            cgd: 0.0.F(),
            pb: 1.0.V(),
            fc: 0.5,
            // Temperature
            tnom: 27.0.deg_C(),
            tcv: 0.0.inv_C(),
            vtotc: 0.0.inv_C(),
            bex: 0.0,
            betatce: 0.0,
            xti: 3.0,
            eg: 1.11,
            // Noise
            kf: 0.0,
            af: 1.0,
            nlev: 1,
            gdsnoi: 1.0,
        }
    }

    pub fn name(&self) -> &String { &self.name }
    pub fn jfet_type(&self) -> JfetType { self.jfet_type }

    pub fn with_vto(&mut self, vto: Volt) -> &mut Self { self.vto = vto; self }
    pub fn with_beta(&mut self, beta: Dimensionless) -> &mut Self { self.beta = beta; self }
    pub fn with_lambda(&mut self, lambda: Dimensionless) -> &mut Self { self.lambda = lambda; self }
    pub fn with_rd(&mut self, rd: Ohm) -> &mut Self { self.rd = rd; self }
    pub fn with_rs(&mut self, rs: Ohm) -> &mut Self { self.rs = rs; self }
    pub fn with_is(&mut self, is: Ampere) -> &mut Self { self.is = is; self }
    pub fn with_b(&mut self, b: Dimensionless) -> &mut Self { self.b = b; self }
    pub fn with_cgs(&mut self, cgs: Farad) -> &mut Self { self.cgs = cgs; self }
    pub fn with_cgd(&mut self, cgd: Farad) -> &mut Self { self.cgd = cgd; self }
    pub fn with_pb(&mut self, pb: Volt) -> &mut Self { self.pb = pb; self }
    pub fn with_fc(&mut self, fc: Dimensionless) -> &mut Self { self.fc = fc; self }
    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self { self.tnom = tnom; self }
    pub fn with_eg(&mut self, eg: ElectronVolt) -> &mut Self { self.eg = eg; self }
    pub fn with_xti(&mut self, xti: Dimensionless) -> &mut Self { self.xti = xti; self }
    pub fn with_noise_parameters(&mut self, kf: Dimensionless, af: Dimensionless) -> &mut Self {
        self.kf = kf;
        self.af = af;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Jfet;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let spice_type = match self.jfet_type {
            JfetType::Njf => "NJF",
            JfetType::Pjf => "PJF",
        };
        format!(".MODEL {} {}", self.name, spice_type)
    }
}

impl JfetModel for DefaultModel {}
