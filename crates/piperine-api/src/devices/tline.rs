use crate::devices::Component;
use crate::node::Node;
use crate::num::Dynamic;
use crate::spice::{SpiceElement, ElementRef, SpiceComponent};
use crate::units::{Dimensionless, Hertz, Ohm, Second};

/// Lossless transmission line (`T`).
///
/// `TXXXX n1+ n1- n2+ n2- Z0=val <TD=val|F=val <NL=val>> <IC=V1,I1,V2,I2>`
/// See ngspice manual §3.3.17.
#[derive(Debug, Clone)]
pub struct TransmissionLine {
    name: String,
    port1_plus: Node,
    port1_minus: Node,
    port2_plus: Node,
    port2_minus: Node,
    /// Z0: Characteristic impedance (Ω). Required.
    z0: Dynamic<Ohm>,
    /// TD: Propagation delay (s). Either TD or F must be specified.
    td: Option<Dynamic<Second>>,
    /// F: Frequency for NL specification (Hz).
    frequency: Option<Dynamic<Hertz>>,
    /// NL: Normalized electrical length at frequency F. Default: 0.25.
    nl: Option<Dynamic<Dimensionless>>,
}

impl TransmissionLine {
    pub const SYMBOL: &str = "T";

    pub fn new(
        name: impl Into<String>,
        port1_plus: impl Into<Node>,
        port1_minus: impl Into<Node>,
        port2_plus: impl Into<Node>,
        port2_minus: impl Into<Node>,
        z0: impl Into<Dynamic<Ohm>>,
    ) -> Self {
        Self {
            name: name.into(),
            port1_plus: port1_plus.into(),
            port1_minus: port1_minus.into(),
            port2_plus: port2_plus.into(),
            port2_minus: port2_minus.into(),
            z0: z0.into(),
            td: None, frequency: None, nl: None,
        }
    }

    pub fn with_td(&mut self, v: impl Into<Dynamic<Second>>) -> &mut Self { self.td = Some(v.into()); self }
    pub fn with_frequency(&mut self, f: impl Into<Dynamic<Hertz>>, nl: Option<Dimensionless>) -> &mut Self {
        self.frequency = Some(f.into());
        if let Some(n) = nl {
            self.nl = Some(n.into());
        }
        self
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn z0(&self) -> &Dynamic<Ohm> { &self.z0 }
}

impl Component for TransmissionLine {}

impl SpiceElement for TransmissionLine {
    fn element_name(&self) -> &str {
        &self.name
    }

    fn element_ref(&self) -> ElementRef {
        ElementRef::new(Self::SYMBOL, &self.name)
    }
}

impl SpiceComponent for TransmissionLine {
    fn into_spice(&self) -> String {
        let mut s = format!(
            "{}{} {} {} {} {} Z0={}",
            Self::SYMBOL, self.name(),
            self.port1_plus, self.port1_minus,
            self.port2_plus, self.port2_minus,
            self.z0
        );
        if let Some(td) = &self.td {
            s.push_str(&format!(" TD={}", td));
        }
        if let Some(f) = &self.frequency {
            s.push_str(&format!(" F={}", f));
            if let Some(nl) = &self.nl {
                s.push_str(&format!(" NL={}", nl));
            }
        }
        s
    }
}
