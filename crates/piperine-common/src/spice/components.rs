use std::fmt::Write;

use super::spice_line::SpiceLine;
use super::spice_node::SpiceNode;

#[derive(Debug, Clone)]
pub struct Resistor {
    pub name: String,
    pub pos: SpiceNode,
    pub neg: SpiceNode,
    pub resistance: f64,
    pub temp: Option<f64>,
    pub tc1: Option<f64>,
    pub tc2: Option<f64>,
}

impl Resistor {
    pub fn new(name: impl Into<String>, pos: impl Into<SpiceNode>, neg: impl Into<SpiceNode>, resistance: f64) -> Self {
        Resistor {
            name: name.into(),
            pos: pos.into(),
            neg: neg.into(),
            resistance,
            temp: None,
            tc1: None,
            tc2: None,
        }
    }
}

impl SpiceLine for Resistor {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {}", self.name, self.pos, self.neg, self.resistance);
        if let Some(t) = self.temp { write!(&mut s, " temp={t}").unwrap(); }
        if let Some(t) = self.tc1 { write!(&mut s, " tc1={t}").unwrap(); }
        if let Some(t) = self.tc2 { write!(&mut s, " tc2={t}").unwrap(); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Capacitor {
    pub name: String,
    pub pos: SpiceNode,
    pub neg: SpiceNode,
    pub capacitance: f64,
    pub ic: Option<f64>,
}

impl Capacitor {
    pub fn new(name: impl Into<String>, pos: impl Into<SpiceNode>, neg: impl Into<SpiceNode>, capacitance: f64) -> Self {
        Capacitor { name: name.into(), pos: pos.into(), neg: neg.into(), capacitance, ic: None }
    }
}

impl SpiceLine for Capacitor {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {}", self.name, self.pos, self.neg, self.capacitance);
        if let Some(v) = self.ic { write!(&mut s, " ic={v}").unwrap(); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Inductor {
    pub name: String,
    pub pos: SpiceNode,
    pub neg: SpiceNode,
    pub inductance: f64,
    pub ic: Option<f64>,
}

impl Inductor {
    pub fn new(name: impl Into<String>, pos: impl Into<SpiceNode>, neg: impl Into<SpiceNode>, inductance: f64) -> Self {
        Inductor { name: name.into(), pos: pos.into(), neg: neg.into(), inductance, ic: None }
    }
}

impl SpiceLine for Inductor {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {}", self.name, self.pos, self.neg, self.inductance);
        if let Some(v) = self.ic { write!(&mut s, " ic={v}").unwrap(); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct VoltageDc {
    pub name: String,
    pub pos: SpiceNode,
    pub neg: SpiceNode,
    pub dc: f64,
}

impl VoltageDc {
    pub fn new(name: impl Into<String>, pos: impl Into<SpiceNode>, neg: impl Into<SpiceNode>, dc: f64) -> Self {
        VoltageDc { name: name.into(), pos: pos.into(), neg: neg.into(), dc }
    }
}

impl SpiceLine for VoltageDc {
    fn spice_line(&self) -> String {
        format!("{} {} {} DC {}", self.name, self.pos, self.neg, self.dc)
    }
}

#[derive(Debug, Clone)]
pub struct CurrentDc {
    pub name: String,
    pub pos: SpiceNode,
    pub neg: SpiceNode,
    pub dc: f64,
}

impl CurrentDc {
    pub fn new(name: impl Into<String>, pos: impl Into<SpiceNode>, neg: impl Into<SpiceNode>, dc: f64) -> Self {
        CurrentDc { name: name.into(), pos: pos.into(), neg: neg.into(), dc }
    }
}

impl SpiceLine for CurrentDc {
    fn spice_line(&self) -> String {
        format!("{} {} {} DC {}", self.name, self.pos, self.neg, self.dc)
    }
}

#[derive(Debug, Clone)]
pub struct Diode {
    pub name: String,
    pub anode: SpiceNode,
    pub cathode: SpiceNode,
    pub model: String,
    pub area: Option<f64>,
}

impl Diode {
    pub fn new(name: impl Into<String>, anode: impl Into<SpiceNode>, cathode: impl Into<SpiceNode>, model: impl Into<String>) -> Self {
        Diode { name: name.into(), anode: anode.into(), cathode: cathode.into(), model: model.into(), area: None }
    }
}

impl SpiceLine for Diode {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {}", self.name, self.anode, self.cathode, self.model);
        if let Some(a) = self.area { write!(&mut s, " {a}").unwrap(); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Bjt {
    pub name: String,
    pub collector: SpiceNode,
    pub base: SpiceNode,
    pub emitter: SpiceNode,
    pub substrate: SpiceNode,
    pub model: String,
    pub area: Option<f64>,
}

impl Bjt {
    pub fn new(
        name: impl Into<String>,
        collector: impl Into<SpiceNode>,
        base: impl Into<SpiceNode>,
        emitter: impl Into<SpiceNode>,
        model: impl Into<String>,
    ) -> Self {
        Bjt {
            name: name.into(),
            collector: collector.into(),
            base: base.into(),
            emitter: emitter.into(),
            substrate: super::node::Node::Ground.into(),
            model: model.into(),
            area: None,
        }
    }
}

impl SpiceLine for Bjt {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {} {} {}",
            self.name, self.collector, self.base, self.emitter, self.substrate, self.model);
        if let Some(a) = self.area { write!(&mut s, " {a}").unwrap(); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Mosfet {
    pub name: String,
    pub drain: SpiceNode,
    pub gate: SpiceNode,
    pub source: SpiceNode,
    pub bulk: SpiceNode,
    pub model: String,
    pub w: Option<f64>,
    pub l: Option<f64>,
}

impl Mosfet {
    pub fn new(
        name: impl Into<String>,
        drain: impl Into<SpiceNode>,
        gate: impl Into<SpiceNode>,
        source: impl Into<SpiceNode>,
        bulk: impl Into<SpiceNode>,
        model: impl Into<String>,
    ) -> Self {
        Mosfet {
            name: name.into(),
            drain: drain.into(),
            gate: gate.into(),
            source: source.into(),
            bulk: bulk.into(),
            model: model.into(),
            w: None,
            l: None,
        }
    }
}

impl SpiceLine for Mosfet {
    fn spice_line(&self) -> String {
        let mut s = format!("{} {} {} {} {} {}", self.name, self.drain, self.gate, self.source, self.bulk, self.model);
        if let Some(w) = self.w { write!(&mut s, " w={w}").unwrap(); }
        if let Some(l) = self.l { write!(&mut s, " l={l}").unwrap(); }
        s
    }
}
