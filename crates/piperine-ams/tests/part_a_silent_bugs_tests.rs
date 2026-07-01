//! Regression tests for Part A of `docs/GAPS.md` — silent wrong-code bugs.
//!
//! Each test asserts that the *old* silent-zero / silent-wrong behavior is
//! gone. They are negative-assertion tests: a regression (e.g. reintroducing
//! a `f64const(0.0)` fallback) trips the test loudly rather than silently
//! producing wrong numerics.
//!
//! The tests build small IR programs through `ams_to_ir` (the AMS frontend
//! is convenient for concise Verilog-A fixtures) and assert on the
//! `CodegenError` shape returned by `ir_analog_to_device`.

use piperine_ams::Document;
use piperine_codegen::{ir_analog_to_device, SimCtx};

fn ir(src: &str) -> piperine_codegen::IrProgram {
    let doc = Document::parse(src).expect("VA parses");
    piperine_ams::ams_to_ir(&doc)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

// ── A.1 — BranchAccess "I" inside a contribution must be rejected ─────────────
//
// Old behavior: `I(a,b)` reads inside a contribution silently emitted 0.0
// (`ir_emit.rs:102-106`), so a VCCS/CCCS pattern compiled and ran with the
// sensed-current term dropped. After the fix, the codegen must reject with
// a clear error mentioning the flow read.

#[test]
fn a1_current_read_in_contribution_is_rejected_not_silently_zero() {
    let prog = ir(r#"
        module vccs(a, c, sense_p, sense_n);
            inout a, c, sense_p, sense_n;
            electrical a, c, sense_p, sense_n;
            parameter real gain = 1.0;
            parameter real r = 1.0e3;
            analog begin
                // CCCS-style: I(a,c) depends on a sensed current I(sense_p, sense_n).
                I(a, c) <+ V(a, c) / r + I(sense_p, sense_n) * gain;
            end
        endmodule
    "#);
    let err = ir_analog_to_device(&prog, "vccs")
        .err()
        .expect("vccs with flow read must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("flow") || msg.contains("i(") || msg.contains("unsupported"),
        "A.1: expected flow-read rejection, got: {msg}"
    );
}

// A.1 (positive case): `V(a,b)` inside a contribution is the supported form
// and must continue to compile after the fix.

#[test]
fn a1_voltage_read_in_contribution_still_works() {
    let prog = ir(r#"
        module res(a, c);
            inout a, c;
            electrical a, c;
            parameter real r = 1.0e3;
            analog begin
                I(a, c) <+ V(a, c) / r;
            end
        endmodule
    "#);
    ir_analog_to_device(&prog, "res").expect("V(a,c) in contrib still works");
}

// ── A.8 — Param/Var unresolved names silently read as 0 in analog JIT ─────────
//
// Old behavior: `ir_emit.rs:90-93` emitted `f64const(0.0)` for any
// `Param(name)` or `Var(name)` not found in the param array. A typo in
// a param name or an undeclared local var read as 0 silently. After the
// fix, `validate_ir_contrib` rejects unresolved names with a clear
// error.

#[test]
fn a8_unresolved_param_in_contribution_is_rejected_not_silently_zero() {
    let prog = ir(r#"
        module typo(a, c);
            inout a, c;
            electrical a, c;
            parameter real r = 1.0e3;
            analog begin
                I(a, c) <+ V(a, c) / r_typo;
            end
        endmodule
    "#);
    let err = ir_analog_to_device(&prog, "typo")
        .err()
        .expect("typo'd param must fail (GAPS §A.8)");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("r_typo") || msg.contains("unresolved") || msg.contains("unsupported"),
        "A.8: error should name the bad identifier, got: {msg}"
    );
}

#[test]
fn a8_known_param_in_contribution_still_works() {
    let prog = ir(r#"
        module res(a, c);
            inout a, c;
            electrical a, c;
            parameter real r = 1.0e3;
            analog begin
                I(a, c) <+ V(a, c) / r;
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "res").expect("known `r` still compiles");
    let params = [1.0e3_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    let expected = 0.5 / 1.0e3;
    assert!(
        (rhs[0] - expected).abs() < 1e-9,
        "A.8: residual should be V/r = {expected}, got {}",
        rhs[0]
    );
}

// ── A.9 — V(a,b) with unknown terminal names silently reads as 0 ─────────────
//
// Old behavior: `analog.rs:401, 405` emitted `f64const(0.0)` when plus or
// minus was not in `port_index`. A typo in a terminal name silently read
// 0 instead of erroring. After the fix, unknown terminal names are
// rejected with a clear error.

#[test]
fn a9_unknown_terminal_in_voltage_read_is_rejected_not_silently_zero() {
    let prog = ir(r#"
        module typo(a, c);
            inout a, c;
            electrical a, c;
            analog begin
                // `nonexistent` is not a port or wire of this module.
                I(a, c) <+ V(a, nonexistent);
            end
        endmodule
    "#);
    let err = ir_analog_to_device(&prog, "typo")
        .err()
        .expect("unknown terminal must fail (GAPS §A.9)");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("nonexistent") || msg.contains("terminal") || msg.contains("unknown"),
        "A.9: error should name the bad terminal, got: {msg}"
    );
}

#[test]
fn a9_known_terminal_in_voltage_read_still_works() {
    // Positive case: V(a,c) on the actual ports works.
    let prog = ir(r#"
        module res(a, c);
            inout a, c;
            electrical a, c;
            parameter real r = 1.0e3;
            analog begin
                I(a, c) <+ V(a, c) / r;
            end
        endmodule
    "#);
    ir_analog_to_device(&prog, "res").expect("V(a, c) still works");
}

// ── A.11 — AMS 4-state sized literals (`4'b1x0z`) silently become 0 ───────────
//
// Old behavior: `from_ams.rs:1126-1144` used `i64::from_str_radix` which
// fails on `x`/`z` digits; the parse silently returned 0. After the fix,
// these literals fail loud with a clear "4-state" error so users are
// not lied to.

#[test]
fn a11_4state_sized_literal_is_rejected_not_silently_zero() {
    use piperine_codegen::ir_analog_to_device;

    // The legacy bug: `from_ams.rs::parse_sized_lit` used
    // `i64::from_str_radix`, which rejects 4-state digits `x/X/z/Z/?`.
    // The unwrap_or(0) silently returned 0 for those literals. The
    // codegen would then compile a 4-state don't-care pattern as a
    // silent 0.0 contribution.
    //
    // After the fix: `ams_to_ir` panics (clearly visible) with a
    // "4-state sized literal" message rather than silently producing
    // Int(0). We catch the panic via `catch_unwind` to assert it is
    // loud and named, not silent.
    use std::panic;

    let src = r#"
        module four_state (a, c);
            inout a, c;
            electrical a, c;
            analog begin
                I(a, c) <+ 4'b1x0z;
            end
        endmodule
    "#;
    let doc = piperine_ams::Document::parse(src).expect("AMS parses");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let ir = piperine_ams::ams_to_ir(&doc);
        ir_analog_to_device(&ir, "four_state")
    }));

    match result {
        // The lowered IR is produced without error AND the device
        // compiles cleanly — that's the legacy silent-zero bug. FAIL.
        Ok(Ok(_)) => panic!(
            "A.11: 4-state sized literal was silently compiled — bug not fixed"
        ),
        // Error from the device compiler (preferred — proper Result).
        Ok(Err(e)) => {
            let msg = format!("{e:?}").to_lowercase();
            assert!(
                msg.contains("4-state") || msg.contains("x") || msg.contains("z") || msg.contains("unsupported"),
                "A.11: error should mention 4-state / x / z, got: {msg}"
            );
        }
        // Panic from ams_to_ir — acceptable today (it's loud). Assert
        // the panic message names the cause so it's actionable.
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains("4-state") || msg.contains("x") || msg.contains("z"),
                "A.11: panic should mention 4-state / x / z, got: {msg}"
            );
        }
    }
}
