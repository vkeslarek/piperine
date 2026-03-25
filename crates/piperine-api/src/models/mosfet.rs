use crate::devices::Mosfet;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{
    Celsius, Dimensionless, Farad, FaradPerMeter, Meter, Ohm, OhmPerSquare, UnitExt, Volt,
};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait MosfetModel: Model<ComponentType = Mosfet> + SpiceModel + Debug {}

pub static DEFAULT_NMOS: LazyLock<Arc<dyn MosfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", MosfetType::Nmos)));

pub static DEFAULT_PMOS: LazyLock<Arc<dyn MosfetModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", MosfetType::Pmos)));

/// MOSFET polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MosfetType {
    Nmos,
    Pmos,
}

/// MOSFET Level 1-3 model parameters (`.MODEL name NMOS/PMOS`).
///
/// All parameters from ngspice manual §7.6.2, pp. 160-161.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub mos_type: MosfetType,

    // --- DC ---
    /// LEVEL: Model index. Default: 1.
    pub level: u32,
    /// VTO: Zero-bias threshold voltage (V). Default: 0.0.
    pub vto: Volt,
    /// KP: Transconductance parameter (A/V²). Default: 2.0e-5.
    pub kp: Dimensionless,
    /// GAMMA: Bulk threshold parameter (√V). Default: 0.0.
    pub gamma: Dimensionless,
    /// PHI: Surface potential (V). Default: 0.6.
    pub phi: Volt,
    /// LAMBDA: Channel-length modulation (1/V). Default: 0.0.
    pub lambda: Dimensionless,
    /// RD: Drain ohmic resistance (Ω). Default: 0.0.
    pub rd: Ohm,
    /// RS: Source ohmic resistance (Ω). Default: 0.0.
    pub rs: Ohm,
    /// CBD: Zero-bias B-D junction capacitance (F). Default: 0.0.
    pub cbd: Farad,
    /// CBS: Zero-bias B-S junction capacitance (F). Default: 0.0.
    pub cbs: Farad,
    /// IS: Bulk junction saturation current (A). Default: 1.0e-14.
    pub is: Dimensionless,
    /// PB: Bulk junction potential (V). Default: 0.8.
    pub pb: Volt,
    /// CGSO: Gate-source overlap capacitance per meter (F/m). Default: 0.0.
    pub cgso: FaradPerMeter,
    /// CGDO: Gate-drain overlap capacitance per meter (F/m). Default: 0.0.
    pub cgdo: FaradPerMeter,
    /// CGBO: Gate-bulk overlap capacitance per meter (F/m). Default: 0.0.
    pub cgbo: FaradPerMeter,
    /// RSH: Drain and source diffusion sheet resistance (Ω/□). Default: 0.0.
    pub rsh: OhmPerSquare,
    /// CJ: Zero-bias bulk junction bottom cap per area (F/m²). Default: 0.0.
    pub cj: Dimensionless,
    /// MJ: Bulk junction bottom grading coefficient. Default: 0.5.
    pub mj: Dimensionless,
    /// CJSW: Zero-bias bulk junction sidewall cap per meter (F/m). Default: 0.0.
    pub cjsw: FaradPerMeter,
    /// MJSW: Bulk junction sidewall grading coefficient. Default: 0.5 (level1) / 0.33 (level2,3).
    pub mjsw: Dimensionless,
    /// JS: Bulk junction saturation current density. Default: 0.0.
    pub js: Dimensionless,
    /// TOX: Oxide thickness (m). Default: 1.0e-7.
    pub tox: Meter,
    /// NSUB: Substrate doping (cm⁻³). Default: 0.0.
    pub nsub: Dimensionless,
    /// NSS: Surface state density (cm⁻²). Default: 0.0.
    pub nss: Dimensionless,
    /// NFS: Fast surface state density (cm⁻²). Default: 0.0.
    pub nfs: Dimensionless,
    /// TPG: Type of gate material (+1 opp, -1 same, 0 Al). Default: 1.
    pub tpg: Dimensionless,
    /// XJ: Metallurgical junction depth (m). Default: 0.0.
    pub xj: Meter,
    /// LD: Lateral diffusion (m). Default: 0.0.
    pub ld: Meter,
    /// UO: Surface mobility (cm²/V·s). Default: 600.
    pub uo: Dimensionless,
    /// UCRIT: Critical field for mobility degradation (V/cm, MOS2). Default: 1.0e4.
    pub ucrit: Dimensionless,
    /// UEXP: Critical field exponent (MOS2). Default: 0.0.
    pub uexp: Dimensionless,
    /// UTRA: Transverse field coefficient (MOS2). Default: 0.0.
    pub utra: Dimensionless,
    /// VMAX: Maximum drift velocity (m/s). Default: 0.0.
    pub vmax: Dimensionless,
    /// NEFF: Total channel-charge coefficient (MOS2). Default: 1.0.
    pub neff: Dimensionless,
    /// FC: Coefficient for forward-bias depletion cap. Default: 0.5.
    pub fc: Dimensionless,

    // --- MOS2/3 only ---
    /// DELTA: Width effect on threshold voltage (MOS2/3). Default: 0.0.
    pub delta: Dimensionless,
    /// THETA: Mobility modulation (1/V, MOS3). Default: 0.0.
    pub theta: Dimensionless,
    /// ETA: Static feedback (MOS3). Default: 0.0.
    pub eta: Dimensionless,
    /// KAPPA: Saturation field factor (MOS3). Default: 0.2.
    pub kappa: Dimensionless,

    // --- Noise ---
    /// KF: Flicker noise coefficient. Default: 0.0.
    pub kf: Dimensionless,
    /// AF: Flicker noise exponent. Default: 1.0.
    pub af: Dimensionless,
    /// NLEV: Noise equation selector. Default: 1.
    pub nlev: u32,
    /// GDSNOI: Channel noise coefficient for nlev=3. Default: 1.0.
    pub gdsnoi: Dimensionless,

    // --- Temperature ---
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, mos_type: MosfetType) -> Self {
        Self {
            name: name.into(),
            mos_type,
            level: 1,
            vto: 0.0.V(),
            kp: 2.0e-5,
            gamma: 0.0,
            phi: 0.6.V(),
            lambda: 0.0,
            rd: 0.0.Ohms(),
            rs: 0.0.Ohms(),
            cbd: 0.0.F(),
            cbs: 0.0.F(),
            is: 1.0e-14,
            pb: 0.8.V(),
            cgso: 0.0,
            cgdo: 0.0,
            cgbo: 0.0,
            rsh: 0.0,
            cj: 0.0,
            mj: 0.5,
            cjsw: 0.0,
            mjsw: 0.5,
            js: 0.0,
            tox: 1.0e-7.m(),
            nsub: 0.0,
            nss: 0.0,
            nfs: 0.0,
            tpg: 1.0,
            xj: 0.0.m(),
            ld: 0.0.m(),
            uo: 600.0,
            ucrit: 1.0e4,
            uexp: 0.0,
            utra: 0.0,
            vmax: 0.0,
            neff: 1.0,
            fc: 0.5,
            delta: 0.0,
            theta: 0.0,
            eta: 0.0,
            kappa: 0.2,
            kf: 0.0,
            af: 1.0,
            nlev: 1,
            gdsnoi: 1.0,
            tnom: 27.0.deg_C(),
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn mos_type(&self) -> MosfetType {
        self.mos_type
    }

    pub fn with_level(&mut self, level: u32) -> &mut Self {
        self.level = level;
        self
    }
    pub fn with_vto(&mut self, vto: Volt) -> &mut Self {
        self.vto = vto;
        self
    }
    pub fn with_kp(&mut self, kp: Dimensionless) -> &mut Self {
        self.kp = kp;
        self
    }
    pub fn with_gamma(&mut self, gamma: Dimensionless) -> &mut Self {
        self.gamma = gamma;
        self
    }
    pub fn with_phi(&mut self, phi: Volt) -> &mut Self {
        self.phi = phi;
        self
    }
    pub fn with_lambda(&mut self, lambda: Dimensionless) -> &mut Self {
        self.lambda = lambda;
        self
    }
    pub fn with_rd(&mut self, rd: Ohm) -> &mut Self {
        self.rd = rd;
        self
    }
    pub fn with_rs(&mut self, rs: Ohm) -> &mut Self {
        self.rs = rs;
        self
    }
    pub fn with_tox(&mut self, tox: Meter) -> &mut Self {
        self.tox = tox;
        self
    }
    pub fn with_nsub(&mut self, nsub: Dimensionless) -> &mut Self {
        self.nsub = nsub;
        self
    }
    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self {
        self.tnom = tnom;
        self
    }
    pub fn with_noise_parameters(&mut self, kf: Dimensionless, af: Dimensionless) -> &mut Self {
        self.kf = kf;
        self.af = af;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Mosfet;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let spice_type = match self.mos_type {
            MosfetType::Nmos => "NMOS",
            MosfetType::Pmos => "PMOS",
        };
        format!(".MODEL {} {}", self.name, spice_type)
    }
}

impl MosfetModel for DefaultModel {}
