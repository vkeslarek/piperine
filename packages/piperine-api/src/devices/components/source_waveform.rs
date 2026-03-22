use crate::circuit::netlist::{BranchIdentifier, NodeIdentifier};
use crate::unit::{Dimensionless, Hertz, Radian, Second};

/// AC magnitude/phase pair (used by `VSRC_AC_MAG/PHASE` or `ISRC_AC_MAG/PHASE`).
#[derive(Debug, Clone, Copy)]
pub struct Ac<T> {
    pub amplitude: T,
    pub phase: Radian,
}

/// Distortion tone (used by `*_D_F1` / `*_D_F2`).
#[derive(Debug, Clone, Copy)]
pub struct Distortion<T> {
    pub magnitude: T,
    pub phase: Radian,
}

/// Repeat semantics for piecewise-linear sources (`*_PWL`).
#[derive(Debug, Clone, Copy)]
pub enum PieceWiseLinearRepeat {
    Once,
    Repeat,
    RepeatFrom(Second),
}

/// Carrier tone metadata for amplitude modulation (`*_AM`).
#[derive(Debug, Clone, Copy)]
pub struct CarrierSignal {
    pub frequency: Hertz,
    pub phase: Radian,
}

/// Envelope used by AM definitions.
#[derive(Debug, Clone, Copy)]
pub struct ModulatedSignal<T> {
    pub offset: T,
    pub amplitude: T,
    pub frequency: Option<Hertz>,
    pub phase: Option<Radian>,
}

/// Random distribution definitions for `*_TRRANDOM`.
#[derive(Debug, Clone, Copy)]
pub enum RandomSource<T> {
    Uniform { range: T, offset: T },
    Gaussian { std_dev: T, mean: T },
    Exponential { mean: T, offset: T },
    Poisson { lambda: Dimensionless, offset: T },
}

/// Runtime context exposed to procedural sources (`*_EXTERNAL`).
pub trait SimulationContext {
    fn time(&self) -> Second;
    fn time_step(&self) -> Second;
    fn iteration(&self) -> usize;
    fn node_voltage(&self, node: &NodeIdentifier, use_guess: bool) -> Option<f64>;
    fn branch_current(&self, branch: &BranchIdentifier, use_guess: bool) -> Option<f64>;
}

/// Waveform definitions covering the standard independent-source keywords.
#[derive(Clone)]
pub enum Waveform<T> {
    /// Constant/DC value (`*_DC`).
    DC(T),
    /// `PULSE` description (`*_PULSE`).
    Pulse {
        initial: T,
        pulsed: T,
        delay: Option<Second>,
        rise: Option<Second>,
        fall: Option<Second>,
        pulse_width: Option<Second>,
        period: Option<Second>,
        number_of_pulses: Option<usize>,
        phase: Option<Radian>,
    },
    /// `SIN` description (`*_SINE`).
    Sine {
        offset: T,
        amplitude: T,
        frequency: Option<Hertz>,
        delay: Option<Second>,
        damping_factor: Option<Hertz>,
        phase: Option<Radian>,
    },
    /// `EXP` description (`*_EXP`).
    Exponential {
        initial: T,
        pulsed: T,
        rise_delay: Option<Second>,
        rise_time_const: Option<Second>,
        fall_delay: Option<Second>,
        fall_time_const: Option<Second>,
    },
    /// Piece-wise linear specification (`*_PWL`).
    PieceWiseLinear {
        values: Vec<(Second, T)>,
        delay: Option<Second>,
        repeat: Option<PieceWiseLinearRepeat>,
    },
    /// Single-frequency FM specification (`*_SFFM`).
    SingleFrequencyFM {
        offset: T,
        amplitude: T,
        carrier_freq: Hertz,
        modulation_index: Dimensionless,
        signal_freq: Hertz,
        carrier_phase: Option<Radian>,
        signal_phase: Option<Radian>,
        delay: Option<Second>,
    },
    /// Amplitude modulation specification (`*_AM`).
    AmplitudeModulated {
        offset: T,
        delay: Option<Second>,
        modulated_signal: ModulatedSignal<T>,
        carrier_signal: Option<CarrierSignal>,
    },
    /// Transient noise specification (`*_TRNOISE`).
    TransientNoise {
        gaussian_amplitude: T,
        time_step: Second,
        alpha_exponent: Option<Dimensionless>,
        flicker_amplitude: Option<T>,
        rts_amplitude: Option<T>,
        rts_capture_time: Option<Second>,
        rts_emission_time: Option<Second>,
    },
    /// Random-process specification (`*_TRRANDOM`).
    Random {
        distribution: RandomSource<T>,
        time_step: Second,
        delay: Option<Second>,
    },
    /// Arbitrary procedural source (`*_EXTERNAL`).
    Procedural(Box<dyn FnMut(&dyn SimulationContext) -> T + Send + Sync>),
}

impl<T> From<T> for Waveform<T> {
    fn from(value: T) -> Self {
        Waveform::DC(value)
    }
}
