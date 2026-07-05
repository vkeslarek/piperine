//! Interned ids and the per-module symbol table (SPEC §3, §7).

use super::expr::IrExpr;
use super::IrStmt;
use super::Type;

// ─── Ids ──────────────────────────────────────────────────────────────────────

/// A resolved net / terminal. Ground is the reserved [`NodeId::GROUND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    /// The MNA reference node (0 V).
    pub const GROUND: NodeId = NodeId(0);

    pub fn is_ground(self) -> bool {
        self == Self::GROUND
    }
}

/// A resolved parameter slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamId(pub u32);

/// A resolved runtime variable slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarId(pub u32);

/// An analog-operator state slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateId(pub u32);

/// A resolved user function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FnId(pub u32);

/// A discipline nature (access name plus potential/flow kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NatureId(pub u32);

// ─── Symbol infos ─────────────────────────────────────────────────────────────

/// Which simulation domain a node belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Analog,
    Digital,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub name: String,
    pub domain: Domain,
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub ty: Type,
    pub default: Option<IrExpr>,
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub ty: Type,
}

/// Whether a nature is an across (potential) or through (flow) quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatureKind {
    Potential,
    Flow,
}

#[derive(Debug, Clone)]
pub struct NatureInfo {
    /// The access-function name: `"V"`, `"I"`, `"Pwr"`, …
    pub access: String,
    pub kind: NatureKind,
}

// ─── Analog state operators (SPEC §7) ─────────────────────────────────────────

/// Inline measured-data table for [`StateKind::Table`].
#[derive(Debug, Clone, PartialEq)]
pub struct TableRef {
    /// `(input, output)` sample points, sorted by input.
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpMode {
    Linear,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaplaceKind {
    NumDen,
    ZerosPoles,
    NumPoles,
    ZerosDen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZKind {
    NumDen,
    ZerosPoles,
    NumPoles,
    ZerosDen,
}

/// An analog operator with internal state, referenced by `IrExpr::State(id)`.
/// `arg` is the operator input, evaluated each Newton iteration.
#[derive(Debug, Clone)]
pub struct StateVar {
    pub kind: StateKind,
    pub arg: IrExpr,
}

#[derive(Debug, Clone)]
pub enum StateKind {
    /// `ddt(x)` — time derivative (reactive).
    Ddt,
    /// `idt(x, ic)` — time integral (reactive).
    Idt { ic: IrExpr },
    /// `idtmod(x, ic, modulus)` — modular integral (reactive).
    IdtMod { ic: IrExpr, modulus: IrExpr },
    /// `ddx(x, node)` — compile-time derivative w.r.t. `V(node)`.
    Ddx { node: NodeId },
    /// `delay(x, t)` / `absdelay(x, t)` — delayed signal (ring buffer).
    Delay { delay: IrExpr },
    /// `transition(x, td, tr, tf, ttol)` — waveform shaping.
    Transition { delay: IrExpr, rise: IrExpr, fall: IrExpr, tol: IrExpr },
    /// `slew(x, rise, fall)` — rate limiting.
    Slew { rise: IrExpr, fall: IrExpr },
    /// Measured-data lookup.
    Table { data: TableRef, mode: InterpMode },
    /// `laplace_*(x, num, den)` — Laplace filter (reactive).
    Laplace { variant: LaplaceKind, num: Vec<IrExpr>, den: Vec<IrExpr> },
    /// `zi_*(x, num, den, dt)` — Z-transform filter (reactive).
    ZTransform { variant: ZKind, num: Vec<IrExpr>, den: Vec<IrExpr>, sample_dt: IrExpr },
}

impl StateKind {
    /// Reactive operators contribute charge stamped with the integration
    /// coefficient; resistive ones evaluate to a plain state value.
    pub fn is_reactive(&self) -> bool {
        matches!(
            self,
            Self::Ddt
                | Self::Idt { .. }
                | Self::IdtMod { .. }
                | Self::Laplace { .. }
                | Self::ZTransform { .. }
        )
    }

    /// Display name for diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ddt => "ddt",
            Self::Idt { .. } => "idt",
            Self::IdtMod { .. } => "idtmod",
            Self::Ddx { .. } => "ddx",
            Self::Delay { .. } => "delay",
            Self::Transition { .. } => "transition",
            Self::Slew { .. } => "slew",
            Self::Table { .. } => "table",
            Self::Laplace { .. } => "laplace",
            Self::ZTransform { .. } => "zi",
        }
    }
}

// ─── Noise (SPEC §6.2) ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NoiseSource {
    pub plus: NodeId,
    pub minus: NodeId,
    pub kind: NoiseKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub enum NoiseKind {
    White { psd: IrExpr },
    Flicker { psd: IrExpr, exponent: IrExpr },
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// A user-defined function. Parameters are variable slots in the module's
/// symbol table; the body uses the shared statement set (SPEC §8).
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<VarId>,
    /// Default value expressions, parallel to [`params`](Self::params) —
    /// `None` for a non-defaulted param, `Some(expr)` for a defaulted
    /// trailing one (the language spec Part I §9.1). Filled by the inliner at expansion.
    pub defaults: Vec<Option<IrExpr>>,
    pub returns: Option<Type>,
    pub body: Vec<IrStmt>,
}

// ─── Symbol table ─────────────────────────────────────────────────────────────

/// Per-module arena mapping dense ids to their infos. Names exist for display
/// and diagnostics only — expressions carry ids.
#[derive(Debug, Clone)]
pub struct SymbolTable {
    nodes: Vec<NodeInfo>,
    params: Vec<ParamInfo>,
    vars: Vec<VarInfo>,
    states: Vec<StateVar>,
    natures: Vec<NatureInfo>,
    fns: Vec<Function>,
}

impl SymbolTable {
    /// A fresh table with ground pre-interned as [`NodeId::GROUND`].
    pub fn new() -> Self {
        Self {
            nodes: vec![NodeInfo { name: "gnd".into(), domain: Domain::Analog }],
            params: Vec::new(),
            vars: Vec::new(),
            states: Vec::new(),
            natures: Vec::new(),
            fns: Vec::new(),
        }
    }

