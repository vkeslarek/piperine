//! Four-state (`x`/`z`/`?`) based-literal evaluation through
//! `elab::const_eval` — LRMC-C1-02. A based literal with four-state digits
//! must evaluate to a `Value::Logic`, never silently coerce to `0`; a
//! four-state value flowing into a context with no defined four-state
//! numeric semantics (array bound, bit width) must fail loud citing the
//! LRM clause, never truncate. Fixtures are LRM §2.6.1's own Example 2/4
//! literal forms.

use piperine_lang::pom::Value;
use piperine_lang::value::logic::LogicValue;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// §2.6.1 Example 4's own form: `16'hx` — an all-unknown hex literal.
#[test]
fn sixteen_bit_hex_x_evaluates_to_a_four_state_logic_value_not_zero() {
    let d = elab(
        r#"
        module m ();
            parameter x = 16'hx;
        endmodule
        "#,
    );
    let m = d.module("m").expect("m elaborated");
    let default = m.params()[0].default.clone().expect("has a default");
    // The old bug: `unwrap_or(0)` made this evaluate as an Int/Nat 0.
    assert_ne!(default, Value::Nat(0));
    assert_ne!(default, Value::Int(0));
    let Value::Logic(lv) = default else {
        panic!("expected Value::Logic — literal silently coerced away from four-state");
    };
    assert_eq!(lv.width(), 16);
    assert!(lv.has_unknown(), "16'hx must carry unknown bits, not read as 0");
    for i in 0..16 {
        assert_eq!(lv.bit(i), LogicValue::X, "every bit of 16'hx must be X");
    }
}

/// §2.6.1's `z` form: `12'hz` — an all-high-impedance literal.
#[test]
fn twelve_bit_hex_z_evaluates_to_a_four_state_logic_value_not_zero() {
    let d = elab(
        r#"
        module m ();
            parameter x = 12'hz;
        endmodule
        "#,
    );
    let m = d.module("m").expect("m elaborated");
    let default = m.params()[0].default.clone().expect("has a default");
    let Value::Logic(lv) = default else {
        panic!("expected Value::Logic, got {default:?}");
    };
    assert_eq!(lv.width(), 12);
    assert!(lv.has_unknown());
    for i in 0..12 {
        assert_eq!(lv.bit(i), LogicValue::Z, "every bit of 12'hz must be Z");
    }
}

/// A mixed binary literal (§2.6.1's per-digit binary expansion): only the
/// unknown bit is X, the rest are the concrete two-state digits.
#[test]
fn mixed_binary_literal_carries_per_bit_four_state() {
    let d = elab(
        r#"
        module m ();
            parameter x = 3'b01x;
        endmodule
        "#,
    );
    let m = d.module("m").expect("m elaborated");
    let default = m.params()[0].default.clone().expect("has a default");
    let Value::Logic(lv) = default else {
        panic!("expected Value::Logic, got {default:?}");
    };
    assert_eq!(lv.bit(0), LogicValue::X);
    assert_eq!(lv.bit(1), LogicValue::One);
    assert_eq!(lv.bit(2), LogicValue::Zero);
    assert!(lv.has_unknown());
}

/// A fully-determined (two-state) based literal is unaffected: it still
/// evaluates to a plain Nat, not a Logic value — no regression for the
/// common case.
#[test]
fn two_state_based_literal_still_evaluates_as_a_plain_integer() {
    let d = elab(
        r#"
        module m ();
            parameter x = 8'hFF;
        endmodule
        "#,
    );
    let m = d.module("m").expect("m elaborated");
    let default = m.params()[0].default.clone().expect("has a default");
    assert_eq!(default, Value::Nat(0xFF));
}

/// Spec Edge Case: a four-state value flowing into a context with no
/// defined four-state semantics (a bus range bound, §3.7) fails loud
/// citing the LRM clause — it must never silently truncate to a two-state
/// value (e.g. treating `x` as `0`, which would produce a bogus width).
#[test]
fn four_state_bus_range_bound_fails_loud_instead_of_truncating() {
    let msg = elab_err(
        r#"
        module m ();
            electrical bus[3'b01x:0];
        endmodule
        "#,
    );
    assert!(
        msg.contains("2.6.1") || msg.to_lowercase().contains("four-state"),
        "error must name the LRM clause or the four-state condition, got: {msg}"
    );
}

/// The removed silent-zero coercion: grep-asserted so a future edit can't
/// silently reintroduce `i64::from_str_radix(..).unwrap_or(0)` on the
/// based-literal parse path (the exact bug this task closes — an unrelated
/// `unwrap_or(0)` elsewhere in the file, e.g. `Pow`'s exponent conversion,
/// is not this task's concern and must not make this assertion fail).
#[test]
fn const_eval_source_no_longer_contains_the_based_literal_unwrap_or_zero_coercion() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/elab/const_eval.rs"
    ))
    .expect("const_eval.rs readable");
    assert!(
        !src.contains("from_str_radix(&digits, radix).unwrap_or(0)"),
        "const_eval.rs must not silently coerce a based literal to 0 on parse failure"
    );
    // Positive check: the fix path (LogicVec-based parsing) is present.
    assert!(
        src.contains("LogicVec::from_based_literal"),
        "based literals must route through LogicVec::from_based_literal"
    );
}
