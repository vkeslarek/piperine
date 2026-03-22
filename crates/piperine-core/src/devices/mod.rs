use crate::node::Node;
use crate::waveform::{AcSpec, Waveform};

/// Trait for circuit components that can produce a SPICE netlist line.
pub trait Component: std::fmt::Debug {
    /// The SPICE netlist line for this component (e.g. "R1 in out 1k").
    fn to_spice_line(&self) -> String;
}

// === Passive Elements ===

#[derive(Debug, Clone)]
pub struct Resistor {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub value: String,
    pub model: Option<String>,
}

impl Component for Resistor {
    fn to_spice_line(&self) -> String {
        if let Some(ref m) = self.model {
            format!("R{} {} {} {} {m}", self.name, self.p, self.n, self.value)
        } else {
            format!("R{} {} {} {}", self.name, self.p, self.n, self.value)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capacitor {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub value: String,
    pub ic: Option<f64>,
    pub model: Option<String>,
}

impl Component for Capacitor {
    fn to_spice_line(&self) -> String {
        let mut s = if let Some(ref m) = self.model {
            format!("C{} {} {} {} {m}", self.name, self.p, self.n, self.value)
        } else {
            format!("C{} {} {} {}", self.name, self.p, self.n, self.value)
        };
        if let Some(ic) = self.ic {
            s.push_str(&format!(" IC={ic}"));
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Inductor {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub value: String,
    pub ic: Option<f64>,
    pub model: Option<String>,
}

impl Component for Inductor {
    fn to_spice_line(&self) -> String {
        let mut s = if let Some(ref m) = self.model {
            format!("L{} {} {} {} {m}", self.name, self.p, self.n, self.value)
        } else {
            format!("L{} {} {} {}", self.name, self.p, self.n, self.value)
        };
        if let Some(ic) = self.ic {
            s.push_str(&format!(" IC={ic}"));
        }
        s
    }
}

// === Independent Sources ===

#[derive(Debug, Clone)]
pub struct VoltageSource {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub waveform: Option<Waveform>,
    pub ac: Option<AcSpec>,
}

impl Component for VoltageSource {
    fn to_spice_line(&self) -> String {
        let mut s = format!("V{} {} {}", self.name, self.p, self.n);
        if let Some(ref w) = self.waveform {
            s.push_str(&format!(" {}", w.to_spice()));
        }
        if let Some(ref ac) = self.ac {
            s.push_str(&format!(" AC {}", ac.magnitude));
            if let Some(phase) = ac.phase {
                s.push_str(&format!(" {phase}"));
            }
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct CurrentSource {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub waveform: Option<Waveform>,
    pub ac: Option<AcSpec>,
}

impl Component for CurrentSource {
    fn to_spice_line(&self) -> String {
        let mut s = format!("I{} {} {}", self.name, self.p, self.n);
        if let Some(ref w) = self.waveform {
            s.push_str(&format!(" {}", w.to_spice()));
        }
        if let Some(ref ac) = self.ac {
            s.push_str(&format!(" AC {}", ac.magnitude));
            if let Some(phase) = ac.phase {
                s.push_str(&format!(" {phase}"));
            }
        }
        s
    }
}

// === Dependent Sources ===

#[derive(Debug, Clone)]
pub struct Vcvs {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub cp: Node,
    pub cn: Node,
    pub gain: f64,
}

impl Component for Vcvs {
    fn to_spice_line(&self) -> String {
        format!("E{} {} {} {} {} {}", self.name, self.p, self.n, self.cp, self.cn, self.gain)
    }
}

#[derive(Debug, Clone)]
pub struct Vccs {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub cp: Node,
    pub cn: Node,
    pub gm: f64,
}

impl Component for Vccs {
    fn to_spice_line(&self) -> String {
        format!("G{} {} {} {} {} {}", self.name, self.p, self.n, self.cp, self.cn, self.gm)
    }
}

#[derive(Debug, Clone)]
pub struct Ccvs {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub vsource: String,
    pub transresistance: f64,
}

impl Component for Ccvs {
    fn to_spice_line(&self) -> String {
        format!("H{} {} {} V{} {}", self.name, self.p, self.n, self.vsource, self.transresistance)
    }
}

#[derive(Debug, Clone)]
pub struct Cccs {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub vsource: String,
    pub gain: f64,
}

impl Component for Cccs {
    fn to_spice_line(&self) -> String {
        format!("F{} {} {} V{} {}", self.name, self.p, self.n, self.vsource, self.gain)
    }
}

// === Behavioral Sources ===

#[derive(Debug, Clone)]
pub struct BehavioralSource {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub kind: BehavioralKind,
    pub expr: String,
}

#[derive(Debug, Clone, Copy)]
pub enum BehavioralKind {
    Voltage,
    Current,
}

impl Component for BehavioralSource {
    fn to_spice_line(&self) -> String {
        match self.kind {
            BehavioralKind::Voltage =>
                format!("B{} {} {} V={}", self.name, self.p, self.n, self.expr),
            BehavioralKind::Current =>
                format!("B{} {} {} I={}", self.name, self.p, self.n, self.expr),
        }
    }
}

// === Semiconductor Devices ===

#[derive(Debug, Clone)]
pub struct Diode {
    pub name: String,
    pub anode: Node,
    pub cathode: Node,
    pub model: String,
    pub area: Option<f64>,
}

impl Component for Diode {
    fn to_spice_line(&self) -> String {
        let mut s = format!("D{} {} {} {}", self.name, self.anode, self.cathode, self.model);
        if let Some(a) = self.area { s.push_str(&format!(" {a}")); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Bjt {
    pub name: String,
    pub c: Node,
    pub b: Node,
    pub e: Node,
    pub model: String,
    pub area: Option<f64>,
}

impl Component for Bjt {
    fn to_spice_line(&self) -> String {
        let mut s = format!("Q{} {} {} {} {}", self.name, self.c, self.b, self.e, self.model);
        if let Some(a) = self.area { s.push_str(&format!(" {a}")); }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Mosfet {
    pub name: String,
    pub d: Node,
    pub g: Node,
    pub s: Node,
    pub b: Node,
    pub model: String,
    pub params: Vec<(String, String)>,
}

impl Component for Mosfet {
    fn to_spice_line(&self) -> String {
        let mut s = format!("M{} {} {} {} {} {}",
            self.name, self.d, self.g, self.s, self.b, self.model);
        for (k, v) in &self.params {
            s.push_str(&format!(" {k}={v}"));
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Jfet {
    pub name: String,
    pub d: Node,
    pub g: Node,
    pub s: Node,
    pub model: String,
}

impl Component for Jfet {
    fn to_spice_line(&self) -> String {
        format!("J{} {} {} {} {}", self.name, self.d, self.g, self.s, self.model)
    }
}

// === Switches ===

#[derive(Debug, Clone)]
pub struct VSwitch {
    pub name: String,
    pub p: Node,
    pub n: Node,
    pub cp: Node,
    pub cn: Node,
    pub model: String,
}

impl Component for VSwitch {
    fn to_spice_line(&self) -> String {
        format!("S{} {} {} {} {} {}", self.name, self.p, self.n, self.cp, self.cn, self.model)
    }
}

// === Transmission Lines ===

#[derive(Debug, Clone)]
pub struct TransmissionLine {
    pub name: String,
    pub p1: Node,
    pub n1: Node,
    pub p2: Node,
    pub n2: Node,
    pub z0: f64,
    pub td: f64,
}

impl Component for TransmissionLine {
    fn to_spice_line(&self) -> String {
        format!("T{} {} {} {} {} Z0={} TD={}",
            self.name, self.p1, self.n1, self.p2, self.n2, self.z0, self.td)
    }
}

// === Coupled Inductors ===

#[derive(Debug, Clone)]
pub struct MutualInductor {
    pub name: String,
    pub l1: String,
    pub l2: String,
    pub coupling: f64,
}

impl Component for MutualInductor {
    fn to_spice_line(&self) -> String {
        format!("K{} L{} L{} {}", self.name, self.l1, self.l2, self.coupling)
    }
}

// === Subcircuit Instance ===

#[derive(Debug, Clone)]
pub struct SubCircuitInstance {
    pub name: String,
    pub subckt: String,
    pub nodes: Vec<Node>,
    pub params: Vec<(String, String)>,
}

impl Component for SubCircuitInstance {
    fn to_spice_line(&self) -> String {
        let nodes: Vec<String> = self.nodes.iter().map(|n| n.to_string()).collect();
        let mut s = format!("X{} {} {}", self.name, nodes.join(" "), self.subckt);
        for (k, v) in &self.params {
            s.push_str(&format!(" {k}={v}"));
        }
        s
    }
}
