//! Behavior-statement resolution for analog bodies: walks VAMS `Stmt` and
//! resolves expressions (constants, analog operators → marker calls, user
//! function inlining). The result is POM `Stmt` stored in `AnalogBody.stmts`.
//!
//! PHDL's separate `BindOp::Force` (`dest <- src`, an ideal-source analog
//! "force" distinct from `<+` contribution) has no VAMS Annex A production
//! — Verilog-AMS has only `<+` (`Stmt::Contribution`) for driving a branch;
//! an ideal-source contribution (`V(p,n) <+ V0;`) is ordinary real
//! Verilog-A, not a distinct operator. `resolve_expr`/`resolve_stmt`
//! (`expr.rs`) already handle `Contribution` directly, so this module is
//! now a thin per-statement-list driver rather than a dispatcher between
//! two bind forms (MD-45-consistent deletion — documented, not silent).

use piperine_vams::ast::Stmt;

use super::expr::resolve_stmt;
use super::LowerCtx;

/// Resolve a body of VAMS `Stmt` for the analog path: walk each statement
/// and resolve its expressions.
pub(crate) fn resolve_stmts(stmts: &[Stmt], ctx: &mut LowerCtx) -> Vec<Stmt> {
    stmts.iter().map(|s| resolve_stmt(s, ctx)).collect()
}
