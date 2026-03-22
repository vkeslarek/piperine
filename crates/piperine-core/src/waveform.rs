use crate::units::*;

/// Waveform specification for independent voltage/current sources.
#[derive(Debug, Clone)]
pub enum Waveform {
    Dc(f64),
    Pulse(Pulse),
    Sine(Sinusoidal),
    Exp(Exponential),
    Pwl(PieceWiseLinear),
    SFfm(SingleFrequencyFM),
    Am(AmplitudeModulated),
    External,
}

#[derive(Debug, Clone)]
pub struct Pulse {
    pub v1: f64,
    pub v2: f64,
    pub td: Option<Second>,
    pub tr: Option<Second>,
    pub tf: Option<Second>,
    pub pw: Option<Second>,
    pub per: Option<Second>,
    pub np: Option<u32>,
}

impl Pulse {
    pub fn new(v1: f64, v2: f64) -> Self {
        Self { v1, v2, td: None, tr: None, tf: None, pw: None, per: None, np: None }
    }

    pub fn delay(mut self, td: Second) -> Self { self.td = Some(td); self }
    pub fn rise(mut self, tr: Second) -> Self { self.tr = Some(tr); self }
    pub fn fall(mut self, tf: Second) -> Self { self.tf = Some(tf); self }
    pub fn width(mut self, pw: Second) -> Self { self.pw = Some(pw); self }
    pub fn period(mut self, per: Second) -> Self { self.per = Some(per); self }
    pub fn num_pulses(mut self, np: u32) -> Self { self.np = Some(np); self }
}

#[derive(Debug, Clone)]
pub struct Sinusoidal {
    pub offset: f64,
    pub amplitude: f64,
    pub freq: Hertz,
    pub delay: Option<Second>,
    pub damping: Option<f64>,
    pub phase: Option<f64>,
}

impl Sinusoidal {
    pub fn new(offset: f64, amplitude: f64, freq: Hertz) -> Self {
        Self { offset, amplitude, freq, delay: None, damping: None, phase: None }
    }

    pub fn delay(mut self, td: Second) -> Self { self.delay = Some(td); self }
    pub fn damping(mut self, df: f64) -> Self { self.damping = Some(df); self }
    pub fn phase(mut self, phase: f64) -> Self { self.phase = Some(phase); self }
}

#[derive(Debug, Clone)]
pub struct Exponential {
    pub v1: f64,
    pub v2: f64,
    pub td1: Second,
    pub tau1: Second,
    pub td2: Second,
    pub tau2: Second,
}

#[derive(Debug, Clone)]
pub struct PieceWiseLinear {
    pub points: Vec<(Second, f64)>,
}

impl PieceWiseLinear {
    pub fn new(points: Vec<(Second, f64)>) -> Self {
        Self { points }
    }
}

#[derive(Debug, Clone)]
pub struct SingleFrequencyFM {
    pub offset: f64,
    pub amplitude: f64,
    pub carrier_freq: Hertz,
    pub modulation_index: f64,
    pub signal_freq: Hertz,
}

#[derive(Debug, Clone)]
pub struct AmplitudeModulated {
    pub offset: f64,
    pub amplitude: f64,
    pub carrier_freq: Hertz,
    pub modulating_freq: Hertz,
    pub delay: Option<Second>,
}

/// AC specification (magnitude and optional phase).
#[derive(Debug, Clone)]
pub struct AcSpec {
    pub magnitude: f64,
    pub phase: Option<f64>,
}

impl AcSpec {
    pub fn new(magnitude: f64) -> Self {
        Self { magnitude, phase: None }
    }

    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = Some(phase);
        self
    }
}

// --- SPICE formatting ---

impl Waveform {
    pub fn to_spice(&self) -> String {
        match self {
            Waveform::Dc(v) => format!("DC {v}"),
            Waveform::Pulse(p) => {
                let mut s = format!("PULSE({} {}", p.v1, p.v2);
                if let Some(v) = p.td { s.push_str(&format!(" {v}")); }
                if let Some(v) = p.tr { s.push_str(&format!(" {v}")); }
                if let Some(v) = p.tf { s.push_str(&format!(" {v}")); }
                if let Some(v) = p.pw { s.push_str(&format!(" {v}")); }
                if let Some(v) = p.per { s.push_str(&format!(" {v}")); }
                if let Some(v) = p.np { s.push_str(&format!(" {v}")); }
                s.push(')');
                s
            }
            Waveform::Sine(si) => {
                let mut s = format!("SIN({} {} {}", si.offset, si.amplitude, si.freq);
                if let Some(v) = si.delay { s.push_str(&format!(" {v}")); }
                if let Some(v) = si.damping { s.push_str(&format!(" {v}")); }
                if let Some(v) = si.phase { s.push_str(&format!(" {v}")); }
                s.push(')');
                s
            }
            Waveform::Exp(e) => {
                format!("EXP({} {} {} {} {} {})", e.v1, e.v2, e.td1, e.tau1, e.td2, e.tau2)
            }
            Waveform::Pwl(p) => {
                let pts: Vec<String> = p.points.iter()
                    .map(|(t, v)| format!("{t} {v}"))
                    .collect();
                format!("PWL({})", pts.join(" "))
            }
            Waveform::SFfm(f) => {
                format!("SFFM({} {} {} {} {})",
                    f.offset, f.amplitude, f.carrier_freq, f.modulation_index, f.signal_freq)
            }
            Waveform::Am(a) => {
                let mut s = format!("AM({} {} {} {}",
                    a.offset, a.amplitude, a.carrier_freq, a.modulating_freq);
                if let Some(v) = a.delay { s.push_str(&format!(" {v}")); }
                s.push(')');
                s
            }
            Waveform::External => "DC 0 EXTERNAL".to_string(),
        }
    }
}
