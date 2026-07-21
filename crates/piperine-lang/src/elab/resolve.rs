use std::collections::HashMap;

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
            // `Behavior::var_types` is populated by the earlier
            // `AttachBehaviors` pass (SIMPLIFICATION.md P3's lowering side
            // table) — already-known local-variable types this pass reuses
            // for overload-disambiguating argument-type inference (DLS-07)
            // instead of re-deriving its own locals tracking.
            let locals = behavior.var_types.clone();
            for stmt in &mut behavior.body {
                resolve_calls_in_stmt(stmt, &module.name, ctx, &locals)?;
            }
        }
    }
    Ok(())
}

fn resolve_calls_in_stmt(
    stmt: &mut BehaviorStmt,
    module_name: &str,
    ctx: &ElabContext,
    locals: &HashMap<String, ValueType>,
) -> Result<(), ElabError> {
    // First resolve any BehaviorStmt-specific logic (Diagnostic validation),
    // then delegate expression traversal to walk_exprs_mut + resolve_calls_in_expr.
    //
    // Diagnostic-statement validity (declared-language-surface T20,
    // DLS-19) now comes from the same `CallableRegistry` every other
    // system task resolves through — the former hardcoded
    // `valid_diagnostics` array (a second, disconnected source for the
    // same category, flagged in design.md) is gone; a name is valid iff
    // `headers/tasks.phdl` declares a matching `extern task $name`.
    if let BehaviorStmt::Diagnostic { sys, .. } = stmt {
        let task_name = format!("${sys}");
        if ctx.callables.candidates(&task_name).is_empty() {
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
                resolve_calls_in_stmt(s, module_name, ctx, locals)?;
            }
            if let Some(eb) = else_body {
                for s in &mut eb.stmts {
                    resolve_calls_in_stmt(s, module_name, ctx, locals)?;
                }
            }
        }
        BehaviorStmt::Match { arms, .. } => {
            for arm in arms {
                for s in &mut arm.body.stmts {
                    resolve_calls_in_stmt(s, module_name, ctx, locals)?;
                }
            }
        }
        BehaviorStmt::Event { body, .. } => {
            for s in &mut body.stmts {
                resolve_calls_in_stmt(s, module_name, ctx, locals)?;
            }
        }
        _ => {}
    }

    // Resolve expressions via walk_exprs_mut. We capture the first error
    // in a cell since the walk closure returns Walk, not Result.
    let mut err: Option<ElabError> = None;
    stmt.walk_exprs_mut(&mut |e| {
        if err.is_some() { return Walk::SkipChildren; }
        match resolve_calls_in_expr(e, module_name, ctx, locals) {
            Ok(()) => Walk::Continue,
            Err(e) => { err = Some(e); Walk::SkipChildren }
        }
    });
    if let Some(e) = err { return Err(e); }

    Ok(())
}

