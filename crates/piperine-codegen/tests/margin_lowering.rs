//! Margin lowering (dv-core T14, DVC-08): `a <= b` → `(b−a)/scale`,
//! `a >= b` → `(a−b)/scale`, the interval form `l <= a && a <= u` →
//! `min(u−a, a−l)/scale` (the grammar has no `in` operator — the `&&`
//! min-of-parts rule IS the `a in [l, u]` rule), `scale = 1` absent a
//! `tol`. The sign convention `m ≥ 0` ⟺ satisfied is asserted **on
//! values**, not by inspection: every form's lowered expression is
//! evaluated at satisfied, violated, and boundary points.
//!
//! **Require-core migrated** (constraints-on-vams): the `require`-only
//! tests run against the live producer. The two `var`/`target` tests
//! further down stay present-and-red — those statements are deferred with
//! dv-core's other statements (`.specs/features/constraints-on-vams/`
//! Out-of-Scope table) and their tests are the executable specification
//! for whoever rebuilds them; do not delete them.

use piperine_codegen::resolve::{MarginKind, lower_bodies};
use piperine_lang::ast::{BinOp, Expr, UnOp};
use piperine_lang::lexer::NumberLit;
use piperine_lang::{SourceMap, parse_and_elaborate};

const DISCIPLINES: &str = "`include \"disciplines.vams\"";

/// The fixture: params only, so every lowered margin is arithmetic over
/// named values and can be evaluated directly.
const PAR: &str = "\
module Par ();
    parameter real a = 2.0;
    parameter real b = 3.0;
    constraint begin
        require le_r : a <= b;
        require ge_r : b >= a;
        require band : 1.0 <= a && a <= b;
        require scaled : a <= b tol 2.0;
        require plain : b >= a;
    end
endmodule
";

/// Lower `src` and return the named module's resolved body.
fn lower(src: &str, module: &str) -> piperine_codegen::resolve::LoweredBody {
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborate");
    lower_bodies(&design).expect("lowering")[module].clone()
}

/// A tiny evaluator over the lowered margin expressions (literals, named
/// values from `env`, arithmetic, `min`) — the value-level assertion
/// vehicle for the sign convention.
fn eval(expr: &Expr, env: &[(String, f64)]) -> f64 {
    match expr {
        Expr::Number { value: NumberLit::Real(v), .. } => *v,
        Expr::Number { value: NumberLit::Integer(v), .. } => *v as f64,
        Expr::Ident { name, .. } => {
            env.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("env binds {name}")).1
        }
        Expr::Unary { op, operand, .. } => {
            let v = eval(operand, env);
            match op {
                UnOp::Neg => -v,
                other => panic!("unary {other:?} outside the margin forms"),
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let (l, r) = (eval(lhs, env), eval(rhs, env));
            match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => l / r,
                other => panic!("binary {other:?} outside the margin forms"),
            }
        }
        Expr::Call { name, args, .. } if name == "min" => {
            eval(&args[0], env).min(eval(&args[1], env))
        }
        other => panic!("{other:?} outside the margin forms"),
    }
}

/// DVC-08 AC1: `a <= b` lowers to `b − a` and `a >= b` to `a − b` (scale 1
/// — no `tol`, no division). Asserted structurally (the exact expression
/// shape) and on values at satisfied / violated / boundary points.
#[test]
fn le_and_ge_lower_to_signed_differences() {
    let body = lower(PAR, "Par");
    let margins = body.constraints.as_ref().expect("constraint margins lowered");

    let le = margins.iter().find(|m| m.label == "le_r").expect("le_r margin");
    assert_eq!(le.kind, MarginKind::Require);
    assert!(
        matches!(&le.expr, Expr::Binary { op: BinOp::Sub, lhs: l, rhs: r, .. }
            if matches!(l.as_ref(), Expr::Ident { name, .. } if name == "b")
            && matches!(r.as_ref(), Expr::Ident { name, .. } if name == "a")),
        "`a <= b` lowers to `b − a`: {:?}",
        le.expr
    );

    let ge = margins.iter().find(|m| m.label == "ge_r").expect("ge_r margin");
    assert!(
        matches!(&ge.expr, Expr::Binary { op: BinOp::Sub, lhs: l, rhs: r, .. }
            if matches!(l.as_ref(), Expr::Ident { name, .. } if name == "b")
            && matches!(r.as_ref(), Expr::Ident { name, .. } if name == "a")),
        "`b >= a` lowers to `b − a`: {:?}",
        ge.expr
    );

    // On values: m ≥ 0 ⟺ satisfied, at satisfied / violated / boundary.
    for (a, b, satisfied) in [(2.0, 3.0, true), (3.0, 2.0, false), (2.0, 2.0, true)] {
        let env = [("a".to_string(), a), ("b".to_string(), b)];
        let m = eval(&le.expr, &env);
        assert_eq!(m >= 0.0, satisfied, "le margin {m} at a={a}, b={b}");
        assert_eq!(m, b - a, "le margin value at a={a}, b={b}");
        let m = eval(&ge.expr, &env);
        assert_eq!(m >= 0.0, b >= a, "ge margin {m} at a={a}, b={b}");
    }
}

