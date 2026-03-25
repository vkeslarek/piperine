use crate::units::{Degree, Dimensionless, Hertz, Second};
use std::fmt;

/// Transient waveform types for independent voltage/current sources.
///
/// All waveform types match ngspice manual Chapter 4, §4.1.1–4.1.9.
#[derive(Debug, Clone)]
pub enum Waveform {
    Pulse(Pulse),
    Sin(Sinusoidal),
    Exp(Exponential),
    Pwl(Pwl),
    Sfm(Sfm),
    Am(Am),
    TrNoise(TrNoise),
    TrRandom(TrRandom),
    External,
}

/// Pulse waveform — §4.1.1.
///
/// `PULSE(V1 V2 TD TR TF PW PER NP)`
#[derive(Debug, Clone)]
pub struct Pulse {
    /// V1: Initial value (V or A). Required.
    pub v1: f64,
    /// V2: Pulsed value (V or A). Required.
    pub v2: f64,
    /// TD: Delay time (s). Default: 0.
    pub delay: Option<Second>,
    /// TR: Rise time (s). Default: TSTEP.
    pub rise_time: Option<Second>,
    /// TF: Fall time (s). Default: TSTEP.
    pub fall_time: Option<Second>,
    /// PW: Pulse width (s). Default: TSTOP.
    pub pulse_width: Option<Second>,
    /// PER: Period (s). Default: TSTOP.
    pub period: Option<Second>,
    /// NP: Number of pulses. Default: unlimited (0).
    pub n_pulses: Option<u32>,
}

impl Pulse {
    pub fn new(v1: f64, v2: f64) -> Self {
        Self {
            v1,
            v2,
            delay: None,
            rise_time: None,
            fall_time: None,
            pulse_width: None,
            period: None,
            n_pulses: None,
        }
    }

    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
    pub fn with_rise_time(mut self, v: Second) -> Self {
        self.rise_time = Some(v);
        self
    }
    pub fn with_fall_time(mut self, v: Second) -> Self {
        self.fall_time = Some(v);
        self
    }
    pub fn with_pulse_width(mut self, v: Second) -> Self {
        self.pulse_width = Some(v);
        self
    }
    pub fn with_period(mut self, v: Second) -> Self {
        self.period = Some(v);
        self
    }
    pub fn with_n_pulses(mut self, n: u32) -> Self {
        self.n_pulses = Some(n);
        self
    }
}

/// Sinusoidal waveform — §4.1.2.
///
/// `SIN(VO VA FREQ TD THETA PHASE)`
#[derive(Debug, Clone)]
pub struct Sinusoidal {
    /// VO: Offset (V or A). Required.
    pub offset: f64,
    /// VA: Amplitude (V or A). Required.
    pub amplitude: f64,
    /// FREQ: Frequency (Hz). Default: 1/TSTOP.
    pub frequency: Option<Hertz>,
    /// TD: Delay (s). Default: 0.
    pub delay: Option<Second>,
    /// THETA: Damping factor (1/s). Default: 0.
    pub damping: Option<f64>,
    /// PHASE: Phase (degrees). Default: 0.
    pub phase: Option<Degree>,
}

impl Sinusoidal {
    pub fn new(offset: f64, amplitude: f64) -> Self {
        Self {
            offset,
            amplitude,
            frequency: None,
            delay: None,
            damping: None,
            phase: None,
        }
    }

    pub fn with_frequency(mut self, v: Hertz) -> Self {
        self.frequency = Some(v);
        self
    }
    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
    pub fn with_damping(mut self, v: f64) -> Self {
        self.damping = Some(v);
        self
    }
    pub fn with_phase(mut self, v: Degree) -> Self {
        self.phase = Some(v);
        self
    }
}

/// Exponential waveform — §4.1.3.
///
/// `EXP(V1 V2 TD1 TAU1 TD2 TAU2)`
#[derive(Debug, Clone)]
pub struct Exponential {
    /// V1: Initial value (V or A). Required.
    pub v1: f64,
    /// V2: Pulsed value (V or A). Required.
    pub v2: f64,
    /// TD1: Rise delay time (s). Default: 0.
    pub rise_delay: Option<Second>,
    /// TAU1: Rise time constant (s). Default: TSTEP.
    pub rise_tau: Option<Second>,
    /// TD2: Fall delay time (s). Default: TD1+TSTEP.
    pub fall_delay: Option<Second>,
    /// TAU2: Fall time constant (s). Default: TSTEP.
    pub fall_tau: Option<Second>,
}

impl Exponential {
    pub fn new(v1: f64, v2: f64) -> Self {
        Self {
            v1,
            v2,
            rise_delay: None,
            rise_tau: None,
            fall_delay: None,
            fall_tau: None,
        }
    }

