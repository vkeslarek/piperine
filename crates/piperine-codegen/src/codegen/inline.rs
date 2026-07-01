//! GAPS §D.5 — User `fn` inlining at the call site.
//!
//! `IrFunction` tables are populated by both frontends but, until D.5,
//! were never read by codegen (the IR-SYSTEM §1.4 doc claimed otherwise).
//! `validate_ir_contrib` rejected any non-builtin call with
//! `CodegenError::Unsupported("call to non-builtin `<name>`")`.
//!
//! This module performs **alpha-substitution inlining** at IR time: a
//! call `f(a, b)` to a user-defined function `f(x, y) = body` is
//! rewritten to `body[x → a, y → b]` and the call is replaced with the
//! function's `Return(expr)` expression. Recursion is guarded by a
//! depth cap (the spec §7.1 forbids unbounded recursion in the
//! elaboration phase, and the depth cap is the backstop).
//!
//! ## What this fixes
//!
//! The Diode model in the spec (`is_sat * (exp(V/thermal_voltage(temp)) - 1)`)
//! uses a user `fn thermal_voltage(t) -> Real { 8.617e-5 * t }`. Without
//! D.5, the call to `thermal_voltage` is rejected at codegen. With D.5,
//! it is inlined to `8.617e-5 * temp` — a plain binary expression the
//! JIT can lower directly.
//!
//! ## Where to call
//!
//! Run [`inline_user_calls`] once on every contribution / digital
//! expression before [`validate_ir_contrib`]. The codegen entry points
//! (`ir_analog_to_device`, `ir_digital_to_interp`) call it automatically.

use std::collections::HashMap;

use crate::ir::{IrExpr, IrFunction, IrModule, IrProgram, IrStmt};

/// Hard depth cap on recursive inlining. Functions whose bodies
/// recursively reference themselves (illegal per spec §7.1, which
/// requires each call to reduce a const parameter) hit this cap and
/// produce a clear error rather than stack-overflowing.
const MAX_INLINE_DEPTH: u32 = 32;

/// Resolve a user call site to an inlined expression.
///
/// `prog` is the program (provides program-level functions),
/// `module` is the calling module (provides module-level functions —
/// searched first to shadow program-level ones with the same name).
/// Returns the rewritten expression (with all reachable user calls
/// inlined). Returns `Err(...)` if a call site is unresolvable, the
/// function body has no `Return`, or the depth cap is exceeded.
pub fn inline_user_calls(
    prog: &IrProgram,
    module: &IrModule,
    e: &IrExpr,
) -> Result<IrExpr, String> {
    inline_with_depth(prog, module, e, 0, &mut HashMap::new())
}

