use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{ElementRef, SpiceComponent, SpiceElement};
use crate::units::{Ampere, Degree};
use crate::waveform::Waveform;

/// Independent current source (`I`).
///
/// Positive current flows from n+ through the source to n-.
/// See ngspice manual Chapter 4, §4.1.
#[derive(Debug, Clone)]
pub struct CurrentSource {
    name: String,
    node_plus: Node,
    node_minus: Node,
    /// DC value (A).
    dc_value: Option<Dynamic<Ampere>>,
    /// AC magnitude (A).
    ac_mag: Option<Dynamic<Ampere>>,
    /// AC phase (degrees). Default: 0.
    ac_phase: Option<Dynamic<Degree>>,
    /// Transient waveform.
    waveform: Option<Waveform>,
}

impl CurrentSource {
    pub const SYMBOL: &str = "I";

    /// Creates a new current source with no value set.
    pub fn new(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
    ) -> Self {
        Self {
            name: name.into(),
            node_plus: node_plus.into(),
            node_minus: node_minus.into(),
            dc_value: None,
            ac_mag: None,
            ac_phase: None,
            waveform: None,
        }
    }

    /// Creates a DC current source.
    pub fn dc(
        name: impl Into<String>,
        node_plus: impl Into<Node>,
        node_minus: impl Into<Node>,
        value: impl Into<Dynamic<Ampere>>,
    ) -> Self {
        let mut s = Self::new(name, node_plus, node_minus);
        s.dc_value = Some(value.into());
        s
    }

    /// Sets the DC value.
    pub fn with_dc(&mut self, value: impl Into<Dynamic<Ampere>>) -> &mut Self {
        self.dc_value = Some(value.into());
        self
    }

    /// Sets AC magnitude and optional phase.
    pub fn with_ac(&mut self, mag: impl Into<Dynamic<Ampere>>, phase: Option<Degree>) -> &mut Self {
        self.ac_mag = Some(mag.into());
        if let Some(p) = phase {
            self.ac_phase = Some(p.into());
        }
        self
    }

    /// Sets the transient waveform.
    pub fn with_waveform(&mut self, wf: Waveform) -> &mut Self {
        self.waveform = Some(wf);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn node_plus(&self) -> &Node {
        &self.node_plus
    }
    pub fn node_minus(&self) -> &Node {
        &self.node_minus
    }
    pub fn dc_value(&self) -> Option<&Dynamic<Ampere>> {
        self.dc_value.as_ref()
    }
    pub fn ac_mag(&self) -> Option<&Dynamic<Ampere>> {
        self.ac_mag.as_ref()
    }
    pub fn ac_phase(&self) -> Option<&Dynamic<Degree>> {
        self.ac_phase.as_ref()
    }
    pub fn waveform(&self) -> Option<&Waveform> {
        self.waveform.as_ref()
    }
}

impl Component for CurrentSource {}

impl SpiceElement for CurrentSource {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for CurrentSource {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {}",
            Self::SYMBOL,
            self.name(),
            self.node_plus(),
            self.node_minus()
        );

        if let Some(dc) = &self.dc_value {
            s.push_str(&format!(" DC {}", dc));
        }

        if let Some(wf) = &self.waveform {
            s.push_str(&format!(" {}", wf));
        }

        if let Some(ac) = &self.ac_mag {
            s.push_str(&format!(" AC {}", ac));
            if let Some(phase) = &self.ac_phase {
                s.push_str(&format!(" {}", phase));
            }
        }

        s
    }
}
