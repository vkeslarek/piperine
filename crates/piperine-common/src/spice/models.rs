use serde::{Deserialize, Serialize};

use super::spice_line::SpiceLine;

fn fmt_val(v: f64) -> String {
    if v.is_infinite() { return String::new(); }
    format!("{v}")
}

fn push_param(parts: &mut Vec<String>, name: &str, v: f64) {
    let s = fmt_val(v);
    if !s.is_empty() {
        parts.push(format!("{name}={s}"));
    }
}

// ── DiodeModel ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DiodeModel {
    pub name: String,
    pub is_: f64,
    pub rs: f64,
    pub n: f64,
    pub tt: f64,
    pub cjo: f64,
    pub vj: f64,
    pub m: f64,
    pub eg: f64,
    pub xti: f64,
    pub fc: f64,
    pub bv: f64,
    pub ibv: f64,
}

impl Default for DiodeModel {
    fn default() -> Self {
        DiodeModel {
            name: String::new(),
            is_: 1e-14, rs: 0.0, n: 1.0, tt: 0.0, cjo: 0.0,
            vj: 1.0, m: 0.5, eg: 1.11, xti: 3.0, fc: 0.5,
            bv: f64::INFINITY, ibv: 1e-3,
        }
    }
}

impl SpiceLine for DiodeModel {
    fn spice_line(&self) -> String {
        let mut parts = Vec::new();
        push_param(&mut parts, "is", self.is_);
        push_param(&mut parts, "rs", self.rs);
        push_param(&mut parts, "n", self.n);
        push_param(&mut parts, "tt", self.tt);
        push_param(&mut parts, "cjo", self.cjo);
        push_param(&mut parts, "vj", self.vj);
        push_param(&mut parts, "m", self.m);
        push_param(&mut parts, "eg", self.eg);
        push_param(&mut parts, "xti", self.xti);
        push_param(&mut parts, "fc", self.fc);
        push_param(&mut parts, "bv", self.bv);
        push_param(&mut parts, "ibv", self.ibv);
        format!(".model {} D ({})", self.name, parts.join(" "))
    }
}

// ── BjtModel ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BjtVariant { Npn, Pnp }

#[derive(Debug, Clone)]
pub struct BjtModel {
    pub name: String,
    pub variant: BjtVariant,
    pub is_: f64,
    pub bf: f64,
    pub br: f64,
    pub nf: f64,
    pub nr: f64,
    pub vaf: f64,
    pub var: f64,
    pub ise: f64,
    pub isc: f64,
    pub rb: f64,
    pub cje: f64,
    pub cjc: f64,
    pub tf: f64,
    pub tr: f64,
}

impl Default for BjtModel {
    fn default() -> Self {
        BjtModel {
            name: String::new(), variant: BjtVariant::Npn,
            is_: 1e-16, bf: 100.0, br: 1.0, nf: 1.0, nr: 1.0,
            vaf: f64::INFINITY, var: f64::INFINITY,
            ise: 0.0, isc: 0.0, rb: 0.0, cje: 0.0, cjc: 0.0, tf: 0.0, tr: 0.0,
        }
    }
}

impl SpiceLine for BjtModel {
    fn spice_line(&self) -> String {
        let typ = match self.variant { BjtVariant::Npn => "NPN", BjtVariant::Pnp => "PNP" };
        let mut parts = Vec::new();
        push_param(&mut parts, "is", self.is_);
        push_param(&mut parts, "bf", self.bf);
        push_param(&mut parts, "br", self.br);
        push_param(&mut parts, "nf", self.nf);
        push_param(&mut parts, "nr", self.nr);
        push_param(&mut parts, "vaf", self.vaf);
        push_param(&mut parts, "var", self.var);
        push_param(&mut parts, "ise", self.ise);
        push_param(&mut parts, "isc", self.isc);
        push_param(&mut parts, "rb", self.rb);
        push_param(&mut parts, "cje", self.cje);
        push_param(&mut parts, "cjc", self.cjc);
        push_param(&mut parts, "tf", self.tf);
        push_param(&mut parts, "tr", self.tr);
        format!(".model {} {} ({})", self.name, typ, parts.join(" "))
    }
}

// ── MosModel ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MosVariant { Nmos, Pmos }

#[derive(Debug, Clone)]
pub struct MosModel {
    pub name: String,
    pub variant: MosVariant,
    pub level: i32,
    pub vto: f64,
    pub kp: f64,
    pub gamma: f64,
    pub phi: f64,
    pub lambda: f64,
    pub tox: f64,
    pub uo: f64,
    pub cgso: f64,
    pub cgdo: f64,
    pub cgbo: f64,
}

impl Default for MosModel {
    fn default() -> Self {
        MosModel {
            name: String::new(), variant: MosVariant::Nmos, level: 1,
            vto: 0.0, kp: 2e-5, gamma: 0.0, phi: 0.6, lambda: 0.0,
            tox: 1e-7, uo: 600.0, cgso: 0.0, cgdo: 0.0, cgbo: 0.0,
        }
    }
}

impl SpiceLine for MosModel {
    fn spice_line(&self) -> String {
        let typ = match self.variant { MosVariant::Nmos => "NMOS", MosVariant::Pmos => "PMOS" };
        let mut parts = vec![format!("level={}", self.level)];
        push_param(&mut parts, "vto", self.vto);
        push_param(&mut parts, "kp", self.kp);
        push_param(&mut parts, "gamma", self.gamma);
        push_param(&mut parts, "phi", self.phi);
        push_param(&mut parts, "lambda", self.lambda);
        push_param(&mut parts, "tox", self.tox);
        push_param(&mut parts, "uo", self.uo);
        push_param(&mut parts, "cgso", self.cgso);
        push_param(&mut parts, "cgdo", self.cgdo);
        push_param(&mut parts, "cgbo", self.cgbo);
        format!(".model {} {} ({})", self.name, typ, parts.join(" "))
    }
}

// ── ResistorModel ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ResistorModel {
    pub name: String,
    pub rsh: f64,
    pub tc1: f64,
    pub tc2: f64,
}

impl Default for ResistorModel {
    fn default() -> Self {
        ResistorModel { name: String::new(), rsh: 0.0, tc1: 0.0, tc2: 0.0 }
    }
}

impl SpiceLine for ResistorModel {
    fn spice_line(&self) -> String {
        format!(".model {} R (rsh={} tc1={} tc2={})", self.name, self.rsh, self.tc1, self.tc2)
    }
}
