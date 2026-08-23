//! LRM-COVERS: 4.3
//!
//! LRMW2-54 (`lrm-conformance-wave2`, T47): the audit's finding
//! ("ceil, atan2, hypot, asinh, acosh, atanh missing from the intrinsic
//! catalogue") does **not reproduce at HEAD** — `piperine-lang/src/math.rs`'s
//! `MATH_FNS` table already carries all six (`ceil`/`atan2`/`hypot`/
//! `asinh`/`acosh`/`atanh`, each with a `libm`-backed `extern "C"`
//! wrapper), and both the analog resolve path (`resolve/pom/expr.rs`'s
//! `resolve_call`) and the JIT emitter (`emit/analog_expr.rs`'s
//! `emit_analog_math_call`) already consult `math::math_fn(name)` for
//! any unrecognized call name. Per the Wave-2 standing rule ("if a task's
//! finding no longer reproduces, deliver the test that proves it doesn't,
//! not a fix"), this file is that proof: each function returns the LRM
//! Table 4-14/4-15 value for a known input, asserted through a JITted
//! device (a compiled `AnalogKernel`/`CircuitCompiler`, not just calling
//! the Rust wrapper directly) — Done-when's own bar — plus a wrong-arity
//! call failing loud by typed kind.
//!
//! Not fixed here (out of this task's stated scope, a separate,
//! pre-existing, broader gap affecting *every* math intrinsic, not just
//! this task's six): the Verilog-function-style `$`-prefixed spelling
//! (`$ceil(x)`, `$atan2(y,x)`, …, LRM Table 4-14/4-15's "Verilog function
//! style" column) is not wired at all — `emit_analog_syscall`'s catch-all
//! arm rejects any unrecognized `$name` with "syscall `$name` in an
//! analog expression", for `$ln`/`$exp`/every other math alias equally,
//! not specifically the six this task names.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// Compiles `V(out) <+ {expr};` and returns the DC operating-point value
/// of `out` — the JITted-device round trip Done-when asks for (not a
/// direct Rust-wrapper call).
fn eval_through_jit(expr: &str) -> f64 {
    let src = format!(
        "module dut (inout electrical out);
            analog V(out) <+ {expr};
        endmodule
        module top ();
            electrical gnd, o;
            dut d1 (o);
        endmodule"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture `{expr}` elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("`{expr}` builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let o = info.nets.get("o").expect("net `o`");
    dc.get_node(o).unwrap_or_else(|| panic!("V(o) for `{expr}`"))
}

const TOL: f64 = 1e-9;

/// LRM Table 4-14: `ceil(x)` — "Ceiling", equivalent C `ceil(x)`. Two
/// different inputs (a fraction and its negation) must give two different,
/// LRM-correct results — an inert/dropped `ceil` would pass neither, or
/// would pass only the trivial one.
#[test]
fn ceil_matches_table_4_14() {
    assert!((eval_through_jit("ceil(2.3)") - 2.3f64.ceil()).abs() < TOL);
    assert!((eval_through_jit("ceil(-2.3)") - (-2.3f64).ceil()).abs() < TOL);
    assert_ne!(2.3f64.ceil(), (-2.3f64).ceil(), "the two reference values must differ");
}

/// LRM Table 4-15: `atan2(y, x)` — "Arc-tangent of y/x", equivalent C
/// `atan2(y,x)`.
#[test]
fn atan2_matches_table_4_15() {
    assert!((eval_through_jit("atan2(1.0, 0.0)") - 1.0f64.atan2(0.0)).abs() < TOL);
    assert!((eval_through_jit("atan2(0.0, -1.0)") - 0.0f64.atan2(-1.0)).abs() < TOL);
    assert_ne!(1.0f64.atan2(0.0), 0.0f64.atan2(-1.0));
}

/// LRM Table 4-15: `hypot(x, y)` — equivalent C `hypot(x,y)`, `sqrt(x²+y²)`.
#[test]
fn hypot_matches_table_4_15() {
    assert!((eval_through_jit("hypot(3.0, 4.0)") - 5.0).abs() < TOL);
    assert!((eval_through_jit("hypot(6.0, 8.0)") - 10.0).abs() < TOL);
}

/// LRM Table 4-15: `asinh(x)` — "Arc-hyperbolic sine", equivalent C
/// `asinh(x)`, domain "All x".
#[test]
fn asinh_matches_table_4_15() {
    assert!((eval_through_jit("asinh(0.0)") - 0.0f64.asinh()).abs() < TOL);
    assert!((eval_through_jit("asinh(3.0)") - 3.0f64.asinh()).abs() < TOL);
    assert_ne!(0.0f64.asinh(), 3.0f64.asinh());
}

/// LRM Table 4-15: `acosh(x)` — "Arc-hyperbolic cosine", equivalent C
/// `acosh(x)`, domain "x >= 1".
#[test]
fn acosh_matches_table_4_15() {
    assert!((eval_through_jit("acosh(1.0)") - 1.0f64.acosh()).abs() < TOL);
    assert!((eval_through_jit("acosh(2.0)") - 2.0f64.acosh()).abs() < TOL);
    assert_ne!(1.0f64.acosh(), 2.0f64.acosh());
}

/// LRM Table 4-15: `atanh(x)` — "Arc-hyperbolic tangent", equivalent C
/// `atanh(x)`, domain "-1 < x < 1".
#[test]
fn atanh_matches_table_4_15() {
    assert!((eval_through_jit("atanh(0.0)") - 0.0f64.atanh()).abs() < TOL);
    assert!((eval_through_jit("atanh(0.5)") - 0.5f64.atanh()).abs() < TOL);
    assert_ne!(0.0f64.atanh(), 0.5f64.atanh());
}

/// Arity errors are loud (Done-when): `hypot` takes exactly 2 arguments —
/// a 3-argument call fails to build with a typed `CodegenError`, not a
/// silent truncation.
#[test]
fn wrong_arity_call_fails_loud_by_typed_kind() {
    use piperine_codegen::CodegenError;
    let src = "module dut (inout electrical out);
        analog V(out) <+ hypot(3.0, 4.0, 5.0);
    endmodule
    module top ();
        electrical gnd, o;
        dut d1 (o);
    endmodule";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let err = match compiler.build_circuit("top") {
        Ok(_) => panic!("a 3-arg hypot() call must fail to build"),
        Err(e) => e,
    };
    assert!(
        matches!(err, CodegenError::Invalid(ref msg) if msg.contains("hypot") && msg.contains("expects 2 args")),
        "expected a typed Invalid error naming the arity mismatch, got: {err:?}"
    );
}
