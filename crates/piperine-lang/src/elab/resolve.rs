use crate::parse::ast::Expr;
use crate::pom::{BehaviorStmt, ElabError, ElabErrorKind};

/// Walk a program and resolve built-in diagnostic calls and type casts.
/// This fulfills GAPS §J.4.
pub fn resolve_calls(design: &mut crate::pom::Design) -> Result<(), ElabError> {
    for module in design.modules_map_mut().values_mut() {
        for behavior in &mut module.behaviors {
            for stmt in &mut behavior.body {
                resolve_calls_in_stmt(stmt, &module.name, &behavior.name)?;
            }
        }
    }
    Ok(())
}

fn resolve_calls_in_stmt(
    stmt: &mut BehaviorStmt,
    module_name: &str,
    behavior_name: &str,
) -> Result<(), ElabError> {
    match stmt {
        BehaviorStmt::Bind { dest, op: _, src } => {
            resolve_calls_in_expr(dest)?;
            resolve_calls_in_expr(src)?;
        }
        BehaviorStmt::If { cond, then_body, else_body } => {
            resolve_calls_in_expr(cond)?;
            for s in then_body {
                resolve_calls_in_stmt(s, module_name, behavior_name)?;
            }
            if let Some(eb) = else_body {
                for s in eb {
                    resolve_calls_in_stmt(s, module_name, behavior_name)?;
                }
            }
        }
        BehaviorStmt::Match { expr, arms } => {
            resolve_calls_in_expr(expr)?;
            for arm in arms {
                for s in &mut arm.body {
                    resolve_calls_in_stmt(s, module_name, behavior_name)?;
                }
            }
        }
        BehaviorStmt::Event { spec: _, guard, body } => {
            if let Some(g) = guard {
                resolve_calls_in_expr(g)?;
            }
            for s in body {
                resolve_calls_in_stmt(s, module_name, behavior_name)?;
            }
        }
        BehaviorStmt::VarDecl { default, .. } => {
            if let Some(d) = default {
                resolve_calls_in_expr(d)?;
            }
        }
        BehaviorStmt::Return(expr) => {
            resolve_calls_in_expr(expr)?;
        }
        BehaviorStmt::Diagnostic { sys, args } => {
            // Validate diagnostic name.
            let valid_diagnostics = [
                "write", "strobe", "display", "info", "warning", "error", "fatal",
                "bound_step", "finish", "stop", "discontinuity"
            ];
            if !valid_diagnostics.contains(&sys.as_str()) {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "Unrecognized diagnostic call `{}` in module `{}`",
                    sys, module_name
                ))));
            }
            for arg in args {
                resolve_calls_in_expr(arg)?;
            }
        }
        BehaviorStmt::Expr(expr) => {
            resolve_calls_in_expr(expr)?;
        }
    }
    Ok(())
}

