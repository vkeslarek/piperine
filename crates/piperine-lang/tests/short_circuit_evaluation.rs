//! LRM-COVERS: 4.2.3
//!
//! lrm-conformance-wave3 (Phase 6, T36): §4.2.3 "Expression evaluation
//! order" — "`&&`, `||`, and `?:` shall use short-circuit evaluation...
//! some of their operand expressions shall not be evaluated as long as
//! the expression contains no analog operators and their value is not
//! required to determine the final value of the operation. ... When short
//! circuiting occurs, any side effects or runtime errors that would have
//! occurred due to evaluation of the short-circuited operand expression
//! shall not occur."
//!
//! Before this task, `elab::const_eval::ConstEnv`'s `Expr::Ternary` arm
//! already short-circuited correctly (it evaluates only the taken
//! branch), but `eval_binary` evaluated **both** operands of `&&`/`||`
//! unconditionally before dispatching on the operator — so a
//! short-circuited right operand's runtime error (division by zero) still
//! surfaced, in direct violation of this clause. Fixed in `eval` (ahead of
//! `eval_binary`) by evaluating the right operand only when the left does
//! not already determine the result. Every test below fails on `main`-of-
//! this-branch (division by zero reported) before that fix and passes
//! after it — proving genuine short-circuit behavior, not merely that the
//! operators produce the right boolean value when both sides are safe to
//! evaluate.

use piperine_lang::{parse_and_elaborate, SourceMap};

/// A `parameter` default is the simplest place to observe elaboration-time
/// constant evaluation: the module still elaborates when the left operand
/// of `&&` already makes the result `false`, even though the right operand
/// alone would divide by zero.
#[test]
fn log_and_short_circuits_a_division_by_zero_on_the_right() {
    let src = "module dut();\nparameter integer flag = 0;\nparameter integer x = (flag == 1) && (1/0 == 0);\nendmodule\n";
    let d = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`&&` must short-circuit past the division by zero, got: {e:?}"));
    let m = d.module("dut").expect("dut elaborated");
    let x = m.params().iter().find(|p| p.name == "x").expect("param x");
    assert_eq!(x.default, Some(piperine_lang::Value::Bool(false)), "flag==1 is false, so x must be false");
}

/// The mirror case: `||`'s left operand is already `true`, so the
/// division-by-zero right operand must never be evaluated.
#[test]
fn log_or_short_circuits_a_division_by_zero_on_the_right() {
    let src = "module dut();\nparameter integer flag = 1;\nparameter integer x = (flag == 1) || (1/0 == 0);\nendmodule\n";
    let d = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`||` must short-circuit past the division by zero, got: {e:?}"));
    let m = d.module("dut").expect("dut elaborated");
    let x = m.params().iter().find(|p| p.name == "x").expect("param x");
    assert_eq!(x.default, Some(piperine_lang::Value::Bool(true)), "flag==1 is true, so x must be true");
}

/// `&&` must still evaluate (and require) the right operand when the left
/// does not already determine the result — short-circuiting is not "the
/// right side never runs."
#[test]
fn log_and_evaluates_the_right_operand_when_needed() {
    let src = "module dut();\nparameter integer flag = 1;\nparameter integer x = (flag == 1) && (2 == 3);\nendmodule\n";
    let d = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = d.module("dut").expect("dut elaborated");
    let x = m.params().iter().find(|p| p.name == "x").expect("param x");
    assert_eq!(x.default, Some(piperine_lang::Value::Bool(false)), "flag==1 is true but 2==3 is false, so x must be false");
}

/// `?:` already short-circuited before this task (unchanged, re-verified
/// here alongside the new `&&`/`||` behavior): the untaken branch's
/// division by zero must not surface.
#[test]
fn ternary_short_circuits_the_untaken_branch() {
    let src = "module dut();\nparameter integer flag = 0;\nparameter integer x = flag ? (1/0) : 42;\nendmodule\n";
    let d = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`?:` must short-circuit past the untaken branch's division by zero, got: {e:?}"));
    let m = d.module("dut").expect("dut elaborated");
    let x = m.params().iter().find(|p| p.name == "x").expect("param x");
    assert_eq!(x.default, Some(piperine_lang::Value::Int(42)), "flag is false, so x must take the else branch (42)");
}
