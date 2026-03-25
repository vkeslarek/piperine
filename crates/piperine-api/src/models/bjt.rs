use crate::devices::Bjt;
use crate::models::Model;
use crate::spice::SpiceModel;
use crate::units::{
    Ampere, Celsius, Coulomb, Degree, Dimensionless, ElectronVolt, Farad, Ohm, Second, UnitExt,
    Volt,
};
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

pub trait BjtModel: Model<ComponentType = Bjt> + SpiceModel + Debug {}

pub static DEFAULT_NPN: LazyLock<Arc<dyn BjtModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", BjtType::Npn)));

pub static DEFAULT_PNP: LazyLock<Arc<dyn BjtModel + Send + Sync>> =
    LazyLock::new(|| Arc::new(DefaultModel::new("default", BjtType::Pnp)));

/// BJT polarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BjtType {
    Npn,
    Pnp,
}

/// Gummel-Poon BJT model parameters (`.MODEL name NPN/PNP`).
///
/// All parameters from ngspice manual §7.3.3, pp. 141-145.
#[derive(Debug)]
pub struct DefaultModel {
    pub name: String,
    pub bjt_type: BjtType,

    // --- DC parameters ---
    /// SUBS: Substrate connection (1=vertical, -1=lateral). Default: 1.
    pub subs: i32,
    /// IS: Transport saturation current (A). Default: 1.0e-16.
    pub is: Ampere,
    /// IBE: Base-Emitter saturation current (A). Default: 0.0.
    pub ibe: Ampere,
    /// IBC: Base-Collector saturation current (A). Default: 0.0.
    pub ibc: Ampere,
    /// ISS: Reverse saturation current (A). Default: 0.0.
    pub iss: Ampere,
    /// BF: Ideal maximum forward beta. Default: 100.
    pub bf: Dimensionless,
    /// NF: Forward current emission coefficient. Default: 1.0.
    pub nf: Dimensionless,
    /// VAF: Forward Early voltage (V). Default: ∞ (None).
    pub vaf: Option<Volt>,
    /// IKF: Forward beta corner current (A). Default: ∞ (None).
    pub ikf: Option<Ampere>,
    /// NKF: High current beta rolloff exponent. Default: 0.5.
    pub nkf: Dimensionless,
    /// ISE: B-E leakage saturation current (A). Default: 0.0.
    pub ise: Ampere,
    /// NE: B-E leakage emission coefficient. Default: 1.5.
    pub ne: Dimensionless,
    /// BR: Ideal maximum reverse beta. Default: 1.
    pub br: Dimensionless,
    /// NR: Reverse current emission coefficient. Default: 1.
    pub nr: Dimensionless,
    /// VAR: Reverse Early voltage (V). Default: ∞ (None).
    pub var: Option<Volt>,
    /// IKR: Reverse beta corner current (A). Default: ∞ (None).
    pub ikr: Option<Ampere>,
    /// ISC: B-C leakage saturation current (A). Default: 0.0.
    pub isc: Ampere,
    /// NC: B-C leakage emission coefficient. Default: 2.
    pub nc: Dimensionless,
    /// RB: Zero bias base resistance (Ω). Default: 0.
    pub rb: Ohm,
    /// IRB: Current where base resistance falls halfway (A). Default: ∞ (None).
    pub irb: Option<Ampere>,
    /// RBM: Minimum base resistance at high currents (Ω). Default: 0.
    pub rbm: Ohm,
    /// RE: Emitter resistance (Ω). Default: 0.
    pub re: Ohm,
    /// RC: Collector resistance (Ω). Default: 0.
    pub rc: Ohm,

