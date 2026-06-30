//! Symbolic differentiation over PHDL [`Expr`].
//!
//! `diff(expr, wrt)` differentiates `expr` with respect to the branch voltage
//! named by `wrt`.  The wrt key is the canonical branch key `"V(plus,minus)"`.
//!
//! The result is a new `Expr` that can be passed back through `emit_phdl_expr`.

use crate::parse::ast::{BinaryOp, Expr, Literal, UnaryOp};

/// Differentiate `expr` with respect to the branch voltage identified by `wrt`.
///
/// `wrt` must be a canonical branch key of the form `"V(a,b)"` where `a` and
/// `b` are port names.  Everything else — literals, identifiers (params), and
/// unrecognised calls — has derivative 0.
pub fn diff(expr: &Expr, wrt: &str) -> Expr {
    match expr {
        Expr::Literal(_) => lit(0.0),

        // Named identifier: either a param (constant w.r.t. voltages → 0)
        // or an in-scope wire/variable — treat as 0 for now.
        Expr::Ident(_) | Expr::Path(_) => lit(0.0),

        Expr::Unary(UnaryOp::Neg, inner) => neg(diff(inner, wrt)),
        Expr::Unary(UnaryOp::Not, _) => lit(0.0),

        Expr::Binary(lhs, op, rhs) => {
            let du = diff(lhs, wrt);
            let dv = diff(rhs, wrt);
            match op {
                BinaryOp::Add => add(du, dv),
                BinaryOp::Sub => sub(du, dv),
                // (u*v)' = u'v + uv'
                BinaryOp::Mul => add(mul(du, *rhs.clone()), mul(*lhs.clone(), dv)),
                // (u/v)' = (u'v − uv') / v²
                BinaryOp::Div => div(
                    sub(mul(du, *rhs.clone()), mul(*lhs.clone(), dv)),
                    mul(*rhs.clone(), *rhs.clone()),
                ),
                // Remainder and comparisons: 0 a.e.
                _ => lit(0.0),
            }
        }

        // Function call: V(a,b), math functions, etc.
        Expr::Call(func, args) => diff_call(func, args, wrt),

        // SysCall (e.g. $display) — not differentiable
        Expr::SysCall(_, _) => lit(0.0),

        // Inline if — diff both branches
        Expr::If { cond, then_body, else_body } => {
            let dt = diff_block(then_body, wrt);
            let de = diff_block(else_body, wrt);
            Expr::If {
                cond: cond.clone(),
                then_body: single_expr_block(dt),
                else_body: single_expr_block(de),
            }
        }

        // Block — differentiate last expression (the value)
        Expr::Block(block) => diff_block(block, wrt),

        // Array/lambda/bundle — 0 (not meaningful in analog context)
        Expr::Array(_) | Expr::Lambda { .. } | Expr::BundleLit { .. }
        | Expr::Index(_, _) | Expr::Slice(_, _) | Expr::Field(_, _) => lit(0.0),
    }
}

