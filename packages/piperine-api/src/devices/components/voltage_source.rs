use crate::circuit::netlist::{IntoNodeIdentifier, NodeIdentifier};
use crate::devices::components::source_waveform::{
    Ac, CarrierSignal, Distortion, ModulatedSignal, PieceWiseLinearRepeat, RandomSource,
    SimulationContext, Waveform,
};
use crate::devices::{Component, Dynamic};
use crate::unit::{Dimensionless, Hertz, Ohm, Radian, Second, Volt, Watt};

/// Two-terminal voltage excitation (`V+`, `V-`) with full `vsrc` parameter coverage.
#[derive(Debug, Clone)]
pub struct VoltageSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    pub params: VoltageSourceParams,
}

/// Parameter block backing the independent voltage source.
#[derive(Debug, Clone)]
pub struct VoltageSourceParams {
    /// Arbitrary waveform definition (`VSRC_DC`, `VSRC_PULSE`, `VSRC_SINE`, ...).
    waveform: Option<Waveform<Volt>>,
    /// Optional AC pair (`VSRC_AC_MAG/VSRC_AC_PHASE`).
    ac: Option<Ac<Volt>>,
    /// Optional distortion tones (`VSRC_D_F1`, `VSRC_D_F2`).
    distortion: Option<[Distortion<Volt>; 2]>,
    /// Optional RF port definition (`VSRC_PORTNUM`, `VSRC_PORTZ0`, ...).
    port: Option<PortDefinition>,
}

/// RF port metadata (used when operating as a ported source).
#[derive(Debug, Clone)]
pub struct PortDefinition {
    pub index: i32,
    pub impedance: Option<Dynamic<Ohm>>,
    pub power: Option<Dynamic<Watt>>,
    pub frequency: Option<Hertz>,
    pub phase: Option<Radian>,
}

impl PortDefinition {
    /// Creates an RF port definition for `VSRC_PORTNUM`.
    pub fn new(index: i32) -> Self {
        Self {
            index,
            impedance: None,
            power: None,
            frequency: None,
            phase: None,
        }
    }

    /// Sets the characteristic impedance (`VSRC_PORTZ0`).
    pub fn with_impedance(mut self, value: impl Into<Dynamic<Ohm>>) -> Self {
        self.impedance = Some(value.into());
        self
    }

    /// Sets the available power (`VSRC_PORTPWR`).
    pub fn with_power(mut self, value: impl Into<Dynamic<Watt>>) -> Self {
        self.power = Some(value.into());
        self
    }

    /// Sets the port frequency (`VSRC_PORTFREQ`).
    pub fn with_frequency(mut self, value: impl Into<Hertz>) -> Self {
        self.frequency = Some(value.into());
        self
    }

    /// Sets the port phase (`VSRC_PORTPHASE`).
    pub fn with_phase(mut self, value: impl Into<Radian>) -> Self {
        self.phase = Some(value.into());
        self
    }
}

impl VoltageSourceParams {
    /// Creates an empty parameter set (no excitation until configured).
    pub fn new() -> Self {
        Self {
            waveform: None,
            ac: None,
            distortion: None,
            port: None,
        }
    }

    /// Returns the configured waveform (`VSRC_*`).
    pub fn waveform(&self) -> Option<&Waveform<Volt>> {
        self.waveform.as_ref()
    }

    /// Returns the optional AC magnitude/phase pair.
    pub fn ac(&self) -> Option<&Ac<Volt>> {
        self.ac.as_ref()
    }

    /// Returns the optional distortion tones.
    pub fn distortion(&self) -> Option<&[Distortion<Volt>; 2]> {
        self.distortion.as_ref()
    }

    /// Returns the optional port definition.
    pub fn port(&self) -> Option<&PortDefinition> {
        self.port.as_ref()
    }
}

impl Default for VoltageSourceParams {
    fn default() -> Self {
        Self::new()
    }
}

