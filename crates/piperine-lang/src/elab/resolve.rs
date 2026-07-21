use crate::elab::registry::{CallableDef, ElabContext};
use crate::parse::ast::{Expr, Literal, Path, Walk};
use crate::pom::{BehaviorStmt, ElabError, ElabErrorKind, ValueType};

/// Walk a program and resolve built-in diagnostic calls, type casts, and
/// (declared-language-surface T11/T12/T13) declared-first call resolution.
///
/// The lookup chain for a call site is: user-declared item (existing
/// `fn`/`impl` method, unchanged behavior — DLS-02) → the relevant registry
/// entry, plain or `extern` (DLS-03) → fail loud, naming the identifier and
/// its use site (DLS-04); an `extern` declaration whose native binding is
/// missing fails loud with a *distinct* error (DLS-05).
///
/// **Scope note (per-category progressive enforcement, design.md):** bare
/// identifier calls (`sin(x)`, `ddt(x)`, `flicker_noise(...)`, …) that have
/// **no** `CallableRegistry` entry are left untouched here — flipping
/// fail-loud globally for those categories is each P4 sub-phase's own job
/// (T16/T19/T20/T22); doing it now would break every existing stdlib header,
/// none of which declare `extern fn`/`extern operator` yet (verified: zero
/// `extern` declarations exist outside this feature's own test fixtures).
/// `Type::method(...)` call syntax (`Expr::Path` callees), by contrast, is
/// unused anywhere in the workspace today, so enforcing declared-first
/// lookup for it immediately is safe — this is the task's real,
/// immediately-provable behavior change (DLS-04), and the first real
/// consumer of the impl-method table built in T10.
pub fn resolve_calls(design: &mut crate::pom::Design, ctx: &ElabContext) -> Result<(), ElabError> {
    for module in design.modules_map_mut().values_mut() {
        for behavior in &mut module.behaviors {
            for stmt in &mut behavior.body {
                resolve_calls_in_stmt(stmt, &module.name, ctx)?;
            }
        }
    }
    Ok(())
}

fn resolve_calls_in_stmt(
    stmt: &mut BehaviorStmt,
    module_name: &str,
    ctx: &ElabContext,
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
            for s in &mut then_body.stmts {
                resolve_calls_in_stmt(s, module_name, ctx)?;
            }
            if let Some(eb) = else_body {
                for s in &mut eb.stmts {
                    resolve_calls_in_stmt(s, module_name, ctx)?;
                }
            }
        }
        BehaviorStmt::Match { arms, .. } => {
            for arm in arms {
                for s in &mut arm.body.stmts {
                    resolve_calls_in_stmt(s, module_name, ctx)?;
                }
            }
        }
        BehaviorStmt::Event { body, .. } => {
            for s in &mut body.stmts {
                resolve_calls_in_stmt(s, module_name, ctx)?;
            }
        }
        _ => {}
    }

    // Resolve expressions via walk_exprs_mut. We capture the first error
    // in a cell since the walk closure returns Walk, not Result.
    let mut err: Option<ElabError> = None;
    stmt.walk_exprs_mut(&mut |e| {
        if err.is_some() { return Walk::SkipChildren; }
        match resolve_calls_in_expr(e, module_name, ctx) {
            Ok(()) => Walk::Continue,
            Err(e) => { err = Some(e); Walk::SkipChildren }
        }
    });
    if let Some(e) = err { return Err(e); }

    Ok(())
}

/// The five bare-name cast forms (SPEC P4-AC7) — still special-cased here
/// unchanged until T17 deletes this and replaces it with `Type::from(x)`
/// (`Expr::Path` call syntax, resolved via `resolve_path_call` below).
const CAST_NAMES: [&str; 5] = ["real", "int", "bit", "Boolean", "Quad"];