fn resolve_calls_in_expr(expr: &mut Expr) -> Result<(), ElabError> {
    match expr {
        Expr::Call(callee, args) => {
            if let Expr::Ident(name) = &**callee {
                if matches!(name.as_str(), "real" | "int" | "bit" | "Boolean" | "Quad") {
                    if args.len() != 1 {
                        return Err(ElabError::from(ElabErrorKind::Other(format!(
                            "Cast to `{}` expects exactly 1 argument, got {}",
                            name, args.len()
                        ))));
                    }
                    let mut arg = args.remove(0);
                    resolve_calls_in_expr(&mut arg)?;
                    *expr = Expr::Cast(name.clone(), Box::new(arg));
                    return Ok(());
                }
            }
            resolve_calls_in_expr(callee)?;
            for arg in args {
                resolve_calls_in_expr(arg)?;
            }
        }
        Expr::SysCall(_name, args) => {
            for arg in args {
                resolve_calls_in_expr(arg)?;
            }
        }
        Expr::Unary(_, inner) => resolve_calls_in_expr(inner)?,
        Expr::Binary(lhs, _, rhs) => {
            resolve_calls_in_expr(lhs)?;
            resolve_calls_in_expr(rhs)?;
        }
        Expr::Index(base, idx) => {
            resolve_calls_in_expr(base)?;
            resolve_calls_in_expr(idx)?;
        }
        Expr::Slice(base, _) => resolve_calls_in_expr(base)?,
        Expr::Field(base, _) => resolve_calls_in_expr(base)?,
        Expr::Block(block) => {
            for s in &mut block.stmts {
                resolve_calls_in_ast_stmt(s)?;
            }
            if let Some(e) = &mut block.expr {
                resolve_calls_in_expr(e)?;
            }
        }
        Expr::If { cond, then_body, else_body } => {
            resolve_calls_in_expr(cond)?;
            for s in &mut then_body.stmts { resolve_calls_in_ast_stmt(s)?; }
            if let Some(e) = &mut then_body.expr { resolve_calls_in_expr(e)?; }
            for s in &mut else_body.stmts { resolve_calls_in_ast_stmt(s)?; }
            if let Some(e) = &mut else_body.expr { resolve_calls_in_expr(e)?; }
        }
        Expr::Array(body) => match body {
            crate::parse::ast::ArrayBody::Repeat(e, n) => {
                resolve_calls_in_expr(e)?;
                resolve_calls_in_expr(n)?;
            }
            crate::parse::ast::ArrayBody::Comprehension(e, _, _) => {
                resolve_calls_in_expr(e)?;
            }
            crate::parse::ast::ArrayBody::List(list) => {
                for e in list {
                    resolve_calls_in_expr(e)?;
                }
            }
        },
        Expr::BundleLit { fields, .. } => {
            for (_, e) in fields {
                resolve_calls_in_expr(e)?;
            }
        }
        Expr::Lambda { body, .. } => {
            resolve_calls_in_expr(body)?;
        }
        Expr::Cast(_, inner) => resolve_calls_in_expr(inner)?,
        Expr::Literal(_) | Expr::Ident(_) | Expr::Path(_) => {}
    }
    Ok(())
}

fn resolve_calls_in_ast_stmt(stmt: &mut crate::parse::ast::Stmt) -> Result<(), ElabError> {
    match stmt {
        crate::parse::ast::Stmt::Expr(e) => resolve_calls_in_expr(e)?,
        crate::parse::ast::Stmt::VarDecl { default, .. } => {
            if let Some(d) = default {
                resolve_calls_in_expr(d)?;
            }
        }
        crate::parse::ast::Stmt::Return(e) => resolve_calls_in_expr(e)?,
        crate::parse::ast::Stmt::If { cond, then_body, else_body } => {
            resolve_calls_in_expr(cond)?;
            for s in &mut then_body.stmts { resolve_calls_in_ast_stmt(s)?; }
            if let Some(e) = &mut then_body.expr { resolve_calls_in_expr(e)?; }
            if let Some(eb) = else_body {
                for s in &mut eb.stmts { resolve_calls_in_ast_stmt(s)?; }
                if let Some(e) = &mut eb.expr { resolve_calls_in_expr(e)?; }
            }
        }
        crate::parse::ast::Stmt::Match { expr, arms } => {
            resolve_calls_in_expr(expr)?;
            for arm in arms {
                for s in &mut arm.body.stmts { resolve_calls_in_ast_stmt(s)?; }
                if let Some(e) = &mut arm.body.expr { resolve_calls_in_expr(e)?; }
            }
        }
        crate::parse::ast::Stmt::For { range, body, .. } => {
            resolve_calls_in_expr(&mut range.start)?;
            resolve_calls_in_expr(&mut range.end)?;
            for s in &mut body.stmts { resolve_calls_in_ast_stmt(s)?; }
            if let Some(e) = &mut body.expr { resolve_calls_in_expr(e)?; }
        }
        crate::parse::ast::Stmt::Bind { dest, src, .. } => {
            resolve_calls_in_expr(dest)?;
            resolve_calls_in_expr(src)?;
        }
    }
    Ok(())
}