    // ── Interning ──

    pub fn add_node(&mut self, name: impl Into<String>, domain: Domain) -> NodeId {
        self.nodes.push(NodeInfo { name: name.into(), domain });
        NodeId(self.nodes.len() as u32 - 1)
    }

    pub fn add_param(
        &mut self,
        name: impl Into<String>,
        ty: Type,
        default: Option<IrExpr>,
    ) -> ParamId {
        self.params.push(ParamInfo { name: name.into(), ty, default });
        ParamId(self.params.len() as u32 - 1)
    }

    pub fn add_var(&mut self, name: impl Into<String>, ty: Type) -> VarId {
        self.vars.push(VarInfo { name: name.into(), ty });
        VarId(self.vars.len() as u32 - 1)
    }

    pub fn add_state(&mut self, state: StateVar) -> StateId {
        self.states.push(state);
        StateId(self.states.len() as u32 - 1)
    }

    pub fn add_nature(&mut self, access: impl Into<String>, kind: NatureKind) -> NatureId {
        self.natures.push(NatureInfo { access: access.into(), kind });
        NatureId(self.natures.len() as u32 - 1)
    }

    pub fn add_fn(&mut self, function: Function) -> FnId {
        self.fns.push(function);
        FnId(self.fns.len() as u32 - 1)
    }

    // ── Lookup (panics on a dangling id: emitters must only produce resolved ids) ──

    pub fn node(&self, id: NodeId) -> &NodeInfo {
        &self.nodes[id.0 as usize]
    }

    pub fn param(&self, id: ParamId) -> &ParamInfo {
        &self.params[id.0 as usize]
    }

    pub fn var(&self, id: VarId) -> &VarInfo {
        &self.vars[id.0 as usize]
    }

    pub fn state(&self, id: StateId) -> &StateVar {
        &self.states[id.0 as usize]
    }

    pub fn nature(&self, id: NatureId) -> &NatureInfo {
        &self.natures[id.0 as usize]
    }

    pub fn function(&self, id: FnId) -> &Function {
        &self.fns[id.0 as usize]
    }

    // ── Checked lookup (for validation) ──

    pub fn try_node(&self, id: NodeId) -> Option<&NodeInfo> {
        self.nodes.get(id.0 as usize)
    }

    pub fn try_param(&self, id: ParamId) -> Option<&ParamInfo> {
        self.params.get(id.0 as usize)
    }

    pub fn try_var(&self, id: VarId) -> Option<&VarInfo> {
        self.vars.get(id.0 as usize)
    }

    pub fn try_state(&self, id: StateId) -> Option<&StateVar> {
        self.states.get(id.0 as usize)
    }

    pub fn try_nature(&self, id: NatureId) -> Option<&NatureInfo> {
        self.natures.get(id.0 as usize)
    }

    pub fn try_fn(&self, id: FnId) -> Option<&Function> {
        self.fns.get(id.0 as usize)
    }

    // ── Iteration ──

    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &NodeInfo)> {
        self.nodes.iter().enumerate().map(|(i, n)| (NodeId(i as u32), n))
    }

    pub fn params(&self) -> impl Iterator<Item = (ParamId, &ParamInfo)> {
        self.params.iter().enumerate().map(|(i, p)| (ParamId(i as u32), p))
    }

    pub fn vars(&self) -> impl Iterator<Item = (VarId, &VarInfo)> {
        self.vars.iter().enumerate().map(|(i, v)| (VarId(i as u32), v))
    }

    pub fn states(&self) -> impl Iterator<Item = (StateId, &StateVar)> {
        self.states.iter().enumerate().map(|(i, s)| (StateId(i as u32), s))
    }

    pub fn num_params(&self) -> usize {
        self.params.len()
    }

    pub fn num_states(&self) -> usize {
        self.states.len()
    }

    /// Look up a function id by name (display convenience; hot paths use ids).
    pub fn fn_by_name(&self, name: &str) -> Option<FnId> {
        self.fns.iter().position(|f| f.name == name).map(|i| FnId(i as u32))
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}
