//! LRM-COVERS: 4.5.9
//! LRMW2-34 (`lrm-conformance-wave2`, T37): `slew(expr, max_pos_slew_rate
//! [, max_neg_slew_rate])`'s omitted third argument was silently lowered
//! to a bare `0.0` instead of the LRM's "opposite of max_pos_slew_rate"
//! default. Checks the lowering step in isolation (the resolved
//! `StateKind::Slew.fall` expression), complementing the full-transient
//! discriminating proof in root `tests/slew_symmetric_fall.rs`.

use piperine_codegen::resolve::{StateKind, StateVar};
use piperine_lang::ast::Expr;
use piperine_lang::lexer::NumberLit;

fn slew_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Slew { .. }).then(|| s.clone()))
        .expect("a slew state")
}

fn as_real(e: &Expr) -> f64 {
    match e {
        Expr::Number { value: NumberLit::Real(r), .. } => *r,
        Expr::Number { value: NumberLit::Integer(i), .. } => *i as f64,
        other => panic!("expected a literal number, got {other:?}"),
    }
}

/// §4.5.9: with no third argument, `fall` must be the same expression as
/// `rise` (which magnitude-wise, after `operators.rs`'s `.abs()`, gives
/// the LRM's "opposite of max_pos_slew_rate") — not a bare `0.0`.
#[test]
fn slew_with_one_rate_defaults_fall_to_rise_not_zero() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ slew(V(p, n), 3.5);\n\
        end\n\
        endmodule\n";
    let StateKind::Slew { rise, fall } = slew_state(src).kind else { unreachable!() };
    assert_eq!(as_real(&rise), 3.5);
    assert_eq!(as_real(&fall), 3.5, "an omitted third argument must default fall to rise, not 0.0");
}

/// §4.5.9: an explicit third argument still wins over the default.
#[test]
fn slew_with_two_rates_keeps_both_distinct() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ slew(V(p, n), 3.5, 1.2);\n\
        end\n\
        endmodule\n";
    let StateKind::Slew { rise, fall } = slew_state(src).kind else { unreachable!() };
    assert_eq!(as_real(&rise), 3.5);
    assert_eq!(as_real(&fall), 1.2, "an explicit fall argument must not be overridden by the default");
}
