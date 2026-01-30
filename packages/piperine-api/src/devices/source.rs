use crate::circuit::netlist::{BranchIdentifier, NodeIdentifier};
use crate::devices::Component;
use crate::num::Scalar;
use crate::unit::{Ampere, Dimensionless, Hertz, Radian, Second, Volt};

pub struct Ac<T: Scalar> {
    pub amplitude: T,
    pub phase: Radian,
}

pub struct Distortion<T: Scalar> {
    pub magnitude: T,
    pub phase: Radian,
}

pub enum PieceWiseLinearRepeat {
    Once,
    Repeat,
    RepeatFrom(Second),
}

pub struct CarrierSignal {
    pub frequency: Hertz,
    pub phase: Radian,
}

pub struct ModulatedSignal<T> {
    pub offset: T,
    pub amplitude: T,
    pub frequency: Option<Hertz>,
    pub phase: Option<Radian>,
}

pub enum RandomSource<T> {
    Uniform { range: T, offset: T },
    Gaussian { std_dev: T, mean: T },
    Exponential { mean: T, offset: T },
    Poisson { lambda: Dimensionless, offset: T },
}

pub trait SimulationContext {
    fn time(&self) -> Second;
    fn time_step(&self) -> Second;
    fn iteration(&self) -> usize;
    fn node_voltage(&self, node: &NodeIdentifier, use_guess: bool) -> Option<Volt>;
    fn branch_current(&self, branch: &BranchIdentifier, use_guess: bool) -> Option<Ampere>;
}

pub enum Waveform<T> {
    DC(T),
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
    Sine {
        offset: T,
        amplitude: T,
        frequency: Option<Hertz>,
        delay: Option<Second>,
        damping_factor: Option<Hertz>,
        phase: Option<Radian>,
    },
    Exponential {
        initial: T,
        pulsed: T,
        rise_delay: Option<Second>,
        rise_time_const: Option<Second>,
        fall_delay: Option<Second>,
        fall_time_const: Option<Second>,
    },
    PieceWiseLinear {
        values: Vec<(Second, T)>,
        delay: Option<Second>,
        repeat: Option<PieceWiseLinearRepeat>,
    },
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
    AmplitudeModulated {
        offset: T,
        delay: Option<Second>,
        modulated_signal: ModulatedSignal<T>,
        carrier_signal_phase: Option<CarrierSignal>,
    },
    TransientNoise {
        gaussian_amplitude: T,
        time_step: Second,
        alpha_exponent: Option<Dimensionless>,
        f_amplitude: Option<T>,
        rts_amplitude: Option<T>,
        rts_capture_time: Option<Second>,
        rts_emission_time: Option<Second>,
    },
    Random {
        distribution: RandomSource<T>,
        time_step: Second,
        delay: Option<Second>,
    },
    Procedural(Box<dyn FnMut(&dyn SimulationContext) -> T>),
}

impl<T> From<T> for Waveform<T> {
    fn from(val: T) -> Self {
        Waveform::DC(val)
    }
}

pub struct VoltageSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    waveform: Option<Waveform<Volt>>,
    ac: Option<Ac<Volt>>,
    distortion: Option<[Distortion<Volt>; 2]>,
}

impl VoltageSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            waveform: None,
            ac: None,
            distortion: None,
        }
    }

    pub fn new_with_waveform(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        waveform: impl Into<Waveform<Volt>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            waveform: Some(waveform.into()),
            ac: None,
            distortion: None,
        }
    }

    pub fn with_value(&mut self, waveform: impl Into<Waveform<Volt>>) -> &mut Self {
        self.waveform = Some(waveform.into());
        self
    }

    pub fn with_waveform(&mut self, waveform: impl Into<Waveform<Volt>>) -> &mut Self {
        self.waveform = Some(waveform.into());
        self
    }

    pub fn with_procedural<F>(&mut self, func: F) -> &mut Self
    where
        F: FnMut(&dyn SimulationContext) -> Volt + Send + Sync + 'static,
    {
        self.waveform = Some(Waveform::Procedural(Box::new(func)));
        self
    }

    pub fn with_ac(&mut self, amplitude: impl Into<Volt>, phase: impl Into<Radian>) -> &mut Self {
        self.ac = Some(Ac {
            amplitude: amplitude.into(),
            phase: phase.into(),
        });
        self
    }

    pub fn with_distortion(&mut self, dist: [Distortion<Volt>; 2]) -> &mut Self {
        self.distortion = Some(dist);
        self
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn waveform(&self) -> Option<&Waveform<Volt>> {
        self.waveform.as_ref()
    }

    pub fn ac(&self) -> Option<&Ac<Volt>> {
        self.ac.as_ref()
    }

    pub fn distortion(&self) -> Option<&[Distortion<Volt>; 2]> {
        self.distortion.as_ref()
    }
}

impl Component for VoltageSource {
    fn name(&self) -> &String {
        &self.name
    }
}

pub struct CurrentSource {
    name: String,
    node_plus: NodeIdentifier,
    node_minus: NodeIdentifier,
    waveform: Option<Waveform<Ampere>>,
    ac: Option<Ac<Ampere>>,
    distortion: Option<[Distortion<Ampere>; 2]>,
}

impl CurrentSource {
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            waveform: None,
            ac: None,
            distortion: None,
        }
    }

    pub fn new_with_waveform(
        name: impl Into<String>,
        node_plus: impl Into<NodeIdentifier>,
        node_minus: impl Into<NodeIdentifier>,
        waveform: impl Into<Waveform<Ampere>>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            waveform: Some(waveform.into()),
            ac: None,
            distortion: None,
        }
    }

    pub fn with_value(&mut self, waveform: impl Into<Waveform<Ampere>>) -> &mut Self {
        self.waveform = Some(waveform.into());
        self
    }

    pub fn with_waveform(&mut self, waveform: impl Into<Waveform<Ampere>>) -> &mut Self {
        self.waveform = Some(waveform.into());
        self
    }

    pub fn with_procedural<F>(&mut self, func: F) -> &mut Self
    where
        F: FnMut(&dyn SimulationContext) -> Ampere + Send + Sync + 'static,
    {
        self.waveform = Some(Waveform::Procedural(Box::new(func)));
        self
    }

    pub fn with_ac(&mut self, amplitude: impl Into<Ampere>, phase: impl Into<Radian>) -> &mut Self {
        self.ac = Some(Ac {
            amplitude: amplitude.into(),
            phase: phase.into(),
        });
        self
    }

    pub fn with_distortion(&mut self, dist: [Distortion<Ampere>; 2]) -> &mut Self {
        self.distortion = Some(dist);
        self
    }

    pub fn node_plus(&self) -> &NodeIdentifier {
        &self.node_plus
    }

    pub fn node_minus(&self) -> &NodeIdentifier {
        &self.node_minus
    }

    pub fn waveform(&self) -> Option<&Waveform<Ampere>> {
        self.waveform.as_ref()
    }

    pub fn ac(&self) -> Option<&Ac<Ampere>> {
        self.ac.as_ref()
    }

    pub fn distortion(&self) -> Option<&[Distortion<Ampere>; 2]> {
        self.distortion.as_ref()
    }
}

impl Component for CurrentSource {
    fn name(&self) -> &String {
        &self.name
    }
}
