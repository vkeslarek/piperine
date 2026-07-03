use crate::parse::ast::{Expr, Walk};
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
    // First resolve any BehaviorStmt-specific logic (Diagnostic validation),
    // then delegate expression traversal to walk_exprs_mut + resolve_calls_in_expr.
    if let BehaviorStmt::Diagnostic { sys, .. } = stmt {
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
    }

    // Recurse into sub-statements for If/Match/Event.
    match stmt {
        BehaviorStmt::If { then_body, else_body, .. } => {
            for s in then_body {
                resolve_calls_in_stmt(s, module_name, behavior_name)?;
            }
            if let Some(eb) = else_body {
                for s in eb {
                    resolve_calls_in_stmt(s, module_name, behavior_name)?;
                }
            }
        }
        BehaviorStmt::Match { arms, .. } => {
            for arm in arms {
                for s in &mut arm.body {
                    resolve_calls_in_stmt(s, module_name, behavior_name)?;
                }
            }
        }
        BehaviorStmt::Event { body, .. } => {
            for s in body {
                resolve_calls_in_stmt(s, module_name, behavior_name)?;
            }
        }
        _ => {}
    }

    // Resolve expressions via walk_exprs_mut. We capture the first error
    // in a cell since the walk closure returns Walk, not Result.
    let mut err: Option<ElabError> = None;
    stmt.walk_exprs_mut(&mut |e| {
        if err.is_some() { return Walk::SkipChildren; }
        match resolve_calls_in_expr(e) {
            Ok(()) => Walk::Continue,
            Err(e) => { err = Some(e); Walk::SkipChildren }
        }
    });
    if let Some(e) = err { return Err(e); }

    Ok(())
}

/// Resolve type-cast calls (`real(x)`, `int(x)`, `bit(x)`, `Boolean(x)`,
/// `Quad(x)`) into `Expr::Cast` nodes. This is a *transform* on the
/// current `Expr` node — the child recursion is done by the caller via
/// `walk_exprs_mut`. It only needs to handle the `Call` variant.
fn resolve_calls_in_expr(expr: &mut Expr) -> Result<(), ElabError> {
    if let Expr::Call(callee, args) = expr {
        if let Expr::Ident(name) = &**callee {
            if matches!(name.as_str(), "real" | "int" | "bit" | "Boolean" | "Quad") {
                if args.len() != 1 {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "Cast to `{}` expects exactly 1 argument, got {}",
                        name, args.len()
                    ))));
                }
                let arg = args.remove(0);
                let cast_name = name.clone();
                *expr = Expr::Cast(cast_name, Box::new(arg));
            }
        }
    }
    Ok(())
}
