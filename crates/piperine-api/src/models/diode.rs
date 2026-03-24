use crate::devices::Diode;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{
    Ampere, Celsius, Dimensionless, ElectronVolt, Farad, Meter, Ohm, Second, UnitExt, Volt,
};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait DiodeModel: Model<ComponentType = Diode> + SpiceModel + Debug {}

pub static DEFAULT: LazyLock<Arc<dyn DiodeModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default")));

/// Diode model parameters (`.MODEL name D`).
///
/// All parameters from ngspice manual §7.2.1, pp. 132-134.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,

    // --- Junction DC parameters ---
    /// IS: Saturation current (A). Default: 1.0e-14.
    pub is: Ampere,
    /// JSW: Sidewall saturation current (A). Default: 0.0.
    pub jsw: Ampere,
    /// N: Emission coefficient. Default: 1.
    pub n: Dimensionless,
    /// RS: Ohmic resistance (Ω). Default: 0.0.
    pub rs: Ohm,
    /// BV: Reverse breakdown voltage (V). Default: ∞ (None).
    pub bv: Option<Volt>,
    /// IBV: Current at breakdown voltage (A). Default: 1.0e-3.
    pub ibv: Ampere,
    /// NBV: Breakdown emission coefficient. Default: 1.2.
    pub nbv: Dimensionless,
    /// IKF: Forward knee current (A). Default: 0.0.
    pub ikf: Ampere,
    /// IKR: Reverse knee current (A). Default: 0.0.
    pub ikr: Ampere,

    // --- Tunneling ---
    /// JTUN: Tunneling saturation current (A). Default: 0.0.
    pub jtun: Ampere,
    /// JTUNSW: Tunneling sidewall saturation current (A). Default: 0.0.
    pub jtunsw: Ampere,
    /// NTUN: Tunneling emission coefficient. Default: 30.
    pub ntun: Dimensionless,
    /// XTITUN: Tunneling saturation current exponential. Default: 3.
    pub xtitun: Dimensionless,
    /// KEG: EG correction factor for tunneling. Default: 1.0.
    pub keg: Dimensionless,

    // --- Recombination ---
    /// ISR: Recombination saturation current (A). Default: 1e-14.
    pub isr: Ampere,
    /// NR: Recombination emission coefficient. Default: 2.
    pub nr: Dimensionless,

    // --- Junction capacitance ---
    /// CJO: Zero-bias junction capacitance (F). Default: 0.0.
    pub cjo: Farad,
    /// CJP: Zero-bias sidewall junction capacitance (F). Default: 0.0.
    pub cjp: Farad,
    /// FC: Forward-bias depletion bottom-wall cap coefficient. Default: 0.5.
    pub fc: Dimensionless,
    /// FCS: Forward-bias depletion sidewall cap coefficient. Default: 0.5.
    pub fcs: Dimensionless,
    /// M: Area junction grading coefficient. Default: 0.5.
    pub m: Dimensionless,
    /// MJSW: Periphery junction grading coefficient. Default: 0.33.
    pub mjsw: Dimensionless,
    /// VJ: Junction potential (V). Default: 1.0.
    pub vj: Volt,
    /// PHP: Periphery junction potential (V). Default: 1.0.
    pub php: Volt,
    /// TT: Transit-time (s). Default: 0.0.
    pub tt: Second,

    // --- Metal/Polysilicon overlap (level=3) ---
    /// LM: Length of metal capacitor (m). Default: 0.0.
    pub lm: Meter,
    /// LP: Length of polysilicon capacitor (m). Default: 0.0.
    pub lp: Meter,
    /// WM: Width of metal capacitor (m). Default: 0.0.
    pub wm: Meter,
    /// WP: Width of polysilicon capacitor (m). Default: 0.0.
    pub wp: Meter,
    /// XOM: Thickness of metal to bulk oxide (Å). Default: 10000.
    pub xom: Dimensionless,
    /// XOI: Thickness of polysilicon to bulk oxide (Å). Default: 10000.
    pub xoi: Dimensionless,
    /// XM: Masking/etching effects in metal (m). Default: 0.0.
    pub xm: Meter,
    /// XP: Masking/etching effects in polysilicon (m). Default: 0.0.
    pub xp: Meter,
    /// XW: Masking/etching effects (m). Default: 0.0.
    pub xw: Meter,

    // --- Temperature ---
    /// EG: Activation energy (eV). Default: 1.11.
    pub eg: ElectronVolt,
    /// GAP1: First bandgap correction factor (eV). Default: 7.02e-4.
    pub gap1: ElectronVolt,
    /// GAP2: Second bandgap correction factor. Default: 1108.
    pub gap2: Dimensionless,
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
    /// TRS1: 1st order tempco for RS (1/°C). Default: 0.0.
    pub trs1: Dimensionless,
    /// TRS2: 2nd order tempco for RS (1/°C²). Default: 0.0.
    pub trs2: Dimensionless,
    /// TM1: 1st order tempco for MJ (1/°C). Default: 0.0.
    pub tm1: Dimensionless,
    /// TM2: 2nd order tempco for MJ (1/°C²). Default: 0.0.
    pub tm2: Dimensionless,
    /// TTT1: 1st order tempco for TT (1/°C). Default: 0.0.
    pub ttt1: Dimensionless,
    /// TTT2: 2nd order tempco for TT (1/°C²). Default: 0.0.
    pub ttt2: Dimensionless,
    /// XTI: Saturation current temperature exponent. Default: 3.0.
    pub xti: Dimensionless,
    /// TLEV: Temperature equation selector (0,1,2). Default: 0.
    pub tlev: u32,
    /// TLEVC: Capacitance temperature equation selector. Default: 0.
    pub tlevc: u32,
    /// CTA: Area junction cap temperature coefficient (1/°C). Default: 0.0.
    pub cta: Dimensionless,
    /// CTP: Perimeter junction cap temperature coefficient (1/°C). Default: 0.0.
    pub ctp: Dimensionless,
    /// TCV: Breakdown voltage temperature coefficient (1/°C). Default: 0.0.
    pub tcv: Dimensionless,

    // --- Noise ---
    /// KF: Flicker noise coefficient. Default: 0.0.
    pub kf: Dimensionless,
    /// AF: Flicker noise exponent. Default: 1.0.
    pub af: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            // DC
            is: 1.0e-14.A(),
            jsw: 0.0.A(),
            n: 1.0,
            rs: 0.0.Ohms(),
            bv: None,
            ibv: 1.0e-3.A(),
            nbv: 1.2,
            ikf: 0.0.A(),
            ikr: 0.0.A(),
            // Tunneling
            jtun: 0.0.A(),
            jtunsw: 0.0.A(),
            ntun: 30.0,
            xtitun: 3.0,
            keg: 1.0,
            // Recombination
            isr: 1.0e-14.A(),
            nr: 2.0,
            // Capacitance
            cjo: 0.0.F(),
            cjp: 0.0.F(),
            fc: 0.5,
            fcs: 0.5,
            m: 0.5,
            mjsw: 0.33,
            vj: 1.0.V(),
            php: 1.0.V(),
            tt: 0.0.s(),
            // Overlap
            lm: 0.0.m(),
            lp: 0.0.m(),
            wm: 0.0.m(),
            wp: 0.0.m(),
            xom: 10000.0,
            xoi: 10000.0,
            xm: 0.0.m(),
            xp: 0.0.m(),
            xw: 0.0.m(),
            // Temperature
            eg: 1.11,
            gap1: 7.02e-4,
            gap2: 1108.0,
            tnom: 27.0.deg_C(),
            trs1: 0.0.inv_C(),
            trs2: 0.0.inv_C2(),
            tm1: 0.0.inv_C(),
            tm2: 0.0.inv_C2(),
            ttt1: 0.0.inv_C(),
            ttt2: 0.0.inv_C2(),
            xti: 3.0,
            tlev: 0,
            tlevc: 0,
            cta: 0.0.inv_C(),
            ctp: 0.0.inv_C(),
            tcv: 0.0.inv_C(),
            // Noise
            kf: 0.0,
            af: 1.0,
        }
    }

    pub fn name(&self) -> &String { &self.name }

    pub fn with_is(&mut self, is: Ampere) -> &mut Self { self.is = is; self }
    pub fn with_jsw(&mut self, jsw: Ampere) -> &mut Self { self.jsw = jsw; self }
    pub fn with_n(&mut self, n: Dimensionless) -> &mut Self { self.n = n; self }
    pub fn with_rs(&mut self, rs: Ohm) -> &mut Self { self.rs = rs; self }
    pub fn with_bv(&mut self, bv: Volt) -> &mut Self { self.bv = Some(bv); self }
    pub fn with_ibv(&mut self, ibv: Ampere) -> &mut Self { self.ibv = ibv; self }
    pub fn with_nbv(&mut self, nbv: Dimensionless) -> &mut Self { self.nbv = nbv; self }
    pub fn with_ikf(&mut self, ikf: Ampere) -> &mut Self { self.ikf = ikf; self }
    pub fn with_ikr(&mut self, ikr: Ampere) -> &mut Self { self.ikr = ikr; self }
    pub fn with_cjo(&mut self, cjo: Farad) -> &mut Self { self.cjo = cjo; self }
    pub fn with_cjp(&mut self, cjp: Farad) -> &mut Self { self.cjp = cjp; self }
    pub fn with_fc(&mut self, fc: Dimensionless) -> &mut Self { self.fc = fc; self }
    pub fn with_m(&mut self, m: Dimensionless) -> &mut Self { self.m = m; self }
    pub fn with_mjsw(&mut self, mjsw: Dimensionless) -> &mut Self { self.mjsw = mjsw; self }
    pub fn with_vj(&mut self, vj: Volt) -> &mut Self { self.vj = vj; self }
    pub fn with_tt(&mut self, tt: Second) -> &mut Self { self.tt = tt; self }
    pub fn with_eg(&mut self, eg: ElectronVolt) -> &mut Self { self.eg = eg; self }
    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self { self.tnom = tnom; self }
    pub fn with_xti(&mut self, xti: Dimensionless) -> &mut Self { self.xti = xti; self }
    pub fn with_tlev(&mut self, tlev: u32) -> &mut Self { self.tlev = tlev; self }
    pub fn with_tlevc(&mut self, tlevc: u32) -> &mut Self { self.tlevc = tlevc; self }
    pub fn with_noise_parameters(&mut self, kf: Dimensionless, af: Dimensionless) -> &mut Self {
        self.kf = kf;
        self.af = af;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Diode;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        format!(".MODEL {} D", self.name)
    }
}

impl DiodeModel for DefaultModel {}
