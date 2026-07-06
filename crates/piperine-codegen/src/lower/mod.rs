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
pub mod diff;
mod validate;

pub mod pom;

pub use piperine_lang::math;

pub use expr::{Analysis, Axis, BinOp, UnOp, pom_eval_const};
pub use stmt::{
    ContribKind, CrossDir, DigitalEvent, EdgeKind, EventSource, AnalogEvent, Severity,
};
pub use symbols::{
    Domain, FnId, InterpMode, Function, NoiseKind, NoiseSource, StateKind, StateVar,
    LaplaceKind, NatureId, NatureInfo, NatureKind, NodeId, NodeInfo, ParamId, ParamInfo, StateId,
    SymbolTable, TableRef, VarId, VarInfo, ZKind,
};
pub use validate::{Diagnostic, DiagnosticKind};
pub use pom::{lower_bodies, LowerError, LowerErrors, LoweredBody};

// ─── Types ────────────────────────────────────────────────────────────────────

/// The IR value types. Everything in analog evaluation is `Real`;
/// `Integer`/`Bool` distinguish storage and control flow; `Quad` is 4-state
/// digital (0/1/X/Z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Real,
    Integer,
    Bool,
    Quad,
}

// ─── Bodies ───────────────────────────────────────────────────────────────────

/// Analog behavior: contribution/force statements plus structured control,
/// with operator state slots and noise sources hoisted out at emit time.
#[derive(Debug, Clone, Default)]
pub struct AnalogBody {
    /// Operator state slots referenced by this body (ids into `symbols.states`).
    pub states: Vec<StateId>,
    pub noise: Vec<NoiseSource>,
    pub stmts: Vec<piperine_lang::parse::ast::Stmt>,
}

/// Digital behavior: the PHDL model — combinational logic with inferred
/// memory, plus clocked registers. Not the Verilog procedural kernel.
#[derive(Debug, Clone, Default)]
pub struct DigitalBody {
    pub inputs: Vec<NodeId>,
    pub outputs: Vec<NodeId>,
    /// Variables holding state across timesteps (registers and latches).
    pub regs: Vec<VarId>,
    /// Combinational statements plus clocked `@`-blocks, kept as the POM
    /// `Stmt` tree (the AST type). The `Codegen` trait emits these directly
    /// to Cranelift via the `Builder` — no `IrStmt` intermediate.
    pub stmts: Vec<piperine_lang::parse::ast::Stmt>,
}

// ─── Ports ────────────────────────────────────────────────────────────────────

/// Port direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
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
pub struct Port {
    pub node: NodeId,
    pub direction: Direction,
}
