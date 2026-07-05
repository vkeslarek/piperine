//! The resolved lowering layer — formerly the standalone `piperine-ir`
//! crate. Verilog-AMS (the IR's other former producer) is gone; PHDL/PPR is
//! the only frontend, so this is no longer a cross-crate contract, just
//! codegen's private resolved form. Pure data plus the POM→resolved pass
//! (`pom/`): no JIT, no solver.
//!
//! Everything here is *resolved*: names are interned ids into a per-module
//! [`SymbolTable`]; ground is the reserved [`NodeId::GROUND`]. No generics,
//! lambdas, bundles, or structural control — those are elaborated away
//! before this layer.

mod expr;
mod stmt;
mod symbols;
mod diff;
mod validate;

pub mod pom;

pub use piperine_math as math;

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
pub use pom::{lower_bodies, LowerError, LowerErrors, LoweredBody};

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

// ─── Ports ────────────────────────────────────────────────────────────────────

/// Port direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrDirection {
    In,
    Out,
    Inout,
}

/// A module port: a resolved node plus a direction. The node's domain
/// (analog/digital) lives on its `NodeInfo`. Instance-level structure
/// (connections, param overrides) is resolved directly from the POM by
/// `device::circuit` at circuit-build time — there is no `IrModule`/
/// `IrInstance` structural twin.
#[derive(Debug, Clone)]
pub struct IrPort {
    pub node: NodeId,
    pub direction: IrDirection,
}