    // --- Capacitance ---
    /// CJE: B-E zero-bias depletion capacitance (F). Default: 0.
    pub cje: Farad,
    /// VJE: B-E built-in potential (V). Default: 0.75.
    pub vje: Volt,
    /// MJE: B-E junction exponential factor. Default: 0.33.
    pub mje: Dimensionless,
    /// TF: Ideal forward transit time (s). Default: 0.
    pub tf: Second,
    /// XTF: Coefficient for bias dependence of TF. Default: 0.
    pub xtf: Dimensionless,
    /// VTF: Voltage describing VBC dependence of TF (V). Default: ∞ (None).
    pub vtf: Option<Volt>,
    /// ITF: High-current parameter for effect on TF (A). Default: 0.
    pub itf: Ampere,
    /// PTF: Excess phase at freq=1/(2πTF) Hz (deg). Default: 0.
    pub ptf: Degree,
    /// CJC: B-C zero-bias depletion capacitance (F). Default: 0.
    pub cjc: Farad,
    /// VJC: B-C built-in potential (V). Default: 0.75.
    pub vjc: Volt,
    /// MJC: B-C junction exponential factor. Default: 0.33.
    pub mjc: Dimensionless,
    /// XCJC: Fraction of B-C cap connected to internal base. Default: 1.
    pub xcjc: Dimensionless,
    /// TR: Ideal reverse transit time (s). Default: 0.
    pub tr: Second,
    /// CJS: C-S zero-bias depletion capacitance (F). Default: 0.
    pub cjs: Farad,
    /// VJS: Substrate junction built-in potential (V). Default: 0.75.
    pub vjs: Volt,
    /// MJS: Substrate junction exponential factor. Default: 0.
    pub mjs: Dimensionless,
    /// FC: Coefficient for forward-bias depletion cap formula. Default: 0.5.
    pub fc: Dimensionless,

    // --- Temperature ---
    /// TNOM: Parameter measurement temperature (°C). Default: 27.
    pub tnom: Celsius,
    /// XTI: Temperature exponent for IS. Default: 3.
    pub xti: Dimensionless,
    /// XTB: Forward/reverse beta temperature exponent. Default: 0.
    pub xtb: Dimensionless,
    /// EG: Energy gap for temperature effect on IS (eV). Default: 1.11.
    pub eg: ElectronVolt,
    /// TLEV: BJT temperature equation selector. Default: 0.
    pub tlev: u32,
    /// TLEVC: BJT capacitance temperature equation selector. Default: 0.
    pub tlevc: u32,

    // Temperature coefficients (all default 0.0)
    pub tre1: Dimensionless,
    pub tre2: Dimensionless,
    pub trc1: Dimensionless,
    pub trc2: Dimensionless,
    pub trb1: Dimensionless,
    pub trb2: Dimensionless,
    pub trbm1: Dimensionless,
    pub trbm2: Dimensionless,
    pub tbf1: Dimensionless,
    pub tbf2: Dimensionless,
    pub tbr1: Dimensionless,
    pub tbr2: Dimensionless,
    pub tikf1: Dimensionless,
    pub tikf2: Dimensionless,
    pub tikr1: Dimensionless,
    pub tikr2: Dimensionless,
    pub tirb1: Dimensionless,
    pub tirb2: Dimensionless,
    pub tnc1: Dimensionless,
    pub tnc2: Dimensionless,
    pub tne1: Dimensionless,
    pub tne2: Dimensionless,
    pub tnf1: Dimensionless,
    pub tnf2: Dimensionless,
    pub tnr1: Dimensionless,
    pub tnr2: Dimensionless,
    pub tvaf1: Dimensionless,
    pub tvaf2: Dimensionless,
    pub tvar1: Dimensionless,
    pub tvar2: Dimensionless,
    pub ctc: Dimensionless,
    pub cte: Dimensionless,
    pub cts: Dimensionless,
    pub tvjc: Dimensionless,
    pub tvje: Dimensionless,
    pub titf1: Dimensionless,
    pub titf2: Dimensionless,
    pub ttf1: Dimensionless,
    pub ttf2: Dimensionless,
    pub ttr1: Dimensionless,
    pub ttr2: Dimensionless,
    pub tmje1: Dimensionless,
    pub tmje2: Dimensionless,
    pub tmjc1: Dimensionless,
    pub tmjc2: Dimensionless,

