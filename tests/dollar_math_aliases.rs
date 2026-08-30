//! LRM-COVERS: 9.14
//!
//! lrm-conformance-wave3 (Phase 6, T37): §9.14 "Math system functions" —
//! "All of these functions, except $clog2, are aliases of the analog
//! math operators described in 4.3.1" (`$clog2` itself is covered by
//! `crates/piperine-lang/tests/dollar_clog2.rs`). Before this task
//! `emit_analog_syscall`'s catch-all rejected every `$`-prefixed math
//! function name (`$exp`, `$ln`, `$sqrt`, `$pow`, …) in an analog
//! expression as an "unsupported syscall" — only the bare, no-`$`
//! spelling (`exp(x)`, an `Expr::Call`) reached the JIT's math-builtin
//! path. `emit_analog_syscall` now falls back to the same
//! `math::math_fn` table `Expr::Call` already uses whenever the stripped
//! name matches a known math builtin.
//!
//! Compiled-behavior proof (not elaboration-only, per the Test Coverage
//! Matrix's codegen row): a DC operating point where `$exp`, `$pow`, and
//! `$sqrt` are used directly in an analog contribution, each checked
//! against Rust's own `f64` implementation of the same function.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

fn op_v_a(expr: &str) -> f64 {
    let src = format!(
        "module dut (inout electrical a);\nanalog I(a, gnd) <+ V(a) - ({expr});\nendmodule\n\
         module Top ();\nelectrical a;\ndut d1 (a);\nendmodule\n"
    );
    let design = piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    op.v("a").expect("v(a)")
}

/// `I(a,gnd) <+ V(a) - $exp(1.0);` forces `V(a) = $exp(1.0)` at DC — the
/// dollar-prefixed spelling of `exp(1.0)` (LRM Table 4-14: `$exp`
/// aliases the analog `exp` operator).
#[test]
fn dollar_exp_aliases_the_analog_exp_operator() {
    let v = op_v_a("$exp(1.0)");
    assert!((v - std::f64::consts::E).abs() < 1e-9, "$exp(1.0) must equal e, got {v}");
}

/// `$pow(2.0, 3.0)` aliases the analog `pow` operator. (A small magnitude
/// is deliberate: the default solver convergence tolerance is not tight
/// enough to pin a DC operating point in the hundreds/thousands to
/// `1e-9` — an unrelated, pre-existing solver characteristic confirmed
/// with a bare numeric-literal RHS, not something this task's `$`-alias
/// dispatch introduces.)
#[test]
fn dollar_pow_aliases_the_analog_pow_operator() {
    let v = op_v_a("$pow(2.0, 3.0)");
    assert!((v - 8.0).abs() < 1e-6, "$pow(2.0, 3.0) must equal 8, got {v}");
}

/// `$sqrt(2.0)` aliases the analog `sqrt` operator.
#[test]
fn dollar_sqrt_aliases_the_analog_sqrt_operator() {
    let v = op_v_a("$sqrt(2.0)");
    assert!((v - std::f64::consts::SQRT_2).abs() < 1e-9, "$sqrt(2.0) must equal sqrt(2), got {v}");
}