/// Resolve type-cast calls (`real(x)`, `int(x)`, `bit(x)`, `Boolean(x)`,
/// `Quad(x)`) into `Expr::Cast` nodes, and (T11/T13) declared-first call
/// resolution for every other `Expr::Call`. This is a *transform* on the
/// current `Expr` node — the child recursion is done by the caller via
/// `walk_exprs_mut`. It only needs to handle the `Call` variant.
fn resolve_calls_in_expr(expr: &mut Expr, module_name: &str, ctx: &ElabContext) -> Result<(), ElabError> {
    if let Expr::Call(callee, args) = expr {
        match &**callee {
            Expr::Ident(name) if CAST_NAMES.contains(&name.as_str()) => {
                let name = name.clone();
                if args.len() != 1 {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "Cast to `{}` expects exactly 1 argument, got {}",
                        name, args.len()
                    ))));
                }
                let arg = args.remove(0);
                *expr = Expr::Cast(name, Box::new(arg));
            }
            Expr::Ident(name) => {
                let name = name.clone();
                resolve_declared_call(&name, args, module_name, ctx)?;
            }
            Expr::Path(path) => {
                resolve_path_call(path, args, module_name, ctx)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Declared-first resolution for a bare-identifier call (DLS-02/03/05): a
/// name with **no** `CallableRegistry` entry is left untouched (today's
/// pass-through behavior — see module-level scope note). A name that *is*
/// registered but resolves to a **plain** declaration proceeds exactly as
/// before this task — no new argument-type inference is forced onto plain
/// `fn` calls (DLS-02: "changes lookup order, not normal-declaration
/// behavior"; real static typechecking of arbitrary call arguments is
/// `elab/typecheck.rs`'s job, unaffected here). A name that resolves to an
/// **`extern`** candidate gets real signature validation (DLS-03) and a
/// native-binding check distinct from DLS-04's "no declaration" case
/// (DLS-05) — safe to enforce immediately since no real header declares any
/// `extern fn`/`extern task` yet.
fn resolve_declared_call(
    name: &str,
    args: &[Expr],
    module_name: &str,
    ctx: &ElabContext,
) -> Result<(), ElabError> {
    let candidates = ctx.callables.candidates(name);
    if candidates.is_empty() {
        return Ok(());
    }
    let resolved: &dyn CallableDef = if candidates.len() == 1 {
        candidates[0].as_ref()
    } else {
        let arg_types = infer_arg_types(args, name, module_name)?;
        ctx.callables.resolve(name, &arg_types)?
    };
    if resolved.is_extern() {
        let arg_types = infer_arg_types(args, name, module_name)?;
        resolved.validate_call(&arg_types)?;
        if crate::math::math_fn(name).is_none() {
            return Err(ElabError::from(ElabErrorKind::ExternMissingBinding {
                name: name.to_string(),
                module: module_name.to_string(),
            })
            .with_span(resolved.decl_span()));
        }
    }
    Ok(())
}

/// Declared-first resolution for a `Type::method(...)` call (DLS-01/03/04):
/// the impl-method table is the sole authority for this syntax — no
/// declaration anywhere (plain or `extern`) is a fail-loud error naming the
/// call site.
fn resolve_path_call(
    path: &Path,
    args: &[Expr],
    module_name: &str,
    ctx: &ElabContext,
) -> Result<(), ElabError> {
    // Only `Type::method` (exactly 2 segments) is meaningful call syntax for
    // the impl-method table today; anything else is left untouched.
    if path.segments.len() != 2 {
        return Ok(());
    }
    let type_name = &path.segments[0];
    let method_name = &path.segments[1];
    let qualified = format!("{type_name}::{method_name}");
    let candidates = ctx.impl_methods.candidates(type_name, method_name);
    if candidates.is_empty() {
        return Err(ElabError::from(ElabErrorKind::Other(format!(
            "no declaration found for `{qualified}` (called in module `{module_name}`) — \
             expected an `impl`/`extern impl {type_name}` method named `{method_name}`"
        ))));
    }
    let arg_types = infer_arg_types(args, &qualified, module_name)?;
    let resolved = if candidates.len() == 1 {
        candidates[0].as_ref()
    } else {
        ctx.impl_methods.resolve(type_name, method_name, &arg_types)?
    };
    resolved.validate_call(&arg_types)?;
    Ok(())
}

/// Best-effort literal argument-type inference for overload disambiguation
/// (DLS-07) — sufficient at this elaboration stage since the overload sets
/// this task exercises are proven via literal-argument fixtures; a call
/// whose argument type cannot be determined here fails loud rather than
/// silently guessing a candidate.
fn infer_arg_types(args: &[Expr], name: &str, module_name: &str) -> Result<Vec<ValueType>, ElabError> {
    args.iter()
        .map(|a| {
            infer_literal_arg_type(a).ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "cannot resolve overload of `{name}` in module `{module_name}`: \
                     argument type is not statically known at elaboration time"
                )))
            })
        })
        .collect()
}

fn infer_literal_arg_type(expr: &Expr) -> Option<ValueType> {
    match expr {
        Expr::Literal(Literal::Int(_)) => Some(ValueType::Integer),
        Expr::Literal(Literal::Real(_)) => Some(ValueType::Real),
        Expr::Literal(Literal::Bool(_)) => Some(ValueType::Boolean),
        Expr::Literal(Literal::Quad(_)) => Some(ValueType::Quad),
        Expr::Literal(Literal::String(_)) => Some(ValueType::Str),
        _ => None,
    }
}
