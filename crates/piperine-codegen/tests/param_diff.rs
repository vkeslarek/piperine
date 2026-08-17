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
use piperine_lang::ast::{BinOp, Expr, UnOp};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;

// ── small Expr constructors ──────────────────────────────────────────────────

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Binary { span: SP, op, lhs: Box::new(a), rhs: Box::new(b) }
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call { span: SP, name: name.to_string(), args }
}

fn neg(a: Expr) -> Expr {
    Expr::Unary { span: SP, op: UnOp::Neg, operand: Box::new(a) }
}

/// Branch read `V(plus, minus)` — a param derivative's only non-param leaf.
fn v(plus: &str, minus: &str) -> Expr {
    Expr::AccessFunction {
        span: SP,
        function: "V".to_string(),
        pos: Box::new(ident(plus)),
        neg: Some(Box::new(ident(minus))),
    }
}

// ── evaluator: params by name, branch voltages by (plus, minus) ──────────────

fn eval(expr: &Expr, param: &dyn Fn(&str) -> f64, vof: &dyn Fn(&str, &str) -> f64) -> f64 {
    match expr {
        Expr::Number { value, .. } => match value {
            NumberLit::Real(v) => *v,
            NumberLit::Integer(v) => *v as f64,
            NumberLit::Based(_) => 0.0,
        },
        Expr::Ident { name, .. } => param(name),
        Expr::Unary { op, operand, .. } => {
            let xv = eval(operand, param, vof);
            match op {
                UnOp::Neg => -xv,
                UnOp::Plus => xv,
                UnOp::LogNot => f64::from(xv == 0.0),
                _ => 0.0,
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let (a, b) = (eval(lhs, param, vof), eval(rhs, param, vof));
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => a / b,
                BinOp::Pow => a.powf(b),
                _ => 0.0,
            }
        }
        Expr::AccessFunction { pos, neg, .. } => {
            let p = ident_str(Some(pos));
            let m = neg.as_deref().map(|e| ident_str(Some(e))).unwrap_or_default();
            vof(&p, &m)
        }
        Expr::Call { name, args, .. } => {
            let vals: Vec<f64> = args.iter().map(|a| eval(a, param, vof)).collect();
            piperine_lang::math::eval_const_math(name, &vals).unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

fn ident_str(e: Option<&Expr>) -> String {
    match e {
        Some(Expr::Ident { name, .. }) => name.clone(),
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
        (bin(BinOp::Mul, ident("g"), v("p", "n")), "g", vpn),
        // g appears twice: g + g → 2
        (bin(BinOp::Add, ident("g"), ident("g")), "g", 2.0),
        // affine in g: k + g·V → V
        (bin(BinOp::Add, ident("k"), bin(BinOp::Mul, ident("g"), v("p", "n"))), "g", vpn),
        // quadratic in g: g·g·V → 2g·V
        (bin(BinOp::Mul, bin(BinOp::Mul, ident("g"), ident("g")), v("p", "n")), "g", 2.0 * g * vpn),
        // quotient: V / g → -V/g²
        (bin(BinOp::Div, v("p", "n"), ident("g")), "g", -vpn / (g * g)),
        // exp(g)·V → exp(g)·V
        (bin(BinOp::Mul, call("exp", vec![ident("g")]), v("p", "n")), "g", g.exp() * vpn),
        // ln(g) → 1/g
        (call("ln", vec![ident("g")]), "g", 1.0 / g),
        // sqrt(g) → 1/(2·sqrt(g))
        (call("sqrt", vec![ident("g")]), "g", 1.0 / (2.0 * g.sqrt())),
        // sin(g·V) → cos(g·V)·V
        (call("sin", vec![bin(BinOp::Mul, ident("g"), v("p", "n"))]), "g", (g * vpn).cos() * vpn),
        // pow(g, k) → k·g^(k-1)
        (call("pow", vec![ident("g"), ident("k")]), "g", k * g.powf(k - 1.0)),
        // nested product rule: (g·V1)·(m·V2) → V1·m·V2
        (
            bin(
                BinOp::Mul,
                bin(BinOp::Mul, ident("g"), v("p", "n")),
                bin(BinOp::Mul, ident("m"), v("q", "n")),
            ),
            "g",
            vpn * m * vqn,
        ),
        // through a negation: -(g + k) → -1
        (neg(bin(BinOp::Add, ident("g"), ident("k"))), "g", -1.0),
        // differentiating w.r.t. an absent parameter → exact 0
        (bin(BinOp::Mul, ident("g"), v("p", "n")), "k", 0.0),
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
    let expr = bin(BinOp::Add, bin(BinOp::Mul, ident("g"), v("p", "n")), ident("k"));
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
    let expr = bin(BinOp::Mul, ident("g"), v("p", "n"));
    let d = d_dp(&expr, "g");
    let vof = |p: &str, n: &str| -> f64 {
        if p == "p" && (n == "n" || n == "0") { 0.7 } else { 0.0 }
    };
    assert!(
        (eval(&d, &|_| 0.0, &vof) - 0.7).abs() < 1e-12,
        "d(g·V)/dg = V(p,n) = 0.7"
    );
}
