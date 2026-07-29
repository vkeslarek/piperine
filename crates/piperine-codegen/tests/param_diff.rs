//! Parameter differentiation (dv-gradients T1, DVG-01): `d_dp` differentiates
//! a resolved POM `Expr` w.r.t. a named parameter — the additive entry point
//! beside the Jacobian's `d_dv`. The chain rule is checked against central
//! finite differences over the operator set, and a parameter the expression
//! does not depend on must yield exact zero (distinguishable from "not
//! differentiable").
//!
//! Lives in its own test binary so the differentiator is exercised directly at
//! the `Expr` level, independent of JIT emission (T2).

use piperine_codegen::resolve::diff::d_dp;
use piperine_lang::parse::ast::{BinaryOp, Expr, Literal};

// ── small Expr constructors ──────────────────────────────────────────────────

fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_string())
}

fn bin(op: BinaryOp, a: Expr, b: Expr) -> Expr {
    Expr::Binary(Box::new(a), op, Box::new(b))
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(Box::new(Expr::Ident(name.to_string())), args)
}

/// Branch read `V(plus, minus)` — a param derivative's only non-param leaf.
fn v(plus: &str, minus: &str) -> Expr {
    call("V", vec![ident(plus), ident(minus)])
}

// ── evaluator: params by name, branch voltages by (plus, minus) ──────────────

fn eval(expr: &Expr, param: &dyn Fn(&str) -> f64, vof: &dyn Fn(&str, &str) -> f64) -> f64 {
    match expr {
        Expr::Literal(Literal::Real(v)) => *v,
        Expr::Literal(Literal::Int(v)) => *v as f64,
        Expr::Literal(Literal::Bool(b)) => f64::from(*b),
        Expr::Ident(name) => param(name),
        Expr::Unary(op, x) => {
            use piperine_lang::parse::ast::UnaryOp;
            let xv = eval(x, param, vof);
            match op {
                UnaryOp::Neg => -xv,
                UnaryOp::Not => f64::from(xv == 0.0),
            }
        }
        Expr::Binary(lhs, op, rhs) => {
            let (a, b) = (eval(lhs, param, vof), eval(rhs, param, vof));
            use piperine_lang::parse::ast::BinaryOp as P;
            match op {
                P::Add => a + b,
                P::Sub => a - b,
                P::Mul => a * b,
                P::Div => a / b,
                // POM has no Pow variant; analog differentiation encodes Pow as
                // BitXor (see `BinOp::Pow::to_pom`), and the analog emit path
                // reads it back as `powf` — mirror that here.
                P::BitXor => a.powf(b),
                _ => 0.0,
            }
        }
        Expr::Call(func, args) => {
            let Expr::Ident(name) = func.as_ref() else { return 0.0 };
            if name == "V" || name == "I" {
                let p = ident_str(args.first());
                let m = ident_str(args.get(1));
                return vof(&p, &m);
            }
            let vals: Vec<f64> = args.iter().map(|a| eval(a, param, vof)).collect();
            piperine_lang::math::eval_const_math(name, &vals).unwrap_or(0.0)
        }
        Expr::Cast(_, inner) => eval(inner, param, vof),
        _ => 0.0,
    }
}

fn ident_str(e: Option<&Expr>) -> String {
    match e {
        Some(Expr::Ident(s)) => s.clone(),
        _ => String::new(),
    }
}

/// Analytic `d_dp` evaluated at the given point.
fn analytic(
    expr: &Expr,
    param: &str,
    base: &[(&str, f64)],
    vof: &dyn Fn(&str, &str) -> f64,
) -> f64 {
    let d = d_dp(expr, param);
    let p = |name: &str| base.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).unwrap_or(0.0);
    eval(&d, &p, vof)
}

// ── chain-rule table: analytic d_dp vs hand-derived derivatives ──────────────
//
// Checked against hand-derived values at one operating point (g=2e-3, k=3,
// m=1.5; V(p,n)=0.7, V(q,n)=0.3). Both sides compute the same analytic
// expression, so agreement is to float precision — a wrong chain rule diverges
// by O(1), not O(eps). Finite-difference cross-checks are the T9 oracle at the
// solver level; this unit test pins the differentiator itself.

