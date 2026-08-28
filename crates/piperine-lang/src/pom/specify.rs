//! POM [`SpecifyBlock`] — Annex A.7 timing specification, const-evaluated
//! at elaboration time so it round-trips through serde like the rest of
//! the POM (T18, LRMW3-06/LRMW3-07). The authored `ast::SpecifyBlock`
//! keeps its delay/limit values as `Expr` (LRM A.7.4/A.7.5.2 both define
//! them as constant expressions); this module's shapes hold the evaluated
//! `f64` instead, mirroring how every other POM value (a param default, a
//! net's nodeset) is a resolved constant, never a compiled expression.
//!
//! Multiple authored `specify...endspecify` blocks in one module compose:
//! `Module::specify()`'s lists are the concatenation, in authored order,
//! of every block's own lists — the same "multiple blocks compose" rule
//! `constraints` already uses.

/// One `specify` block's contents, const-evaluated (T18).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SpecifyBlock {
    /// `specparam name = value;` (Annex A.2.4), in authored order.
    pub specparams: Vec<(String, f64)>,
    /// Simple path delays (Annex A.7.2/A.7.4), in authored order.
    pub paths: Vec<PathDelay>,
    /// System timing checks (Annex A.7.5), in authored order.
    pub checks: Vec<TimingCheck>,
}

impl SpecifyBlock {
    pub fn is_empty(&self) -> bool {
        self.specparams.is_empty() && self.paths.is_empty() && self.checks.is_empty()
    }
}

/// A simple path declaration's terminals and const-evaluated delay(s)
/// (Annex A.7.2/A.7.3/A.7.4). `inputs`/`outputs` name declared ports of
/// the enclosing module (validated at lowering — T18).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PathDelay {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// `true` for a full-connection (`*>`) path, `false` for parallel (`=>`).
    pub full: bool,
    /// 1, 2, 3, 6 or 12 const-evaluated delay values, A.7.4.
    pub delays: Vec<f64>,
}

/// `edge_identifier`, Annex A.7.2/A.7.5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EdgeIdent {
    Posedge,
    Negedge,
}

/// `system_timing_check`'s eight in-scope kinds (Annex A.7.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TimingCheckKind {
    Setup,
    Hold,
    SetupHold,
    Width,
    Period,
    Recovery,
    Removal,
    Skew,
}

/// One const-evaluated system timing check (Annex A.7.5.1/A.7.5.2/A.7.5.3).
/// `data_terminal`/`data_edge` are `None` for the single-event forms
/// (`$width`/`$period`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TimingCheck {
    pub kind: TimingCheckKind,
    pub reference_terminal: String,
    pub reference_edge: Option<EdgeIdent>,
    pub data_terminal: Option<String>,
    pub data_edge: Option<EdgeIdent>,
    /// One limit, except `$setuphold` (setup limit, then hold limit).
    pub limits: Vec<f64>,
    pub notifier: Option<String>,
}