    pub fn with_rise_delay(mut self, v: Second) -> Self {
        self.rise_delay = Some(v);
        self
    }
    pub fn with_rise_tau(mut self, v: Second) -> Self {
        self.rise_tau = Some(v);
        self
    }
    pub fn with_fall_delay(mut self, v: Second) -> Self {
        self.fall_delay = Some(v);
        self
    }
    pub fn with_fall_tau(mut self, v: Second) -> Self {
        self.fall_tau = Some(v);
        self
    }
}

/// Piece-wise linear waveform — §4.1.4.
///
/// `PWL(T1 V1 <T2 V2 ...>) <r=value> <td=value>`
#[derive(Debug, Clone)]
pub struct Pwl {
    /// Time-value pairs: (time_s, value_V_or_A).
    pub points: Vec<(Second, f64)>,
    /// r: Repeat time point. If set to 0, repeats forever from T1.
    pub repeat_start: Option<Second>,
    /// td: Time delay for the entire PWL sequence.
    pub delay: Option<Second>,
}

impl Pwl {
    pub fn new(points: Vec<(Second, f64)>) -> Self {
        Self {
            points,
            repeat_start: None,
            delay: None,
        }
    }

    pub fn with_repeat(mut self, r: Second) -> Self {
        self.repeat_start = Some(r);
        self
    }
    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
}

/// Single-frequency FM waveform — §4.1.5.
///
/// `SFFM(VO VA FM MDI FC TD PHASEM PHASEC)`
#[derive(Debug, Clone)]
pub struct Sfm {
    /// VO: Offset (V or A). Required.
    pub offset: f64,
    /// VA: Amplitude (V or A). Required.
    pub amplitude: f64,
    /// FM: Modulating frequency (Hz). Default: 5/TSTOP.
    pub mod_freq: Option<Hertz>,
    /// MDI: Modulation index. Default: 90.
    pub mod_index: Option<Dimensionless>,
    /// FC: Carrier frequency (Hz). Default: 500/TSTOP.
    pub carrier_freq: Option<Hertz>,
    /// TD: Signal delay (s). Default: 0.
    pub delay: Option<Second>,
    /// PHASEM: Modulation signal phase (degrees). Default: 0.
    pub phase_mod: Option<Degree>,
    /// PHASEC: Carrier signal phase (degrees). Default: 0.
    pub phase_carrier: Option<Degree>,
}

impl Sfm {
    pub fn new(offset: f64, amplitude: f64) -> Self {
        Self {
            offset,
            amplitude,
            mod_freq: None,
            mod_index: None,
            carrier_freq: None,
            delay: None,
            phase_mod: None,
            phase_carrier: None,
        }
    }

    pub fn with_mod_freq(mut self, v: Hertz) -> Self {
        self.mod_freq = Some(v);
        self
    }
    pub fn with_mod_index(mut self, v: Dimensionless) -> Self {
        self.mod_index = Some(v);
        self
    }
    pub fn with_carrier_freq(mut self, v: Hertz) -> Self {
        self.carrier_freq = Some(v);
        self
    }
    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
    pub fn with_phase_mod(mut self, v: Degree) -> Self {
        self.phase_mod = Some(v);
        self
    }
    pub fn with_phase_carrier(mut self, v: Degree) -> Self {
        self.phase_carrier = Some(v);
        self
    }
}

/// Amplitude modulated waveform — §4.1.6.
///
/// `AM(VO VMO VMA FM FC TD PHASEM PHASEC)`
#[derive(Debug, Clone)]
pub struct Am {
    /// VO: Overall offset (V or A). Required.
    pub offset: f64,
    /// VMO: Modulation signal offset (V or A). Required.
    pub mod_offset: f64,
    /// VMA: Modulation signal amplitude (V or A). Default: 1.
    pub mod_amplitude: Option<f64>,
    /// FM: Modulation signal frequency (Hz). Default: 5/TSTOP.
    pub mod_freq: Option<Hertz>,
    /// FC: Carrier signal frequency (Hz). Default: 500/TSTOP.
    pub carrier_freq: Option<Hertz>,
    /// TD: Overall delay (s). Default: 0.
    pub delay: Option<Second>,
    /// PHASEM: Modulation signal phase (degrees). Default: 0.
    pub phase_mod: Option<Degree>,
    /// PHASEC: Carrier signal phase (degrees). Default: 0.
    pub phase_carrier: Option<Degree>,
}