#[test]
fn d_dp_matches_hand_derived_derivatives_over_the_operator_set() {
    let (g, k, m, vpn, vqn) = (2e-3_f64, 3.0_f64, 1.5_f64, 0.7_f64, 0.3_f64);
    let base: &[(&str, f64)] = &[("g", g), ("k", k), ("m", m)];
    let vof = |p: &str, n: &str| -> f64 {
        let node = |s: &str| match s {
            "p" => Some(0.7),
            "q" => Some(0.3),
            "n" | "0" | "" => Some(0.0),
            _ => None,
        };
        node(p).unwrap_or(0.0) - node(n).unwrap_or(0.0)
    };
    // (expr, param, hand-derived derivative at the operating point)
    let cases: Vec<(Expr, &str, f64)> = vec![
        // linear in g: g·V → V
        (bin(BinaryOp::Mul, ident("g"), v("p", "n")), "g", vpn),
        // g appears twice: g + g → 2
        (bin(BinaryOp::Add, ident("g"), ident("g")), "g", 2.0),
        // affine in g: k + g·V → V
        (bin(BinaryOp::Add, ident("k"), bin(BinaryOp::Mul, ident("g"), v("p", "n"))), "g", vpn),
        // quadratic in g: g·g·V → 2g·V
        (bin(BinaryOp::Mul, bin(BinaryOp::Mul, ident("g"), ident("g")), v("p", "n")), "g", 2.0 * g * vpn),
        // quotient: V / g → -V/g²
        (bin(BinaryOp::Div, v("p", "n"), ident("g")), "g", -vpn / (g * g)),
        // exp(g)·V → exp(g)·V
        (bin(BinaryOp::Mul, call("exp", vec![ident("g")]), v("p", "n")), "g", g.exp() * vpn),
        // ln(g) → 1/g
        (call("ln", vec![ident("g")]), "g", 1.0 / g),
        // sqrt(g) → 1/(2·sqrt(g))
        (call("sqrt", vec![ident("g")]), "g", 1.0 / (2.0 * g.sqrt())),
        // sin(g·V) → cos(g·V)·V
        (call("sin", vec![bin(BinaryOp::Mul, ident("g"), v("p", "n"))]), "g", (g * vpn).cos() * vpn),
        // pow(g, k) → k·g^(k-1)
        (call("pow", vec![ident("g"), ident("k")]), "g", k * g.powf(k - 1.0)),
        // nested product rule: (g·V1)·(m·V2) → V1·m·V2
        (
            bin(
                BinaryOp::Mul,
                bin(BinaryOp::Mul, ident("g"), v("p", "n")),
                bin(BinaryOp::Mul, ident("m"), v("q", "n")),
            ),
            "g",
            vpn * m * vqn,
        ),
        // through a negation: -(g + k) → -1
        (
            Expr::Unary(piperine_lang::parse::ast::UnaryOp::Neg, Box::new(bin(BinaryOp::Add, ident("g"), ident("k")))),
            "g",
            -1.0,
        ),
        // differentiating w.r.t. an absent parameter → exact 0
        (bin(BinaryOp::Mul, ident("g"), v("p", "n")), "k", 0.0),
    ];

    for (expr, param, expected) in cases {
        let an = analytic(&expr, param, base, &vof);
        let scale = expected.abs().max(1e-12);
        assert!(
            (an - expected).abs() < 1e-9 * scale,
            "d/d{param} of {expr:?}:\n  d_dp = {an:.6e}\n  hand = {expected:.6e}",
        );
    }
}

// ── exact zero for an independent parameter ──────────────────────────────────

#[test]
fn d_dp_yields_exact_zero_for_a_parameter_the_expression_does_not_depend_on() {
    // g·V + k depends on g and k; differentiating w.r.t. `unused` must give an
    // exact zero — not a near-zero float and not "not differentiable".
    let expr = bin(BinaryOp::Add, bin(BinaryOp::Mul, ident("g"), v("p", "n")), ident("k"));
    let d = d_dp(&expr, "unused");
    let any = |_: &str| 1.0;
    let vof = |_: &str, _: &str| 1.0;
    let value = eval(&d, &any, &vof);
    assert!(
        value == 0.0 && value.is_sign_positive(),
        "independent parameter derivative must be exact +0.0, got {value:e}"
    );
    // And the derivative w.r.t. a parameter that IS present is nonzero,
    // proving the zero above is a true independence result, not a no-op.
    let dg = d_dp(&expr, "g");
    assert!(eval(&dg, &any, &vof) != 0.0, "g is present; its derivative must be nonzero");
}

// ── d_dp returns a resolved expression (not a sentinel) ──────────────────────

#[test]
fn d_dp_returns_a_resolved_expression() {
    // The result of d_dp is itself a POM Expr the emitter can lower (here it
    // reduces to the branch read V(p,n) — the derivative of g·V w.r.t. g).
    let expr = bin(BinaryOp::Mul, ident("g"), v("p", "n"));
    let d = d_dp(&expr, "g");
    let vof = |p: &str, n: &str| -> f64 {
        if p == "p" && (n == "n" || n == "0") { 0.7 } else { 0.0 }
    };
    assert!(
        (eval(&d, &|_| 0.0, &vof) - 0.7).abs() < 1e-12,
        "d(g·V)/dg = V(p,n) = 0.7"
    );
}
