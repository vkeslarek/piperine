//! Behavior-statement resolution for analog bodies and function bodies:
//! walks POM `Stmt` and resolves expressions (constants, enum values,
//! analog operators → marker calls, user function inlining). The result is
//! POM `Stmt` stored in `AnalogBody.stmts` or `Function.body`.

use piperine_lang::parse::ast::{BindOp, Expr, Stmt};

use crate::resolve::*;

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

/// Check if an expression contains a `__ddt` marker (reactive).
pub(crate) fn has_ddt_marker(expr: &Expr) -> bool {
    use piperine_lang::parse::ast::Walk;
    let mut found = false;
    expr.walk(&mut |e| {
        if let Expr::Call(func, _) = e
            && let Expr::Ident(name) = func.as_ref()
                && (name == "__ddt" || name == "__laplace" || name == "__ztransform") {
                    found = true;
                    return Walk::SkipChildren;
                }
        Walk::Continue
    });
    found
}

/// Extract the `(plus, minus)` node pair from a contribution destination
/// `V(p,n)` or `I(p,n)`. Used by the flattener.
pub(crate) fn contrib_branch(dest: &Expr, ctx: &mut LowerCtx) -> (NatureId, NodeId, NodeId) {
    parse_contrib_dest(dest, ctx)
}