/// Declared-first call resolution for every `Expr::Call` (T11/T13). The
/// former bare-name cast rewrite (`real(x)`/`int(x)`/`bit(x)`/`Boolean(x)`/
/// `Quad(x)` → a synthetic `Expr::Cast` node) lived here until T17 deleted
/// it (SPEC P4-AC7) — casts are now ordinary `Type::from(x)` calls
/// (`Expr::Path` callees), resolved via `resolve_path_call` below through
/// the `extern impl` blocks in `headers/types.phdl`, exactly like any other
/// declared method. No bare identifier carries compiler-special meaning
/// after this deletion. This is a *transform* on the current `Expr` node —
/// the child recursion is done by the caller via `walk_exprs_mut`. It only
/// needs to handle the `Call` variant.
fn resolve_calls_in_expr(
    expr: &mut Expr,
    module_name: &str,
    ctx: &ElabContext,
    locals: &HashMap<String, ValueType>,
) -> Result<(), ElabError> {
    if let Expr::Call(callee, args) = expr {
        match &**callee {
            Expr::Ident(name) => {
                let name = name.clone();
                resolve_declared_call(&name, args, module_name, ctx, locals)?;
            }
            Expr::Path(path) => {
                resolve_path_call(path, args, module_name, ctx, locals)?;
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
    locals: &HashMap<String, ValueType>,
) -> Result<(), ElabError> {
    let candidates = ctx.callables.candidates(name);
    if candidates.is_empty() {
        return Ok(());
    }
    let resolved: &dyn CallableDef = if candidates.len() == 1 {
        candidates[0].as_ref()
    } else {
        // Genuine overload disambiguation needs every argument's type
        // known — unlike the single-candidate case below, there is no
        // fallback: an unresolvable argument type is a fail-loud error
        // naming the call site (DLS-07).
        let arg_types = infer_arg_types(args, name, module_name, locals)?;
        ctx.callables.resolve(name, &arg_types)?
    };
    if resolved.is_extern() {
        // Signature validation is best-effort here: a single (non-
        // overloaded) candidate has nothing to disambiguate, so an
        // argument whose type can't be statically inferred at this
        // elaboration stage (e.g. a member access or nested call, as
        // opposed to a literal or a known local) simply skips validation
        // rather than failing loud — full argument type-checking for
        // arbitrary expressions is `elab/typecheck.rs`'s job, run later in
        // the pipeline. This keeps DLS-03's "extern does not weaken
        // argument checking" promise for the checkable cases (literals,
        // locals) without forcing every extern-fn call site in the stdlib
        // to have a statically-literal argument.
        if let Some(arg_types) = try_infer_arg_types(args, locals) {
            resolved.validate_call(&arg_types)?;
        }
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
    locals: &HashMap<String, ValueType>,
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
    if candidates.len() == 1 {
        // No ambiguity to resolve — signature validation is best-effort,
        // same reasoning as `resolve_declared_call`'s single-candidate path.
        if let Some(arg_types) = try_infer_arg_types(args, locals) {
            candidates[0].validate_call(&arg_types)?;
        }
        return Ok(());
    }
    let arg_types = infer_arg_types(args, &qualified, module_name, locals)?;
    let resolved = ctx.impl_methods.resolve(type_name, method_name, &arg_types)?;
    resolved.validate_call(&arg_types)?;
    Ok(())
}

/// Argument-type inference for overload disambiguation (DLS-07), required
/// (fail-loud) variant: a call whose argument type cannot be determined
/// here fails loud rather than silently guessing a candidate — used only
/// where disambiguation is actually needed (more than one registered
/// candidate). See `try_infer_arg_types` for the best-effort variant used
/// where a single candidate leaves nothing to disambiguate.
fn infer_arg_types(
    args: &[Expr],
    name: &str,
    module_name: &str,
    locals: &HashMap<String, ValueType>,
) -> Result<Vec<ValueType>, ElabError> {
    args.iter()
        .map(|a| {
            infer_arg_type(a, locals).ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "cannot resolve overload of `{name}` in module `{module_name}`: \
                     argument type is not statically known at elaboration time"
                )))
            })
        })
        .collect()
}

/// Best-effort variant of `infer_arg_types`: `None` (rather than an error)
/// when any argument's type can't be determined at this elaboration stage
/// — used for single-candidate `extern` signature validation, where a
/// skipped check just defers to `elab/typecheck.rs`'s later, fuller
/// analysis instead of forcing every call site to have statically-known
/// argument types.
fn try_infer_arg_types(args: &[Expr], locals: &HashMap<String, ValueType>) -> Option<Vec<ValueType>> {
    args.iter().map(|a| infer_arg_type(a, locals)).collect()
}

/// Type inference for a single call argument, sufficient for overload
/// disambiguation (DLS-07): literals infer their own type; a bare
/// identifier resolves through the enclosing behavior's already-known
/// local-variable types (`Behavior::var_types`, populated by the earlier
/// `AttachBehaviors` pass) — module elaboration resolves argument types
/// concretely before call resolution runs (design.md's "Overload
/// resolution" component), so a local var's declared type is exactly as
/// legitimate a source as a literal here. Anything else (member access,
/// nested calls, arithmetic) is not attempted — that's `elab/typecheck.rs`'s
/// job, run later in the pipeline.
fn infer_arg_type(expr: &Expr, locals: &HashMap<String, ValueType>) -> Option<ValueType> {
    match expr {
        Expr::Literal(Literal::Int(_)) => Some(ValueType::Integer),
        Expr::Literal(Literal::Real(_)) => Some(ValueType::Real),
        Expr::Literal(Literal::Bool(_)) => Some(ValueType::Boolean),
        Expr::Literal(Literal::Quad(_)) => Some(ValueType::Quad),
        Expr::Literal(Literal::String(_)) => Some(ValueType::Str),
        Expr::Ident(name) => locals.get(name).cloned(),
        _ => None,
    }
}