/// DVC-08 AC1: the interval form `l <= a && a <= u` lowers to
/// `min(a − l, u − a)` — the min of the parts' margins — and its sign
/// tracks satisfaction on values, inside, outside both ends, and on the
/// boundaries.
#[test]
fn the_interval_form_lowers_to_min_of_parts() {
    let body = lower(PAR, "Par");
    let margins = body.constraints.as_ref().expect("constraint margins lowered");
    let band = margins.iter().find(|m| m.label == "band").expect("band margin");

    let Expr::Call { name: combinator, args: parts, .. } = &band.expr else {
        panic!("`&&` lowers to a min call: {:?}", band.expr)
    };
    assert_eq!(combinator, "min", "the combinator is min");
    assert_eq!(parts.len(), 2, "two parts");
    assert!(
        matches!(&parts[0], Expr::Binary { op: BinOp::Sub, lhs: l, rhs: r, .. }
            if matches!(l.as_ref(), Expr::Ident { name, .. } if name == "a")
            && matches!(r.as_ref(), Expr::Number { value: NumberLit::Real(v), .. } if *v == 1.0)),
        "first part is `a − l`: {:?}",
        parts[0]
    );
    assert!(
        matches!(&parts[1], Expr::Binary { op: BinOp::Sub, lhs: l, rhs: r, .. }
            if matches!(l.as_ref(), Expr::Ident { name, .. } if name == "b")
            && matches!(r.as_ref(), Expr::Ident { name, .. } if name == "a")),
        "second part is `u − a`: {:?}",
        parts[1]
    );

    // On values: inside / below / above / on each boundary.
    for (a, satisfied) in [(2.0, true), (0.5, false), (4.0, false), (1.0, true), (3.0, true)] {
        let env = [("a".to_string(), a), ("b".to_string(), 3.0)];
        let m = eval(&band.expr, &env);
        assert_eq!(m >= 0.0, satisfied, "band margin {m} at a={a}");
        assert_eq!(m, (a - 1.0).min(3.0 - a), "band margin value at a={a}");
    }
}

/// DVC-08 AC1: a `tol`-normalized require divides by the scale; absent a
/// `tol` the scale is 1 — **no** division is emitted (no implicit
/// normalization). Asserted structurally and on values.
#[test]
fn tol_normalizes_and_its_absence_means_scale_one() {
    let body = lower(PAR, "Par");
    let margins = body.constraints.as_ref().expect("constraint margins lowered");

    let scaled = margins.iter().find(|m| m.label == "scaled").expect("scaled margin");
    assert_eq!(scaled.kind, MarginKind::Require);
    let Expr::Binary { op: BinOp::Div, lhs: diff, rhs: scale, .. } = &scaled.expr else {
        panic!("a tol require divides by the scale: {:?}", scaled.expr)
    };
    assert!(matches!(scale.as_ref(), Expr::Number { value: NumberLit::Real(v), .. } if *v == 2.0));
    assert!(matches!(diff.as_ref(), Expr::Binary { op: BinOp::Sub, .. }));

    let plain = margins.iter().find(|m| m.label == "plain").expect("plain margin");
    assert!(
        !matches!(&plain.expr, Expr::Binary { op: BinOp::Div, .. }),
        "absent a tol no division is emitted: {:?}",
        plain.expr
    );

    // On values: the normalized margin carries the scale, not just the sign.
    let env = [("a".to_string(), 1.0), ("b".to_string(), 2.0)];
    assert_eq!(eval(&scaled.expr, &env), 0.5, "(2−1)/2 — the scale is in the value");
    assert_eq!(eval(&plain.expr, &env), 1.0, "scale 1 — the raw difference");
}

