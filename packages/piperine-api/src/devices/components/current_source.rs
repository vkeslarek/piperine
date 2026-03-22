use crate::circuit::netlist::{IntoNodeIdentifier, NodeIdentifier};
use crate::devices::components::source_waveform::{
    Ac, CarrierSignal, Distortion, ModulatedSignal, PieceWiseLinearRepeat, RandomSource,
    SimulationContext, Waveform,
};
use crate::devices::{Component, Dynamic};
use crate::unit::{Ampere, Dimensionless, Hertz, Radian, Second};

/// Two-terminal current excitation (`I+`, `I-`) mirroring the `isrc` keyword set.
#[derive(Debug, Clone)]
pub struct CurrentSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    pub params: CurrentSourceParams,
}

/// Parameter block for the independent current source.
#[derive(Debug, Clone)]
pub struct CurrentSourceParams {
    /// Arbitrary waveform definition (`ISRC_DC`, `ISRC_PULSE`, ...).
    waveform: Option<Waveform<Ampere>>,
    /// Optional AC pair (`ISRC_AC_MAG/ISRC_AC_PHASE`).
    ac: Option<Ac<Ampere>>,
    /// Optional distortion tones (`ISRC_D_F1`, `ISRC_D_F2`).
    distortion: Option<[Distortion<Ampere>; 2]>,
    /// Parallel multiplier (`ISRC_M`). Defaults to 1.
    multiplier: Dynamic<Dimensionless>,
}

impl CurrentSourceParams {
    pub const DEFAULT_MULTIPLIER: Dimensionless = 1.0;

    /// Creates an empty parameter block.
    pub fn new() -> Self {
        Self {
            waveform: None,
            ac: None,
            distortion: None,
            multiplier: Self::DEFAULT_MULTIPLIER.into(),
        }
    }

    /// Returns the configured waveform (`ISRC_*`).
    pub fn waveform(&self) -> Option<&Waveform<Ampere>> {
        self.waveform.as_ref()
    }

    /// Returns the optional AC magnitude/phase pair.
    pub fn ac(&self) -> Option<&Ac<Ampere>> {
        self.ac.as_ref()
    }

    /// Returns the optional distortion tones.
    pub fn distortion(&self) -> Option<&[Distortion<Ampere>; 2]> {
        self.distortion.as_ref()
    }

    /// Returns the multiplier (`ISRC_M`).
    pub fn multiplier(&self) -> &Dynamic<Dimensionless> {
        &self.multiplier
    }
}

impl Default for CurrentSourceParams {
    fn default() -> Self {
        Self::new()
    }
}