fn inline_with_depth(
    prog: &IrProgram,
    module: &IrModule,
    e: &IrExpr,
    depth: u32,
    visiting: &mut HashMap<String, ()>,
) -> Result<IrExpr, String> {
    if depth > MAX_INLINE_DEPTH {
        return Err(format!(
            "function inlining depth exceeded ({MAX_INLINE_DEPTH}); \
             is there a recursive user fn call? See docs/GAPS.md §D.5"
        ));
    }

    match e {
        IrExpr::Call(name, args) => {
            // Even for builtin math calls (e.g. `exp(user_fn(x))`), we
            // must recurse into the args so user calls nested in them
            // are inlined. Then we return the (inlined) call unchanged.
            let inlined_args: Vec<IrExpr> = args
                .iter()
                .map(|a| inline_with_depth(prog, module, a, depth, visiting))
                .collect::<Result<_, _>>()?;
            if is_builtin_math(name) {
                return Ok(IrExpr::Call(name.clone(), inlined_args));
            }

            // Look up the function: module-level first, then program-level.
            let func = find_fn(prog, module, name)
                .ok_or_else(|| format!("unknown function `{name}`"))?;

            // Cycle detection: re-entering `name` while already visiting it
            // means recursive fn. The depth cap catches this too, but the
            // cycle check gives a clearer error.
            if visiting.contains_key(name) {
                return Err(format!(
                    "recursive function `{name}` is not supported (spec §7.1)"
                ));
            }
            visiting.insert(name.clone(), ());

            // 1. Evaluate each argument (recursively inline nested calls).
            let inlined_args: Vec<IrExpr> = args
                .iter()
                .map(|a| inline_with_depth(prog, module, a, depth + 1, visiting))
                .collect::<Result<_, _>>()?;

            // 2. Build the alpha-substitution map: param name → arg expr.
            if func.params.len() != inlined_args.len() {
                visiting.remove(name);
                return Err(format!(
                    "function `{name}` expects {} args, got {}",
                    func.params.len(),
                    inlined_args.len()
                ));
            }
            let mut subst: HashMap<String, IrExpr> = HashMap::new();
            for (p, a) in func.params.iter().zip(inlined_args.iter()) {
                subst.insert(p.clone(), a.clone());
            }

            // 3. Find the function body's `Return(expr)` and inline it.
            let body_expr = extract_return(&func.body).ok_or_else(|| {
                format!("function `{name}` has no `Return(expr)` body")
            })?;

            // 4. Substitute params with args in the body expression, then
            //    recursively inline any nested user calls.
            let substituted = substitute(&body_expr, &subst);
            let inlined = inline_with_depth(prog, module, &substituted, depth + 1, visiting)?;

            visiting.remove(name);
            Ok(inlined)
        }
        // Recurse into sub-expressions of compound nodes.
        IrExpr::Unary(op, x) => {
            let x2 = inline_with_depth(prog, module, x, depth, visiting)?;
            Ok(IrExpr::Unary(*op, Box::new(x2)))
        }
        IrExpr::Binary(op, a, b) => {
            let a2 = inline_with_depth(prog, module, a, depth, visiting)?;
            let b2 = inline_with_depth(prog, module, b, depth, visiting)?;
            Ok(IrExpr::Binary(*op, Box::new(a2), Box::new(b2)))
        }
        IrExpr::Select(c, t, f) => {
            let c2 = inline_with_depth(prog, module, c, depth, visiting)?;
            let t2 = inline_with_depth(prog, module, t, depth, visiting)?;
            let f2 = inline_with_depth(prog, module, f, depth, visiting)?;
            Ok(IrExpr::Select(Box::new(c2), Box::new(t2), Box::new(f2)))
        }
        // Leaves: literal, param, var, branch access, state ref, sim query,
        // quad, string — nothing to inline.
        _ => Ok(e.clone()),
    }
}

/// Look up a function by name. Module-level shadows program-level.
fn find_fn<'a>(prog: &'a IrProgram, module: &'a IrModule, name: &str) -> Option<&'a IrFunction> {
    module
        .functions
        .iter()
        .find(|f| f.name == name)
        .or_else(|| prog.functions.iter().find(|f| f.name == name))
}

/// Extract the expression from `Return(Some(expr))`, or `None`.
fn extract_return(body: &[IrStmt]) -> Option<IrExpr> {
    for s in body {
        if let IrStmt::Return(Some(e)) = s {
            return Some(e.clone());
        }
    }
    None
}