/// DVC-08: a module with no constraint block lowers `constraints` to
/// `None` — nothing lowered, nothing allocated.
#[test]
fn no_constraint_block_lowers_to_none() {
    let body = lower(
        "\
module Bare (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
",
        "Bare",
    );
    assert!(body.constraints.is_none(), "no constraint block → None, not an empty vec");
}

/// Fail loud (AGENTS.md): a `require` predicate form with no margin
/// (anything but `<=`, `>=`, `&&`) is a lowering error naming the
/// constraint — never a silent `0.0` margin.
#[test]
fn an_unsupported_require_form_fails_loud() {
    let design = parse_and_elaborate(
        "\
module Par ();
    parameter real a = 1.0;
    parameter real b = 2.0;
    constraint begin
        require strict_lt : a < b;
    end
endmodule
",
        &SourceMap::dummy(),
    )
    .expect("elaborate");
    let err = lower_bodies(&design).expect_err("lowering must fail");
    let msg = err.to_string();
    assert!(msg.contains("strict_lt"), "the error names the constraint: {msg}");
    assert!(msg.contains("require predicate"), "the error names the form: {msg}");
}

// ── Present-and-red: the deferred `var`/`target` statements ────────────────
//
// `constraint var` (substitution into margins) and `target` (unlabeled,
// reduced-class, `target#<index>` naming) are deferred with dv-core's
// other statements. These two tests are their executable specification —
// they still use the PHDL grammar and fail at elaboration until the
// follow-up feature rebuilds them; present-and-red, never deleted.

/// DVC-08: a constraint `var` substitutes into the margins that read it
/// (authored order), so `via_var : ap1 <= b` lowers to `b − (a + 1)` —
/// asserted on values. **Blocked on the deferred `var` statement** — the
/// fixture fails at elaboration (the block accepts only `require`)
/// until that follow-up lands; present-and-red, never deleted.
#[test]
fn constraint_vars_substitute_into_margins() {
    let body = lower(
        "\
module Par ();
    parameter real a = 2.0;
    parameter real b = 3.0;
    constraint begin
        var ap1 = a + 1.0;
        require via_var : ap1 <= b;
    end
endmodule
",
        "Par",
    );
    let margins = body.constraints.as_ref().expect("constraint margins lowered");
    let via_var = margins.iter().find(|m| m.label == "via_var").expect("via_var margin");

    for (a, b, satisfied) in [(2.0, 4.0, true), (2.0, 3.0, true), (2.0, 2.5, false)] {
        let env = [("a".to_string(), a), ("b".to_string(), b)];
        let m = eval(&via_var.expr, &env);
        assert_eq!(m, b - (a + 1.0), "the var's `a + 1` is in the margin at a={a}, b={b}");
        assert_eq!(m >= 0.0, satisfied, "via_var margin {m} at a={a}, b={b}");
    }
}

/// DVC-08 / D7: a reduced-class target lowers like the rest — the class
/// travels with the margin (the kernel stage keeps only pointwise ones).
/// **Blocked on the deferred `target` statement** — the fixture fails at
/// elaboration until that follow-up lands; present-and-red, never
/// deleted.
#[test]
fn reduced_margins_lower_and_carry_their_class() {
    let body = lower(
        "\
module Amp (inout electrical out, inout electrical gnd);
    constraint begin
        target ac_unity_gain_freq(V(out, gnd)) >= 10e6 tol 1e6;
        require no_clip : V(out, gnd) <= 3.3;
    end
endmodule
",
        "Amp",
    );
    let margins = body.constraints.as_ref().expect("constraint margins lowered");
    assert_eq!(margins.len(), 2, "both statements lower");

    assert_eq!(margins[0].class, piperine_codegen::resolve::QuantityClass::Reduced, "the ugf target is reduced");
    assert_eq!(margins[0].analyses, vec!["ac".to_string()], "its analysis travels");
    assert_eq!(margins[0].kind, piperine_codegen::resolve::MarginKind::Target);
    assert_eq!(margins[1].class, piperine_codegen::resolve::QuantityClass::Pointwise, "the require is pointwise");
    assert_eq!(margins[1].label, "no_clip");
}
