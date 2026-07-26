//! Behavior-statement resolution for analog bodies and function bodies:
//! walks POM `Stmt` and resolves expressions (constants, enum values,
//! analog operators → marker calls, user function inlining). The result is
//! POM `Stmt` stored in `AnalogBody.stmts` or `Function.body`.

use piperine_lang::parse::ast::{BindOp, Stmt};

use super::expr::{resolve_expr, resolve_stmt, parse_contrib_dest, scan_noise};
use super::LowerCtx;

/// Resolve a body of POM `Stmt` for the analog path: walk each statement
/// and resolve its expressions. Returns POM `Stmt` (not `IrStmt`).
pub(crate) fn resolve_stmts(stmts: &[Stmt], ctx: &mut LowerCtx) -> Vec<Stmt> {
    stmts.iter().map(|s| resolve_behavior_stmt(s, ctx)).collect()
}

/// Resolve a single POM `Stmt`, handling analog-specific constructs
/// (contributions, forces, events) that need special processing.
fn resolve_behavior_stmt(stmt: &Stmt, ctx: &mut LowerCtx) -> Stmt {
    match stmt {
        Stmt::Bind { dest, op: BindOp::Contrib, src } => {
            let (_nature, plus, minus) = parse_contrib_dest(dest, ctx);
            let resolved_dest = resolve_expr(dest, ctx);
            let resolved_src = resolve_expr(src, ctx);
            scan_noise(src, plus, minus, ctx);
            Stmt::Bind {
                dest: resolved_dest,
                op: BindOp::Contrib,
                src: resolved_src,
            }
        }
        Stmt::Bind { dest, op: BindOp::Force, src } => {
            let resolved_dest = resolve_expr(dest, ctx);
            let resolved_src = resolve_expr(src, ctx);
            Stmt::Bind { dest: resolved_dest, op: BindOp::Force, src: resolved_src }
        }
        _ => resolve_stmt(stmt, ctx),
    }
}