/// Replace every `Param(name)` reference in `e` whose name is in `subst`
/// with the corresponding substituted expression. `Var`, `BranchAccess`,
/// `StateRef`, `Sim`, etc. are preserved.
fn substitute(e: &IrExpr, subst: &HashMap<String, IrExpr>) -> IrExpr {
    match e {
        IrExpr::Param(name) => subst.get(name).cloned().unwrap_or_else(|| e.clone()),
        IrExpr::Var(_) | IrExpr::BranchAccess { .. } | IrExpr::StateRef(_) | IrExpr::Sim(_)
        | IrExpr::Quad(_) | IrExpr::String(_) | IrExpr::Real(_) | IrExpr::Int(_)
        | IrExpr::Bool(_) => e.clone(),
        IrExpr::Unary(op, x) => IrExpr::Unary(*op, Box::new(substitute(x, subst))),
        IrExpr::Binary(op, a, b) => IrExpr::Binary(
            *op,
            Box::new(substitute(a, subst)),
            Box::new(substitute(b, subst)),
        ),
        IrExpr::Select(c, t, f) => IrExpr::Select(
            Box::new(substitute(c, subst)),
            Box::new(substitute(t, subst)),
            Box::new(substitute(f, subst)),
        ),
        IrExpr::Call(name, args) => {
            let new_args: Vec<IrExpr> = args.iter().map(|a| substitute(a, subst)).collect();
            IrExpr::Call(name.clone(), new_args)
        }
        // Other variants (Index, Slice, PartSelect, Concat, Array, BundleLit,
        // Lambda, …) are validated-out of contributions; substitute
        // structurally without recursing into typed slots that the inliner
        // won't see in practice.
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_with_fn(name: &str, params: Vec<&str>, body_expr: IrExpr) -> IrModule {
        IrModule {
            name: "m".into(),
            ports: Vec::new(),
            params: Vec::new(),
            wires: Vec::new(),
            branches: Vec::new(),
            events: Vec::new(),
            vars: Vec::new(),
            grounds: Vec::new(),
            instances: Vec::new(),
            connections: Vec::new(),
            continuous_assigns: Vec::new(),
            analog: None,
            digital: None,
            functions: vec![IrFunction {
                name: name.into(),
                params: params.into_iter().map(String::from).collect(),
                body: vec![IrStmt::Return(Some(body_expr))],
            }],
        }
    }

    fn empty_prog() -> IrProgram {
        IrProgram {
            source: "test".into(),
            modules: Vec::new(),
            functions: Vec::new(),
        }
    }

    #[test]
    fn inline_substitutes_call_with_body() {
        // f(x) = x * 2.0 ;  call: f(V)  →  V * 2.0
        let module = module_with_fn(
            "f",
            vec!["x"],
            IrExpr::Binary(
                crate::ir::IrBinOp::Mul,
                Box::new(IrExpr::Param("x".into())),
                Box::new(IrExpr::Real(2.0)),
            ),
        );
        let prog = empty_prog();
        let expr = IrExpr::Call(
            "f".into(),
            vec![IrExpr::Param("V".into())],
        );
        let inlined = inline_user_calls(&prog, &module, &expr).expect("inline");
        // Expected: Binary(Mul, Param("V"), Real(2.0))
        match inlined {
            IrExpr::Binary(crate::ir::IrBinOp::Mul, a, b) => {
                assert!(matches!(*a, IrExpr::Param(ref n) if n == "V"));
                assert!(matches!(*b, IrExpr::Real(v) if v == 2.0));
            }
            other => panic!("expected Mul, got {other:?}"),
        }
    }

    #[test]
    fn inline_unknown_fn_errors_loudly() {
        let module = module_with_fn("g", vec!["x"], IrExpr::Real(1.0));
        let prog = empty_prog();
        let expr = IrExpr::Call("missing".into(), vec![IrExpr::Real(1.0)]);
        let err = inline_user_calls(&prog, &module, &expr).unwrap_err();
        assert!(err.contains("missing"), "err should name fn: {err}");
    }

    #[test]
    fn inline_recursive_call_errors() {
        // f() = f() — illegal recursion
        let module = module_with_fn(
            "f",
            vec![],
            IrExpr::Call("f".into(), vec![]),
        );
        let prog = empty_prog();
        let expr = IrExpr::Call("f".into(), vec![]);
        let err = inline_user_calls(&prog, &module, &expr).unwrap_err();
        assert!(err.contains("recursive"), "err should mention recursion: {err}");
    }

    #[test]
    fn builtin_math_calls_pass_through() {
        // exp(...) must NOT be treated as a user call — it's a libm fn.
        let module = module_with_fn("ignored", vec![], IrExpr::Real(1.0));
        let prog = empty_prog();
        let expr = IrExpr::Call(
            "exp".into(),
            vec![IrExpr::Param("x".into())],
        );
        let inlined = inline_user_calls(&prog, &module, &expr).expect("inline");
        // Should pass through unchanged.
        assert!(matches!(inlined, IrExpr::Call(ref n, _) if n == "exp"));
    }
}

/// True if `name` is a built-in math function understood by the
/// Cranelift emitter. Mirrors [crate::codegen::expr::is_builtin_math]
/// so inline.rs does not need to import from expr.rs.
fn is_builtin_math(name: &str) -> bool {
    matches!(
        name,
        "exp" | "ln" | "log" | "log10" | "sqrt" | "abs" | "sin" | "cos" | "tan"
            | "asin" | "acos" | "atan" | "atan2" | "pow" | "min" | "max"
            | "floor" | "ceil" | "limexp"
    )
}