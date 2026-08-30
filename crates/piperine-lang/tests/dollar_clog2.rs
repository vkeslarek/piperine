//! LRM-COVERS: 9.14
//!
//! lrm-conformance-wave3 (Phase 6, T37): §9.14 "Math system functions" —
//! "$clog2 shall return the ceiling of the log base 2 of the argument
//! (the log rounded up to an integer value)... defined more completely
//! in IEEE Std 1364-2005" — the one math system function the LRM calls
//! out as NOT an alias of an analog math operator (every other one —
//! `$exp`/`$ln`/`$sqrt`/`$pow`/… — is covered by
//! `crates/piperine-codegen/tests/dollar_math_aliases.rs`).
//!
//! Before this task `$clog2` was entirely unrecognized — no `MATH_FNS`
//! entry, no `Expr::SystemFunctionCall` arm anywhere in
//! `elab::const_eval::ConstEnv::eval`. `$clog2`'s own practical use is
//! elaboration-constant bit-width sizing (`parameter integer nbits =
//! $clog2(depth);`), so it is implemented there by exact integer bit-
//! counting — deliberately never `ceil(ln(n)/ln(2))`, which can round an
//! exact power of two up by one due to floating-point error and silently
//! mis-size a bus computed from it.

use piperine_lang::{parse_and_elaborate, SourceMap, Value};

fn clog2_of(n: i64) -> Value {
    let src = format!("module dut();\nparameter integer x = $clog2({n});\nendmodule\n");
    let d = parse_and_elaborate(&src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("$clog2({n}) must elaborate: {e:?}"));
    let m = d.module("dut").expect("dut elaborated");
    m.params().iter().find(|p| p.name == "x").expect("param x").default.clone().expect("x has a default")
}

/// IEEE 1364-2005's own worked values, including the two edge cases
/// (`$clog2(0) = 0`, `$clog2(1) = 0`) and several exact powers of two
/// (2, 4, 8, 1024) where a logarithm-based implementation would be most
/// at risk of off-by-one floating-point error.
#[test]
fn clog2_matches_the_1364_2005_worked_values() {
    let cases: &[(i64, i64)] =
        &[(0, 0), (1, 0), (2, 1), (3, 2), (4, 2), (5, 3), (7, 3), (8, 3), (9, 4), (1023, 10), (1024, 10), (1025, 11)];
    for &(n, expected) in cases {
        assert_eq!(clog2_of(n), Value::Int(expected), "$clog2({n}) must be {expected}");
    }
}

/// A negative argument has no defined ceiling-log2 — fails loud rather
/// than silently coercing to some integer.
#[test]
fn clog2_of_a_negative_argument_fails_loud() {
    let src = "module dut();\nparameter integer x = $clog2(-1);\nendmodule\n";
    let err = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a negative $clog2 argument must fail loud, not silently coerce");
    let msg = format!("{err:?}");
    assert!(!msg.is_empty());
}