impl CurrentSource {
    /// Creates a current source bound to `I+`/`I-` with no waveform yet.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            params: CurrentSourceParams::new(),
        }
    }

    /// Sets a DC value (`ISRC_DC`).
    pub fn with_dc(&mut self, value: impl Into<Ampere>) -> &mut Self {
        self.params.waveform = Some(Waveform::DC(value.into()));
        self
    }

    /// Sets an arbitrary waveform (`ISRC_*`).
    pub fn with_waveform(&mut self, waveform: impl Into<Waveform<Ampere>>) -> &mut Self {
        self.params.waveform = Some(waveform.into());
        self
    }

    /// Configures a `PULSE` waveform (`ISRC_PULSE`).
    #[allow(clippy::too_many_arguments)]
    pub fn with_pulse(
        &mut self,
        initial: Ampere,
        pulsed: Ampere,
        delay: Option<Second>,
        rise: Option<Second>,
        fall: Option<Second>,
        width: Option<Second>,
        period: Option<Second>,
        number_of_pulses: Option<usize>,
        phase: Option<Radian>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::Pulse {
            initial,
            pulsed,
            delay,
            rise,
            fall,
            pulse_width: width,
            period,
            number_of_pulses,
            phase,
        });
        self
    }

    /// Configures a `SIN` waveform (`ISRC_SINE`).
    pub fn with_sine(
        &mut self,
        offset: Ampere,
        amplitude: Ampere,
        frequency: Option<Hertz>,
        delay: Option<Second>,
        damping_factor: Option<Hertz>,
        phase: Option<Radian>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::Sine {
            offset,
            amplitude,
            frequency,
            delay,
            damping_factor,
            phase,
        });
        self
    }

    /// Configures an `EXP` waveform (`ISRC_EXP`).
    pub fn with_exponential(
        &mut self,
        initial: Ampere,
        pulsed: Ampere,
        rise_delay: Option<Second>,
        rise_tau: Option<Second>,
        fall_delay: Option<Second>,
        fall_tau: Option<Second>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::Exponential {
            initial,
            pulsed,
            rise_delay,
            rise_time_const: rise_tau,
            fall_delay,
            fall_time_const: fall_tau,
        });
        self
    }

    /// Configures a `PWL` waveform (`ISRC_PWL`).
    pub fn with_piecewise_linear(
        &mut self,
        values: Vec<(Second, Ampere)>,
        delay: Option<Second>,
        repeat: Option<PieceWiseLinearRepeat>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::PieceWiseLinear {
            values,
            delay,
            repeat,
        });
        self
    }

    /// Configures an `SFFM` waveform (`ISRC_SFFM`).
    pub fn with_single_frequency_fm(
        &mut self,
        offset: Ampere,
        amplitude: Ampere,
        carrier_freq: Hertz,
        modulation_index: Dimensionless,
        signal_freq: Hertz,
        carrier_phase: Option<Radian>,
        signal_phase: Option<Radian>,
        delay: Option<Second>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::SingleFrequencyFM {
            offset,
            amplitude,
            carrier_freq,
            modulation_index,
            signal_freq,
            carrier_phase,
            signal_phase,
            delay,
        });
        self
    }

    /// Configures an `AM` waveform (`ISRC_AM`).
    pub fn with_amplitude_modulation(
        &mut self,
        offset: Ampere,
        delay: Option<Second>,
        modulated_signal: ModulatedSignal<Ampere>,
        carrier_signal: Option<CarrierSignal>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::AmplitudeModulated {
            offset,
            delay,
            modulated_signal,
            carrier_signal,
        });
        self
    }

    /// Configures a transient-noise waveform (`ISRC_TRNOISE`).
    pub fn with_transient_noise(
        &mut self,
        gaussian_amplitude: Ampere,
        time_step: Second,
        alpha: Option<Dimensionless>,
        flicker_amplitude: Option<Ampere>,
        rts_amplitude: Option<Ampere>,
        rts_capture_time: Option<Second>,
        rts_emission_time: Option<Second>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::TransientNoise {
            gaussian_amplitude,
            time_step,
            alpha_exponent: alpha,
            flicker_amplitude,
            rts_amplitude,
            rts_capture_time,
            rts_emission_time,
        });
        self
    }

    /// Configures a random-process waveform (`ISRC_TRRANDOM`).
    pub fn with_random(
        &mut self,
        distribution: RandomSource<Ampere>,
        time_step: Second,
        delay: Option<Second>,
    ) -> &mut Self {
        self.params.waveform = Some(Waveform::Random {
            distribution,
            time_step,
            delay,
        });
        self
    }

    /// Configures an arbitrary procedural source (`ISRC_EXTERNAL`).
    pub fn with_procedural<F>(&mut self, func: F) -> &mut Self
    where
        F: FnMut(&dyn SimulationContext) -> Ampere + Send + Sync + 'static,
    {
        self.params.waveform = Some(Waveform::Procedural(Box::new(func)));
        self
    }

    /// Sets the AC pair (`ISRC_AC_MAG`, `ISRC_AC_PHASE`).
    pub fn with_ac(&mut self, amplitude: Ampere, phase: Radian) -> &mut Self {
        self.params.ac = Some(Ac { amplitude, phase });
        self
    }

    /// Sets the distortion tones (`ISRC_D_F1`, `ISRC_D_F2`).
    pub fn with_distortion(&mut self, tones: [Distortion<Ampere>; 2]) -> &mut Self {
        self.params.distortion = Some(tones);
        self
    }

    /// Sets the multiplier (`ISRC_M`).
    pub fn with_multiplier(&mut self, multiplier: impl Into<Dynamic<Dimensionless>>) -> &mut Self {
        self.params.multiplier = multiplier.into();
        self
    }

    /// Instance name (e.g. `I1`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Reference to the positive terminal.
    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    /// Reference to the negative terminal.
    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    /// Returns both terminal identifiers.
    pub fn nodes(&self) -> (&NodeIdentifier, &NodeIdentifier) {
        (&self.node_plus, &self.node_minus)
    }

    /// Immutable view of the parameter block.
    pub fn params(&self) -> &CurrentSourceParams {
        &self.params
    }

    /// Mutable view of the parameter block.
    pub fn params_mut(&mut self) -> &mut CurrentSourceParams {
        &mut self.params
    }

    /// Returns the multiplier literal/expression.
    pub fn multiplier(&self) -> Dynamic<Dimensionless> {
        self.params.multiplier.clone()
    }
}

impl Component for CurrentSource {
    fn name(&self) -> &str {
        self.name()
    }
}