impl VoltageSource {
    /// Creates a voltage source bound to `V+`/`V-` with no waveform yet.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl IntoNodeIdentifier,
        node_minus: impl IntoNodeIdentifier,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            params: VoltageSourceParams::new(),
        }
    }

    /// Sets a DC value (`VSRC_DC`).
    pub fn with_dc(&mut self, value: impl Into<Volt>) -> &mut Self {
        self.params.waveform = Some(Waveform::DC(value.into()));
        self
    }

    /// Sets an arbitrary waveform structure (maps to the appropriate `VSRC_*` keyword).
    pub fn with_waveform(&mut self, waveform: impl Into<Waveform<Volt>>) -> &mut Self {
        self.params.waveform = Some(waveform.into());
        self
    }

    /// Configures a `PULSE` waveform (`VSRC_PULSE`).
    #[allow(clippy::too_many_arguments)]
    pub fn with_pulse(
        &mut self,
        initial: Volt,
        pulsed: Volt,
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

    /// Configures a `SIN` waveform (`VSRC_SINE`).
    pub fn with_sine(
        &mut self,
        offset: Volt,
        amplitude: Volt,
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

    /// Configures an `EXP` waveform (`VSRC_EXP`).
    pub fn with_exponential(
        &mut self,
        initial: Volt,
        pulsed: Volt,
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

    /// Configures a `PWL` waveform (`VSRC_PWL`).
    pub fn with_piecewise_linear(
        &mut self,
        values: Vec<(Second, Volt)>,
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

    /// Configures an `SFFM` waveform (`VSRC_SFFM`).
    pub fn with_single_frequency_fm(
        &mut self,
        offset: Volt,
        amplitude: Volt,
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

    /// Configures an `AM` waveform (`VSRC_AM`).
    pub fn with_amplitude_modulation(
        &mut self,
        offset: Volt,
        delay: Option<Second>,
        modulated_signal: ModulatedSignal<Volt>,
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

    /// Configures a transient-noise waveform (`VSRC_TRNOISE`).
    pub fn with_transient_noise(
        &mut self,
        gaussian_amplitude: Volt,
        time_step: Second,
        alpha: Option<Dimensionless>,
        flicker_amplitude: Option<Volt>,
        rts_amplitude: Option<Volt>,
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

    /// Configures a random-process waveform (`VSRC_TRRANDOM`).
    pub fn with_random(
        &mut self,
        distribution: RandomSource<Volt>,
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

    /// Configures an arbitrary procedural source (`VSRC_EXTERNAL`).
    pub fn with_procedural<F>(&mut self, func: F) -> &mut Self
    where
        F: FnMut(&dyn SimulationContext) -> Volt + Send + Sync + 'static,
    {
        self.params.waveform = Some(Waveform::Procedural(Box::new(func)));
        self
    }

    /// Sets the AC pair (`VSRC_AC_MAG`, `VSRC_AC_PHASE`).
    pub fn with_ac(&mut self, amplitude: Volt, phase: Radian) -> &mut Self {
        self.params.ac = Some(Ac { amplitude, phase });
        self
    }

    /// Sets the distortion tones (`VSRC_D_F1`, `VSRC_D_F2`).
    pub fn with_distortion(&mut self, tones: [Distortion<Volt>; 2]) -> &mut Self {
        self.params.distortion = Some(tones);
        self
    }

    /// Defines RF port metadata (`VSRC_PORT*`).
    pub fn with_port(&mut self, port: PortDefinition) -> &mut Self {
        self.params.port = Some(port);
        self
    }

    /// Instance name (e.g. `V1`).
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
    pub fn params(&self) -> &VoltageSourceParams {
        &self.params
    }

    /// Mutable view of the parameter block.
    pub fn params_mut(&mut self) -> &mut VoltageSourceParams {
        &mut self.params
    }
}

impl Component for VoltageSource {
    fn name(&self) -> &str {
        self.name()
    }
}
