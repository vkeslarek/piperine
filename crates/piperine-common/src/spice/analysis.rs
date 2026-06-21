use std::fmt::Write;

use serde::{Deserialize, Serialize};

use super::spice_line::SpiceLine;
use super::spice_node::SpiceNode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Analysis {
    Op,
    Dc { src: String, start: f64, stop: f64, step: f64 },
    Tran { step: f64, stop: f64, start: f64, max: Option<f64>, uic: bool },
    Ac { variant: AcVariant, n: usize, start: f64, stop: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcVariant { Dec, Oct, Lin }

impl Analysis {
    pub fn deck_line(&self) -> String {
        match self {
            Analysis::Op => ".op".into(),
            Analysis::Dc { src, start, stop, step } =>
                format!(".dc {src} {start} {stop} {step}"),
            Analysis::Tran { step, stop, start, max, uic } => {
                let mut s = format!(".tran {step} {stop} {start}");
                if let Some(m) = max { write!(&mut s, " {m}").unwrap(); }
                if *uic { s.push_str(" uic"); }
                s
            }
            Analysis::Ac { variant, n, start, stop } => {
                let v = match variant { AcVariant::Dec => "dec", AcVariant::Oct => "oct", AcVariant::Lin => "lin" };
                format!(".ac {v} {n} {start} {stop}")
            }
        }
    }

    pub fn cmd_line(&self) -> String {
        match self {
            Analysis::Op => "op".into(),
            Analysis::Dc { src, start, stop, step } =>
                format!("dc {src} {start} {stop} {step}"),
            Analysis::Tran { step, stop, start, max, uic } => {
                let mut s = format!("tran {step} {stop} {start}");
                if let Some(m) = max { write!(&mut s, " {m}").unwrap(); }
                if *uic { s.push_str(" uic"); }
                s
            }
            Analysis::Ac { variant, n, start, stop } => {
                let v = match variant { AcVariant::Dec => "dec", AcVariant::Oct => "oct", AcVariant::Lin => "lin" };
                format!("ac {v} {n} {start} {stop}")
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub reltol: Option<f64>,
    pub abstol: Option<f64>,
    pub vntol: Option<f64>,
    pub temp: Option<f64>,
    pub tnom: Option<f64>,
    pub itl1: Option<i32>,
    pub itl4: Option<i32>,
}

impl SpiceLine for Options {
    fn spice_line(&self) -> String {
        let mut parts = vec!["".to_string()];
        if let Some(v) = self.reltol { parts.push(format!("reltol={v}")); }
        if let Some(v) = self.abstol { parts.push(format!("abstol={v}")); }
        if let Some(v) = self.vntol { parts.push(format!("vntol={v}")); }
        if let Some(v) = self.temp { parts.push(format!("temp={v}")); }
        if let Some(v) = self.tnom { parts.push(format!("tnom={v}")); }
        if let Some(v) = self.itl1 { parts.push(format!("itl1={v}")); }
        if let Some(v) = self.itl4 { parts.push(format!("itl4={v}")); }
        if parts.len() == 1 { return String::new(); }
        format!(".options{}", parts.join(" "))
    }
}

#[derive(Debug, Clone)]
pub struct Temp { pub value: f64 }

impl SpiceLine for Temp {
    fn spice_line(&self) -> String {
        format!(".temp {}", self.value)
    }
}

#[derive(Debug, Clone)]
pub struct Save { pub vectors: Vec<String> }

impl SpiceLine for Save {
    fn spice_line(&self) -> String {
        format!(".save {}", self.vectors.join(" "))
    }
}

#[derive(Debug, Clone)]
pub struct Ic { pub node: SpiceNode, pub value: f64 }

impl SpiceLine for Ic {
    fn spice_line(&self) -> String {
        format!(".ic v({})={}", self.node, self.value)
    }
}