    // --- Quasi-saturation ---
    /// RCO: Epitaxial region resistance (Ω). Default: 0.
    pub rco: Ohm,
    /// VO: Carrier mobility knee voltage (V). Default: 10.
    pub vo: Volt,
    /// GAMMA: Epitaxial region doping factor. Default: 1e-11.
    pub gamma: Dimensionless,
    /// QCO: Epitaxial region charge factor (C). Default: 0.
    pub qco: Coulomb,
    /// VG: Energy gap for QS temperature dependence (V). Default: 1.206.
    pub vg: Volt,
    /// CN: Temperature exponent of RCI. Default: 2.42.
    pub cn: Dimensionless,
    /// D: Temperature exponent of VO. Default: 0.87.
    pub d: Dimensionless,

    // --- Noise ---
    /// KF: Flicker-noise coefficient. Default: 0.
    pub kf: Dimensionless,
    /// AF: Flicker-noise exponent. Default: 1.
    pub af: Dimensionless,
}

impl DefaultModel {
    pub fn new(name: impl Into<String>, bjt_type: BjtType) -> Self {
        Self {
            name: name.into(),
            bjt_type,
            // DC
            subs: 1,
            is: 1.0e-16.A(),
            ibe: 0.0.A(),
            ibc: 0.0.A(),
            iss: 0.0.A(),
            bf: 100.0,
            nf: 1.0,
            vaf: None,
            ikf: None,
            nkf: 0.5,
            ise: 0.0.A(),
            ne: 1.5,
            br: 1.0,
            nr: 1.0,
            var: None,
            ikr: None,
            isc: 0.0.A(),
            nc: 2.0,
            rb: 0.0.Ohms(),
            irb: None,
            rbm: 0.0.Ohms(),
            re: 0.0.Ohms(),
            rc: 0.0.Ohms(),
            // Capacitance
            cje: 0.0.F(),
            vje: 0.75.V(),
            mje: 0.33,
            tf: 0.0.s(),
            xtf: 0.0,
            vtf: None,
            itf: 0.0.A(),
            ptf: 0.0,
            cjc: 0.0.F(),
            vjc: 0.75.V(),
            mjc: 0.33,
            xcjc: 1.0,
            tr: 0.0.s(),
            cjs: 0.0.F(),
            vjs: 0.75.V(),
            mjs: 0.0,
            fc: 0.5,
            // Temperature
            tnom: 27.0.deg_C(),
            xti: 3.0,
            xtb: 0.0,
            eg: 1.11,
            tlev: 0,
            tlevc: 0,
            tre1: 0.0,
            tre2: 0.0,
            trc1: 0.0,
            trc2: 0.0,
            trb1: 0.0,
            trb2: 0.0,
            trbm1: 0.0,
            trbm2: 0.0,
            tbf1: 0.0,
            tbf2: 0.0,
            tbr1: 0.0,
            tbr2: 0.0,
            tikf1: 0.0,
            tikf2: 0.0,
            tikr1: 0.0,
            tikr2: 0.0,
            tirb1: 0.0,
            tirb2: 0.0,
            tnc1: 0.0,
            tnc2: 0.0,
            tne1: 0.0,
            tne2: 0.0,
            tnf1: 0.0,
            tnf2: 0.0,
            tnr1: 0.0,
            tnr2: 0.0,
            tvaf1: 0.0,
            tvaf2: 0.0,
            tvar1: 0.0,
            tvar2: 0.0,
            ctc: 0.0,
            cte: 0.0,
            cts: 0.0,
            tvjc: 0.0,
            tvje: 0.0,
            titf1: 0.0,
            titf2: 0.0,
            ttf1: 0.0,
            ttf2: 0.0,
            ttr1: 0.0,
            ttr2: 0.0,
            tmje1: 0.0,
            tmje2: 0.0,
            tmjc1: 0.0,
            tmjc2: 0.0,
            // Quasi-saturation
            rco: 0.0.Ohms(),
            vo: 10.0.V(),
            gamma: 1.0e-11,
            qco: 0.0,
            vg: 1.206.V(),
            cn: 2.42,
            d: 0.87,
            // Noise
            kf: 0.0,
            af: 1.0,
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }
    pub fn bjt_type(&self) -> BjtType {
        self.bjt_type
    }

    pub fn with_is(&mut self, is: Ampere) -> &mut Self {
        self.is = is;
        self
    }
    pub fn with_bf(&mut self, bf: Dimensionless) -> &mut Self {
        self.bf = bf;
        self
    }
    pub fn with_nf(&mut self, nf: Dimensionless) -> &mut Self {
        self.nf = nf;
        self
    }
    pub fn with_vaf(&mut self, vaf: Volt) -> &mut Self {
        self.vaf = Some(vaf);
        self
    }
    pub fn with_ikf(&mut self, ikf: Ampere) -> &mut Self {
        self.ikf = Some(ikf);
        self
    }
    pub fn with_ise(&mut self, ise: Ampere) -> &mut Self {
        self.ise = ise;
        self
    }
    pub fn with_ne(&mut self, ne: Dimensionless) -> &mut Self {
        self.ne = ne;
        self
    }
    pub fn with_br(&mut self, br: Dimensionless) -> &mut Self {
        self.br = br;
        self
    }
    pub fn with_nr(&mut self, nr: Dimensionless) -> &mut Self {
        self.nr = nr;
        self
    }
    pub fn with_var(&mut self, var: Volt) -> &mut Self {
        self.var = Some(var);
        self
    }
    pub fn with_ikr(&mut self, ikr: Ampere) -> &mut Self {
        self.ikr = Some(ikr);
        self
    }
    pub fn with_rb(&mut self, rb: Ohm) -> &mut Self {
        self.rb = rb;
        self
    }
    pub fn with_re(&mut self, re: Ohm) -> &mut Self {
        self.re = re;
        self
    }
    pub fn with_rc(&mut self, rc: Ohm) -> &mut Self {
        self.rc = rc;
        self
    }
    pub fn with_cje(&mut self, cje: Farad) -> &mut Self {
        self.cje = cje;
        self
    }
    pub fn with_vje(&mut self, vje: Volt) -> &mut Self {
        self.vje = vje;
        self
    }
    pub fn with_mje(&mut self, mje: Dimensionless) -> &mut Self {
        self.mje = mje;
        self
    }
    pub fn with_tf(&mut self, tf: Second) -> &mut Self {
        self.tf = tf;
        self
    }
    pub fn with_cjc(&mut self, cjc: Farad) -> &mut Self {
        self.cjc = cjc;
        self
    }
    pub fn with_vjc(&mut self, vjc: Volt) -> &mut Self {
        self.vjc = vjc;
        self
    }
    pub fn with_mjc(&mut self, mjc: Dimensionless) -> &mut Self {
        self.mjc = mjc;
        self
    }
    pub fn with_tr(&mut self, tr: Second) -> &mut Self {
        self.tr = tr;
        self
    }
    pub fn with_cjs(&mut self, cjs: Farad) -> &mut Self {
        self.cjs = cjs;
        self
    }
    pub fn with_fc(&mut self, fc: Dimensionless) -> &mut Self {
        self.fc = fc;
        self
    }
    pub fn with_tnom(&mut self, tnom: Celsius) -> &mut Self {
        self.tnom = tnom;
        self
    }
    pub fn with_eg(&mut self, eg: ElectronVolt) -> &mut Self {
        self.eg = eg;
        self
    }
    pub fn with_xti(&mut self, xti: Dimensionless) -> &mut Self {
        self.xti = xti;
        self
    }
    pub fn with_xtb(&mut self, xtb: Dimensionless) -> &mut Self {
        self.xtb = xtb;
        self
    }
    pub fn with_noise_parameters(&mut self, kf: Dimensionless, af: Dimensionless) -> &mut Self {
        self.kf = kf;
        self.af = af;
        self
    }
}

impl Model for DefaultModel {
    type ComponentType = Bjt;
}

impl SpiceModel for DefaultModel {
    fn model_name(&self) -> &str {
        &self.name
    }

    fn to_spice_model_line(&self) -> String {
        let spice_type = match self.bjt_type {
            BjtType::Npn => "NPN",
            BjtType::Pnp => "PNP",
        };
        format!(".MODEL {} {}", self.name, spice_type)
    }
}

impl BjtModel for DefaultModel {}