impl Am {
    pub fn new(offset: f64, mod_offset: f64) -> Self {
        Self {
            offset,
            mod_offset,
            mod_amplitude: None,
            mod_freq: None,
            carrier_freq: None,
            delay: None,
            phase_mod: None,
            phase_carrier: None,
        }
    }

    pub fn with_mod_amplitude(mut self, v: f64) -> Self {
        self.mod_amplitude = Some(v);
        self
    }
    pub fn with_mod_freq(mut self, v: Hertz) -> Self {
        self.mod_freq = Some(v);
        self
    }
    pub fn with_carrier_freq(mut self, v: Hertz) -> Self {
        self.carrier_freq = Some(v);
        self
    }
    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
    pub fn with_phase_mod(mut self, v: Degree) -> Self {
        self.phase_mod = Some(v);
        self
    }
    pub fn with_phase_carrier(mut self, v: Degree) -> Self {
        self.phase_carrier = Some(v);
        self
    }
}

/// Transient noise source — §4.1.7.
///
/// `TRNOISE(NA NT NALPHA NAMP RTSAM RTSCAPT RTSEMT)`
#[derive(Debug, Clone)]
pub struct TrNoise {
    /// NA: Gaussian RMS noise amplitude (V or A). Required.
    pub rms_amplitude: f64,
    /// NT: Time step (s). Required.
    pub time_step: Second,
    /// NALPHA: 1/f exponent (0 < α < 2). Optional.
    pub alpha: Option<f64>,
    /// NAMP: 1/f noise amplitude (V or A). Optional.
    pub noise_amplitude: Option<f64>,
    /// RTSAM: Random telegraph signal amplitude (V or A). Optional.
    pub rts_amplitude: Option<f64>,
    /// RTSCAPT: Trap capture time (s). Optional.
    pub rts_capture_time: Option<Second>,
    /// RTSEMT: Trap emission time (s). Optional.
    pub rts_emission_time: Option<Second>,
}

impl TrNoise {
    pub fn new(rms_amplitude: f64, time_step: Second) -> Self {
        Self {
            rms_amplitude,
            time_step,
            alpha: None,
            noise_amplitude: None,
            rts_amplitude: None,
            rts_capture_time: None,
            rts_emission_time: None,
        }
    }

    pub fn with_alpha(mut self, v: f64) -> Self {
        self.alpha = Some(v);
        self
    }
    pub fn with_noise_amplitude(mut self, v: f64) -> Self {
        self.noise_amplitude = Some(v);
        self
    }
    pub fn with_rts(mut self, amplitude: f64, capture: Second, emission: Second) -> Self {
        self.rts_amplitude = Some(amplitude);
        self.rts_capture_time = Some(capture);
        self.rts_emission_time = Some(emission);
        self
    }
}

/// Random distribution type for TrRandom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomDistribution {
    /// Type 1: Uniform distribution. PARAM1=range, PARAM2=offset.
    Uniform = 1,
    /// Type 2: Gaussian distribution. PARAM1=std_dev, PARAM2=mean.
    Gaussian = 2,
    /// Type 3: Exponential distribution. PARAM1=mean, PARAM2=offset.
    Exponential = 3,
    /// Type 4: Poisson distribution. PARAM1=lambda, PARAM2=offset.
    Poisson = 4,
}

/// Random voltage/current source — §4.1.8.
///
/// `TRRANDOM(TYPE TS <TD <PARAM1 PARAM2>>)`
#[derive(Debug, Clone)]
pub struct TrRandom {
    /// Distribution type (1-4).
    pub distribution: RandomDistribution,
    /// TS: Duration of each random value (s). Required.
    pub duration: Second,
    /// TD: Time delay before random values start (s). Optional.
    pub delay: Option<Second>,
    /// PARAM1: Distribution-specific parameter. Default: 1.
    pub param1: Option<f64>,
    /// PARAM2: Distribution-specific parameter. Default: 0.
    pub param2: Option<f64>,
}

impl TrRandom {
    pub fn new(distribution: RandomDistribution, duration: Second) -> Self {
        Self {
            distribution,
            duration,
            delay: None,
            param1: None,
            param2: None,
        }
    }

    pub fn with_delay(mut self, v: Second) -> Self {
        self.delay = Some(v);
        self
    }
    pub fn with_params(mut self, p1: f64, p2: f64) -> Self {
        self.param1 = Some(p1);
        self.param2 = Some(p2);
        self
    }
}

// --- Display / SPICE formatting ---

