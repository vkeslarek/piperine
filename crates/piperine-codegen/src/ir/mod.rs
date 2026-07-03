//! The Piperine IR — the post-elaboration, resolved representation both
//! frontends lower into and the codegen consumes. See `docs/SPEC.md`.
//!
//! Everything here is *resolved*: names are interned ids into a per-module
//! [`SymbolTable`]; ground is the reserved [`NodeId::GROUND`]. The IR carries
//! no generics, lambdas, bundles, or structural control — those are
//! elaborated away before emission.

mod expr;
mod stmt;
mod symbols;
mod validate;

pub use expr::{Analysis, Axis, IrBinOp, IrExpr, IrUnOp, SimQuery};
pub use stmt::{
    ContribKind, CrossDir, DigitalEvent, EdgeKind, EventSource, IrAnalogEvent, IrStmt, Lval,
    Pattern, Severity, Trit,
};
pub use symbols::{
    Domain, FnId, InterpMode, IrFunction, IrNoise, IrNoiseSource, IrStateKind, IrStateVar,
    LaplaceKind, NatureId, NatureInfo, NatureKind, NodeId, NodeInfo, ParamId, ParamInfo, StateId,
    SymbolTable, TableRef, VarId, VarInfo, ZKind,
};
pub use validate::{IrDiagnostic, IrDiagnosticKind};

// ─── Types ────────────────────────────────────────────────────────────────────

/// The IR value types. Everything in analog evaluation is `Real`;
/// `Integer`/`Bool` distinguish storage and control flow; `Quad` is 4-state
/// digital (0/1/X/Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    Real,
    Integer,
    Bool,
    Quad,
}

// ─── Bodies ───────────────────────────────────────────────────────────────────

/// Analog behavior: contribution/force statements plus structured control,
/// with operator state slots and noise sources hoisted out at emit time.
#[derive(Debug, Clone, Default)]
pub struct IrAnalogBody {
    /// Operator state slots referenced by this body (ids into `symbols.states`).
    pub states: Vec<StateId>,
    pub noise: Vec<IrNoiseSource>,
    pub stmts: Vec<IrStmt>,
}

/// Digital behavior: the PHDL model — combinational logic with inferred
/// memory, plus clocked registers. Not the Verilog procedural kernel.
#[derive(Debug, Clone, Default)]
pub struct IrDigitalBody {
    pub inputs: Vec<NodeId>,
    pub outputs: Vec<NodeId>,
    /// Variables holding state across timesteps (registers and latches).
    pub regs: Vec<VarId>,
    /// Combinational statements plus `ClockedBlock`s.
    pub stmts: Vec<IrStmt>,
}

// ─── Module and program ───────────────────────────────────────────────────────

/// Port direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrDirection {
    In,
    Out,
    Inout,
}

/// A module port: a resolved node plus a direction. The node's domain
/// (analog/digital) lives on its `NodeInfo`.
#[derive(Debug, Clone)]
pub struct IrPort {
    pub node: NodeId,
    pub direction: IrDirection,
}

/// A child instance. `connections[i]` is the parent node wired to the child's
/// i-th port; `params` carries override expressions evaluated in the parent's
/// scope, keyed by the child's `ParamId`.
#[derive(Debug, Clone)]
pub struct IrInstance {
    pub label: String,
    pub module: String,
    pub connections: Vec<NodeId>,
    pub params: Vec<(ParamId, IrExpr)>,
}

/// A monomorphic, flat module.
#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub symbols: SymbolTable,
    pub ports: Vec<IrPort>,
    pub instances: Vec<IrInstance>,
    pub analog: Option<IrAnalogBody>,
    pub digital: Option<IrDigitalBody>,
}

impl IrModule {
    /// A module with an empty symbol table and no bodies.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            symbols: SymbolTable::new(),
            ports: Vec::new(),
            instances: Vec::new(),
            analog: None,
            digital: None,
        }
    }
}

/// Which frontend produced the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Ams,
    Ppr,
}

/// A resolved program: a set of monomorphic modules.
#[derive(Debug, Clone)]
pub struct IrProgram {
    pub source: Source,
    pub modules: Vec<IrModule>,
}

impl IrProgram {
    pub fn new(source: Source) -> Self {
        Self { source, modules: Vec::new() }
    }

    /// Look up a module by name.
    pub fn module(&self, name: &str) -> Option<&IrModule> {
        self.modules.iter().find(|m| m.name == name)
    }
}