fn diff_call(func: &Expr, args: &[Expr], wrt: &str) -> Expr {
    let fname = match func {
        Expr::Ident(n) => n.as_str(),
        _ => return lit(0.0),
    };

    // Branch voltage V(a, b) — check against wrt
    if fname == "V" {
        if let (Some(Expr::Ident(a)), Some(Expr::Ident(b))) = (args.first(), args.get(1)) {
            let key = branch_key(a, b);
            return if key == wrt { lit(1.0) } else { lit(0.0) };
        }
        return lit(0.0);
    }

    // Branch current I(a, b) — not a function of branch voltage directly
    if fname == "I" {
        return lit(0.0);
    }

    // ddt / idt — treat derivative as 0 (steady-state / DC approximation)
    if fname == "ddt" || fname == "idt" {
        return lit(0.0);
    }

    let u = args.first().cloned().unwrap_or(lit(0.0));
    let du = args.first().map(|a| diff(a, wrt)).unwrap_or(lit(0.0));

    match fname {
        // (exp u)' = exp(u) * u'
        "exp" => mul(call1("exp", u), du),
        // (ln u)' = u' / u
        "ln" | "log" => div(du, u),
        // (log10 u)' = u' / (u * ln10)
        "log10" => div(du, mul(u, lit(std::f64::consts::LN_10))),
        // (sqrt u)' = u' / (2*sqrt(u))
        "sqrt" => div(du, mul(lit(2.0), call1("sqrt", u))),
        // (sin u)' = cos(u) * u'
        "sin" => mul(call1("cos", u), du),
        // (cos u)' = -sin(u) * u'
        "cos" => mul(neg(call1("sin", u)), du),
        // (tan u)' = u' / cos²(u)
        "tan" => div(du, mul(call1("cos", u.clone()), call1("cos", u))),
        // (asin u)' = u' / sqrt(1 − u²)
        "asin" => div(du, call1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        // (acos u)' = −u' / sqrt(1 − u²)
        "acos" => div(neg(du), call1("sqrt", sub(lit(1.0), mul(u.clone(), u)))),
        // (atan u)' = u' / (1 + u²)
        "atan" => div(du, add(lit(1.0), mul(u.clone(), u))),
        // (|u|)' = sign(u) * u'
        "abs" => mul(
            Expr::If {
                cond: Box::new(Expr::Binary(
                    Box::new(u.clone()),
                    BinaryOp::Ge,
                    Box::new(lit(0.0)),
                )),
                then_body: single_expr_block(lit(1.0)),
                else_body: single_expr_block(lit(-1.0)),
            },
            du,
        ),
        // pow(u, v)' ≈ v * pow(u, v-1) * u'  (constant exponent common case)
        "pow" => {
            let v = args.get(1).cloned().unwrap_or(lit(1.0));
            mul(mul(v.clone(), call2("pow", u.clone(), sub(v, lit(1.0)))), du)
        }
        // floor/ceil/min/max — 0 a.e.
        "floor" | "ceil" | "min" | "max" | "fmin" | "fmax" => lit(0.0),
        // Unknown or non-differentiable: 0
        _ => lit(0.0),
    }
}

// ── Branch key ────────────────────────────────────────────────────────────────

/// Canonical key for branch voltage V(plus, minus).
pub fn branch_key(plus: &str, minus: &str) -> String {
    format!("V({plus},{minus})")
}

// ── Smart constructors with constant folding ──────────────────────────────────

pub fn lit(v: f64) -> Expr {
    Expr::Literal(Literal::Real(v))
}

fn is_lit(e: &Expr, v: f64) -> bool {
    matches!(e, Expr::Literal(Literal::Real(x)) if *x == v)
}

pub fn add(a: Expr, b: Expr) -> Expr {
    if is_lit(&a, 0.0) { return b; }
    if is_lit(&b, 0.0) { return a; }
    Expr::Binary(Box::new(a), BinaryOp::Add, Box::new(b))
}

pub fn sub(a: Expr, b: Expr) -> Expr {
    if is_lit(&b, 0.0) { return a; }
    Expr::Binary(Box::new(a), BinaryOp::Sub, Box::new(b))
}

pub fn mul(a: Expr, b: Expr) -> Expr {
    if is_lit(&a, 0.0) || is_lit(&b, 0.0) { return lit(0.0); }
    if is_lit(&a, 1.0) { return b; }
    if is_lit(&b, 1.0) { return a; }
    Expr::Binary(Box::new(a), BinaryOp::Mul, Box::new(b))
}

pub fn div(a: Expr, b: Expr) -> Expr {
    Expr::Binary(Box::new(a), BinaryOp::Div, Box::new(b))
}

pub fn neg(a: Expr) -> Expr {
    if let Expr::Literal(Literal::Real(v)) = &a { return lit(-v); }
    Expr::Unary(UnaryOp::Neg, Box::new(a))
}

fn call1(name: &str, a: Expr) -> Expr {
    Expr::Call(Box::new(Expr::Ident(name.to_string())), vec![a])
}

fn call2(name: &str, a: Expr, b: Expr) -> Expr {
    Expr::Call(Box::new(Expr::Ident(name.to_string())), vec![a, b])
}

// ── Block helpers ─────────────────────────────────────────────────────────────

use crate::parse::ast::{Block, Stmt};

fn diff_block(block: &Block, wrt: &str) -> Expr {
    // Block value = explicit trailing expr, or last Stmt::Return/Stmt::Expr.
    if let Some(e) = &block.expr {
        return diff(e, wrt);
    }
    match block.stmts.last() {
        Some(Stmt::Expr(e)) | Some(Stmt::Return(e)) => diff(e, wrt),
        _ => lit(0.0),
    }
}

fn single_expr_block(e: Expr) -> Block {
    Block { stmts: vec![], expr: Some(Box::new(e)) }
}

// ── Branch-collection helper (public for analog.rs) ──────────────────────────

/// Collect all unique `V(a,b)` branch keys appearing in `expr`.
pub fn collect_branches(expr: &Expr, out: &mut Vec<(String, String)>) {
    match expr {
        Expr::Call(func, args) => {
            if let Expr::Ident(fname) = func.as_ref() {
                if fname == "V" {
                    if let (Some(Expr::Ident(a)), Some(Expr::Ident(b))) =
                        (args.first(), args.get(1))
                    {
                        let pair = (a.clone(), b.clone());
                        if !out.contains(&pair) {
                            out.push(pair);
                        }
                    }
                }
            }
            for arg in args {
                collect_branches(arg, out);
            }
        }
        Expr::Binary(lhs, _, rhs) => {
            collect_branches(lhs, out);
            collect_branches(rhs, out);
        }
        Expr::Unary(_, inner) => collect_branches(inner, out),
        Expr::If { then_body, else_body, .. } => {
            collect_branches_block(then_body, out);
            collect_branches_block(else_body, out);
        }
        Expr::Block(b) => collect_branches_block(b, out),
        _ => {}
    }
}

fn collect_branches_block(block: &Block, out: &mut Vec<(String, String)>) {
    for s in &block.stmts { collect_branches_stmt(s, out); }
    if let Some(e) = &block.expr { collect_branches(e, out); }
}

fn collect_branches_stmt(stmt: &Stmt, out: &mut Vec<(String, String)>) {
    match stmt {
        Stmt::Expr(e) | Stmt::Return(e) => collect_branches(e, out),
        Stmt::Bind { dest, src, .. } => {
            collect_branches(dest, out);
            collect_branches(src, out);
        }
        Stmt::If { cond, then_body, else_body } => {
            collect_branches(cond, out);
            collect_branches_block(then_body, out);
            if let Some(eb) = else_body { collect_branches_block(eb, out); }
        }
        Stmt::VarDecl { default: Some(e), .. } => collect_branches(e, out),
        _ => {}
    }
}
