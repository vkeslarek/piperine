//! LRMC-C1-11 (§4.2.1.1): implicit real->integer conversion rounds to the
//! nearest integer, ties away from zero, instead of erroring on a
//! fractional value. Exercised through `var`'s own `[msb:lsb]` array-bound
//! context (`elab::const_eval::ConstEnv::eval_int`), the same "array
//! bound" context the LRM clause itself names as the motivating example.

use piperine_lang::pom::ValueType;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

fn array_width(src: &str) -> u64 {
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    match &m.vars()[0].ty {
        ValueType::Array(_, width) => *width,
        other => panic!("expected an array-typed var, got {other:?}"),
    }
}

#[test]
fn positive_fractional_bound_rounds_up_35_7_to_36() {
    // msb=35.7 -> 36, lsb=0 -> 0, width = (36-0)+1 = 37.
    let width = array_width("module m (); integer arr[35.7:0]; endmodule");
    assert_eq!(width, 37, "35.7 must round to 36, not error or truncate to 35");
}

#[test]
fn negative_half_bound_rounds_away_from_zero_neg1_5_to_neg2() {
    // msb=0 -> 0, lsb=-1.5 -> -2 (ties away from zero, not toward zero
    // which would give -1), width = (0-(-2))+1 = 3.
    let width = array_width("module m (); integer arr[0:-1.5]; endmodule");
    assert_eq!(width, 3, "-1.5 must round to -2 (away from zero), not -1");
}

#[test]
fn positive_half_bound_rounds_away_from_zero_2_5_to_3() {
    // msb=2.5 -> 3 (ties away from zero -- round-to-even would give 2,
    // which this asserts against), lsb=0 -> 0, width = (3-0)+1 = 4.
    let width = array_width("module m (); integer arr[2.5:0]; endmodule");
    assert_eq!(width, 4, "2.5 must round to 3 (away from zero, not round-to-even's 2)");
}

#[test]
fn negative_round_result_where_a_natural_is_required_still_fails_loud() {
    // A port-connection bus-tap index (`elab::const_eval::ConstEnv::
    // eval_nat`, `elab/lower/vams.rs::expr_to_net_ref`) requires a
    // Natural -- a negative index, rounded or not, still fails loud. The
    // fix narrows the "fractional value errors" case (LRMC-C1-11), it
    // does not loosen negative-value rejection.
    let src = r#"
        module leaf (inout electrical p);
        endmodule
        module top ();
            electrical bus [3:0];
            leaf l0 (bus[-1.5]);
        endmodule
    "#;
    let _ = elab_err(src);
}