impl fmt::Display for Waveform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Waveform::Pulse(p) => write!(f, "{}", p),
            Waveform::Sin(s) => write!(f, "{}", s),
            Waveform::Exp(e) => write!(f, "{}", e),
            Waveform::Pwl(p) => write!(f, "{}", p),
            Waveform::Sfm(s) => write!(f, "{}", s),
            Waveform::Am(a) => write!(f, "{}", a),
            Waveform::TrNoise(t) => write!(f, "{}", t),
            Waveform::TrRandom(t) => write!(f, "{}", t),
            Waveform::External => write!(f, "EXTERNAL"),
        }
    }
}

/// Helper: format positional args, stopping at the last Some value.
fn fmt_positional(args: &[Option<String>]) -> String {
    let last_some = args.iter().rposition(|a| a.is_some()).unwrap_or(0);
    args[..=last_some]
        .iter()
        .map(|a| a.as_deref().unwrap_or("0"))
        .collect::<Vec<_>>()
        .join(" ")
}

impl fmt::Display for Pulse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![Some(self.v1.to_string()), Some(self.v2.to_string())];
        let optionals: Vec<Option<String>> = vec![
            self.delay.map(|v| v.to_string()),
            self.rise_time.map(|v| v.to_string()),
            self.fall_time.map(|v| v.to_string()),
            self.pulse_width.map(|v| v.to_string()),
            self.period.map(|v| v.to_string()),
            self.n_pulses.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "PULSE({})", fmt_positional(&args))
    }
}

impl fmt::Display for Sinusoidal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![
            Some(self.offset.to_string()),
            Some(self.amplitude.to_string()),
        ];
        let optionals: Vec<Option<String>> = vec![
            self.frequency.map(|v| v.to_string()),
            self.delay.map(|v| v.to_string()),
            self.damping.map(|v| v.to_string()),
            self.phase.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "SIN({})", fmt_positional(&args))
    }
}

impl fmt::Display for Exponential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![Some(self.v1.to_string()), Some(self.v2.to_string())];
        let optionals: Vec<Option<String>> = vec![
            self.rise_delay.map(|v| v.to_string()),
            self.rise_tau.map(|v| v.to_string()),
            self.fall_delay.map(|v| v.to_string()),
            self.fall_tau.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "EXP({})", fmt_positional(&args))
    }
}

impl fmt::Display for Pwl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pairs: Vec<String> = self
            .points
            .iter()
            .map(|(t, v)| format!("{} {}", t, v))
            .collect();
        let mut s = format!("PWL({})", pairs.join(" "));
        if let Some(r) = self.repeat_start {
            s.push_str(&format!(" r={}", r));
        }
        if let Some(td) = self.delay {
            s.push_str(&format!(" td={}", td));
        }
        write!(f, "{}", s)
    }
}

impl fmt::Display for Sfm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![
            Some(self.offset.to_string()),
            Some(self.amplitude.to_string()),
        ];
        let optionals: Vec<Option<String>> = vec![
            self.mod_freq.map(|v| v.to_string()),
            self.mod_index.map(|v| v.to_string()),
            self.carrier_freq.map(|v| v.to_string()),
            self.delay.map(|v| v.to_string()),
            self.phase_mod.map(|v| v.to_string()),
            self.phase_carrier.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "SFFM({})", fmt_positional(&args))
    }
}

impl fmt::Display for Am {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![
            Some(self.offset.to_string()),
            Some(self.mod_offset.to_string()),
        ];
        let optionals: Vec<Option<String>> = vec![
            self.mod_amplitude.map(|v| v.to_string()),
            self.mod_freq.map(|v| v.to_string()),
            self.carrier_freq.map(|v| v.to_string()),
            self.delay.map(|v| v.to_string()),
            self.phase_mod.map(|v| v.to_string()),
            self.phase_carrier.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "AM({})", fmt_positional(&args))
    }
}

impl fmt::Display for TrNoise {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![
            Some(self.rms_amplitude.to_string()),
            Some(self.time_step.to_string()),
        ];
        let optionals: Vec<Option<String>> = vec![
            self.alpha.map(|v| v.to_string()),
            self.noise_amplitude.map(|v| v.to_string()),
            self.rts_amplitude.map(|v| v.to_string()),
            self.rts_capture_time.map(|v| v.to_string()),
            self.rts_emission_time.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "TRNOISE({})", fmt_positional(&args))
    }
}

impl fmt::Display for TrRandom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut args = vec![
            Some((self.distribution as u8).to_string()),
            Some(self.duration.to_string()),
        ];
        let optionals: Vec<Option<String>> = vec![
            self.delay.map(|v| v.to_string()),
            self.param1.map(|v| v.to_string()),
            self.param2.map(|v| v.to_string()),
        ];
        args.extend(optionals);
        write!(f, "TRRANDOM({})", fmt_positional(&args))
    }
}
