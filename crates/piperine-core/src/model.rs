use crate::netlist::ToNetlist;

/// Device model kind (maps to SPICE .model types).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    R,
    C,
    L,
    D,
    NPN,
    PNP,
    NJF,
    PJF,
    NMOS,
    PMOS,
    VDMOS,
    SW,
    CSW,
}

impl ModelKind {
    pub fn to_spice(&self) -> &'static str {
        match self {
            ModelKind::R => "R",
            ModelKind::C => "C",
            ModelKind::L => "L",
            ModelKind::D => "D",
            ModelKind::NPN => "NPN",
            ModelKind::PNP => "PNP",
            ModelKind::NJF => "NJF",
            ModelKind::PJF => "PJF",
            ModelKind::NMOS => "NMOS",
            ModelKind::PMOS => "PMOS",
            ModelKind::VDMOS => "VDMOS",
            ModelKind::SW => "SW",
            ModelKind::CSW => "CSW",
        }
    }
}

/// A .model definition with parameters.
#[derive(Debug, Clone)]
pub struct ModelDef {
    pub name: String,
    pub kind: ModelKind,
    pub params: Vec<(String, String)>,
    pub level: Option<u32>,
}

impl ModelDef {
    pub fn new(name: &str, kind: ModelKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            params: Vec::new(),
            level: None,
        }
    }

    pub fn param(mut self, name: &str, value: impl std::fmt::Display) -> Self {
        self.params.push((name.to_string(), value.to_string()));
        self
    }

    pub fn level(mut self, level: u32) -> Self {
        self.level = Some(level);
        self
    }
}

impl ToNetlist for ModelDef {
    fn to_netlist_lines(&self) -> Vec<String> {
        let mut s = format!(".model {} {}", self.name, self.kind.to_spice());
        if let Some(lvl) = self.level {
            s.push_str(&format!(" level={lvl}"));
        }
        if !self.params.is_empty() {
            let params: Vec<String> = self.params.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            s.push_str(&format!("({})", params.join(" ")));
        }
        vec![s]
    }
}
