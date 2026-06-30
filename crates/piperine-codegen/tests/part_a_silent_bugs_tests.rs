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
use piperine_codegen::{ams_to_ir, ir_analog_to_device, SimCtx};

fn ir(src: &str) -> piperine_codegen::IrProgram {
    let doc = Document::parse(src).expect("VA parses");
    ams_to_ir(&doc)
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

// ── A.4 — Digital Pow/Shl/Shr/AShl/AShr silently become Add ────────────────────
//
// Old behavior: `ir_digital_to_interp.rs:154-156` mapped `Pow`, `Shl`, `Shr`,
// `AShl`, `AShr` to `BinaryOp::Add` in the digital path. A guard like
// `if (x << 4 == 0)` silently became `if (x + 4 == 0)`. After the fix, the
// codegen must reject these operators with a clear error. We exercise the
// IR → interpreter path through a hand-built IrProgram (the IR frontend is
// the one that lowers the wrong-op bug).

#[test]
fn a4_shift_in_digital_guard_is_rejected_not_silently_add() {
    use piperine_codegen::ir_digital_to_interp;
    use piperine_codegen::ir::{
        IrBinOp, IrExpr, IrModule, IrProgram, IrStmt, IrDigitalBody,
    };

    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };
    prog.modules.push(IrModule {
        name: "shift_fsm".into(),
        ports: Vec::new(),
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: Some(IrDigitalBody {
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_vars: Vec::new(),
            stmts: vec![IrStmt::If {
                cond: IrExpr::Binary(
                    IrBinOp::Shl,
                    Box::new(IrExpr::Param("x".into())),
                    Box::new(IrExpr::Int(4)),
                ),
                then_: vec![],
                else_: vec![],
                label: None,
            }],
        }),
        functions: Vec::new(),
    });

    let err = ir_digital_to_interp(&prog, "shift_fsm")
        .err()
        .expect("shift in digital guard must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("shl") || msg.contains("shift") || msg.contains("unsupported"),
        "A.4: expected shift rejection, got: {msg}"
    );
}

// through the JIT-compiled function pointers, and `Temperature`/`Abstime`
// read the live simulator state. This test asserts both: at T=300K the
// residual is non-zero (old behavior produced zero) and matches the
// kT/q formula.

// A.3 — `$vt` must compute kT/q from the simulation temperature, not the
// hardcoded 0.025852 V. We assert at two temperatures: T=300K matches the
// legacy constant (regression-safe), T=350K gives ~0.03016 V, which the
// hardcoded constant gets wrong.

#[test]
fn a2_dollar_temperature_does_not_silently_zero() {
    use piperine_codegen::{JitAnalogDevice, SimCtx};
    let prog = ir(r#"
        module tmod(a, c);
            inout a, c;
            electrical a, c;
            parameter real gain = 1.0;
            analog begin
                // Use $temperature as a multiplier — if it silently reads 0,
                // the residual is 0; if it reads T (in Kelvin), it's non-zero.
                I(a, c) <+ V(a, c) * $temperature * gain;
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "tmod").expect("tmod compiles");
    let params = [1.0_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::at_300k(), &mut rhs);
    let expected = 0.5 * 300.0 * 1.0;
    assert!(
        rhs[0].abs() > 1.0,
        "A.2: $temperature must not silently emit 0; got I={}",
        rhs[0]
    );
    assert!(close(rhs[0], expected), "A.2: I(a) = {}, want {}", rhs[0], expected);
}

#[test]
fn a3_dollar_vt_scales_with_temperature() {
    use piperine_codegen::{JitAnalogDevice, SimCtx};
    let prog = ir(r#"
        module diode_t(a, c);
            inout a, c;
            electrical a, c;
            parameter real is_sat = 1.0e-14;
            analog begin
                I(a, c) <+ is_sat * (exp(V(a, c) / $vt) - 1.0);
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "diode_t").expect("diode_t compiles");

    let params = [1.0e-14_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];

    // T=300K: legacy constant.
    rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::at_300k(), &mut rhs);
    let vt_300: f64 = 8.617333262e-5 * 300.0;
    let i_300: f64 = 1.0e-14_f64 * ((0.5_f64 / vt_300).exp() - 1.0);
    assert!(close(rhs[0], i_300), "A.3 T=300K: I={}, want {}", rhs[0], i_300);

    // T=350K: legacy hardcoded 0.025852 is wrong; correct value is
    // k * 350 / q ≈ 0.030161.
    rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::new(350.0), &mut rhs);
    let vt_350: f64 = 8.617333262e-5 * 350.0;
    let i_350: f64 = 1.0e-14_f64 * ((0.5_f64 / vt_350).exp() - 1.0);
    let legacy: f64 = 1.0e-14_f64 * ((0.5_f64 / 0.025852_f64).exp() - 1.0);
    assert!(
        close(rhs[0], i_350),
        "A.3 T=350K: I={}, want {} (legacy hardcoded vt={} would give {})",
        rhs[0],
        i_350,
        0.025852,
        legacy
    );
    // And explicitly assert the two temperatures produce different residuals
    // (the bug was the same value regardless of T).
    let mut rhs_300 = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::at_300k(), &mut rhs_300);
    assert!(
        (rhs[0] - rhs_300[0]).abs() > 1e-6,
        "A.3: residuals at T=300K and T=350K must differ; got both = {}",
        rhs[0]
    );
}

// ── A.5 — Digital BitNot / reduction ops silently become Not ──────────────────
//
// Old behavior: `ir_digital_to_interp.rs:167` mapped BitNot and every
// reduction op to `UnaryOp::Not`. `~x` and `&x` both became `!x`. After the
// fix, the codegen must reject them with a clear error.

fn make_digital_ir_with_unary_op(
    module_name: &str,
    op: piperine_codegen::ir::IrUnOp,
) -> piperine_codegen::IrProgram {
    use piperine_codegen::ir::{
        IrExpr, IrModule, IrProgram, IrStmt, IrDigitalBody,
    };
    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };
    prog.modules.push(IrModule {
        name: module_name.into(),
        ports: Vec::new(),
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: Some(IrDigitalBody {
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_vars: Vec::new(),
            stmts: vec![IrStmt::Assign {
                lval: "out".into(),
                expr: IrExpr::Unary(op, Box::new(IrExpr::Param("x".into()))),
                delay: None,
                event: None,
            }],
        }),
        functions: Vec::new(),
    });
    prog
}

#[test]
fn a5_bitnot_in_digital_is_rejected_not_silently_not() {
    use piperine_codegen::ir_digital_to_interp;
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("bitnot_fsm", IrUnOp::BitNot);
    let err = ir_digital_to_interp(&prog, "bitnot_fsm")
        .err()
        .expect("BitNot in digital must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("bitnot") || msg.contains("unary") || msg.contains("unsupported"),
        "A.5: expected BitNot rejection, got: {msg}"
    );
}

#[test]
fn a5_reduction_op_in_digital_is_rejected_not_silently_not() {
    use piperine_codegen::ir_digital_to_interp;
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("redand_fsm", IrUnOp::RedAnd);
    let err = ir_digital_to_interp(&prog, "redand_fsm")
        .err()
        .expect("RedAnd in digital must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("red") || msg.contains("unary") || msg.contains("unsupported"),
        "A.5: expected RedAnd rejection, got: {msg}"
    );
}

// ── A.5 (positive case) — Neg still works in digital ──────────────────────────

#[test]
fn a5_neg_in_digital_still_works() {
    // Positive case: Neg should NOT be rejected — only BitNot / reductions
    // are. The existing DFF/Buf tests already cover Neg/Not positively,
    // so we just smoke-test that a Neg assignment still lowers.
    use piperine_codegen::ir_digital_to_interp;
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("neg_fsm", IrUnOp::Neg);
    ir_digital_to_interp(&prog, "neg_fsm")
        .expect("Neg in digital must still work");
}

// ── A.6 — from_ir propagates child compile errors ─────────────────────────────
//
// Old behavior: `from_ir.rs:146-147, 153` used `.ok()` on
// `ir_analog_to_device` and `ir_digital_to_interp`, then silently
// dropped the instance if both returned `None`. A child whose analog
// body failed to compile would vanish from the circuit without any
// diagnostic. After the fix, the compile error is propagated with the
// instance label and module name in the message.

#[test]
fn a6_from_ir_propagates_child_compile_error_not_silent_skip() {
    use piperine_codegen::from_ir;
    use piperine_codegen::ir::{
        ContribKind, IrAnalogBody, IrConnection, IrDirection, IrExpr, IrInstance,
        IrModule, IrNature, IrProgram, IrPort, IrStmt,
    };

    // We model a `vsource` child whose analog contribution does
    // `I(p,n) <+ I(p,n)` — GAPS §A.1 makes that uncompilable (it was
    // the bug that silently dropped to 0). The parent `top` instantiates
    // it; `from_ir("top")` must propagate the error.
    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };

    prog.modules.push(IrModule {
        name: "vsource".into(),
        ports: vec![
            IrPort { name: "p".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
            IrPort { name: "n".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
        ],
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: Some(IrAnalogBody {
            state_vars: Vec::new(),
            noise_sources: Vec::new(),
            vars: Vec::new(),
            stmts: vec![IrStmt::Contrib {
                nature: IrNature::Flow("I".into()),
                plus: "p".into(),
                minus: "n".into(),
                expr: IrExpr::BranchAccess {
                    access: "I".into(),
                    plus: "p".into(),
                    minus: "n".into(),
                },
                kind: ContribKind::Resistive,
            }],
        }),
        digital: None,
        functions: Vec::new(),
    });

    prog.modules.push(IrModule {
        name: "top".into(),
        ports: vec![
            IrPort { name: "a".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
            IrPort { name: "b".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
        ],
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: vec![IrInstance {
            label: "u1".into(),
            module: "vsource".into(),
            connections: vec![
                IrConnection { port: Some("p".into()), net: "a".into() },
                IrConnection { port: Some("n".into()), net: "b".into() },
            ],
            params: Vec::new(),
        }],
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: None,
        functions: Vec::new(),
    });

    let err = from_ir(&prog, "top").err().expect("top with bad child must fail");
    assert!(err.contains("u1"), "A.6: error should name instance `u1`, got: {err}");
    assert!(err.contains("vsource"), "A.6: error should name module `vsource`, got: {err}");
}

// ── A.7 — from_elab `compile_analog_module` rejects ddt/idt ───────────────────
//
// Old behavior: the `from_elab` analog path silently stamped `ddt` as 0
// (`analog.rs:188-190`). A capacitor compiled through `from_elab` had no
// charge term and behaved like an open circuit in transient. After the
// fix, `compile_analog_module` rejects `ddt`/`idt` with a clear error
// (the legacy silent zero is now loud). Capacitors that use `ddt` must
// route through the IR path (`ppr_to_ir` + `ir_analog_to_device`).

#[test]
fn a7_ppr_ddt_in_from_elab_is_rejected_not_silently_zero() {
    use piperine_codegen::compile_analog_module;
    use piperine_lang::parse_and_elaborate;

    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        mod Cap ( inout p : Electrical, inout n : Electrical ) {
            param c : Real = 1.0e-9;
        }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#;
    let elab = parse_and_elaborate(src).expect("PHDL parses + elaborates");
    let err = compile_analog_module(&elab, "Cap")
        .err()
        .expect("from_elab must reject ddt with a clear error (GAPS §A.7)");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("ddt") && (msg.contains("a.7") || msg.contains("silent")),
        "A.7: error should mention ddt and GAPS §A.7, got: {msg}"
    );
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
    use piperine_codegen::{ams_to_ir, ir_analog_to_device};

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
        let ir = ams_to_ir(&doc);
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

// ── D.5 — User fn inlined at call site (GAPS §D.5) ──────────────────────────────
//
// Old behavior: `IrFunction` is populated by both frontends but never
// read by codegen. `validate_ir_contrib` rejects non-builtin calls
// (`ir_emit.rs:463-466`), so a model like `I(a,c) <+ my_gain * V(a,c)`
// fails compilation even when `my_gain` is a defined user function.
// After the fix, user-fn calls are inlined at the call site (alpha-
// substitute params with args, replace call with body's `Return`
// expression).

#[test]
fn d5_user_fn_inlined_at_call_site_in_contribution() {
    // PHDL `fn` declared at file scope; used inside the analog block.
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        fn scale_v (x : Real) -> Real { return x * 2.0; }

        mod Resistor ( inout p : Electrical, inout n : Electrical ) {
            param r : Real = 1.0e3;
        }
        analog Resistor { I(p, n) <+ scale_v(V(p, n)) / r; }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src)
        .expect("PHDL with user fn parses + elaborates");
    let ir = piperine_codegen::ppr_to_ir(&elab);
    let dev = piperine_codegen::ir_analog_to_device(&ir, "Resistor")
        .expect("D.5: user-fn inlining must compile");

    // Closed-form: I = 2*V / r = 2 * 0.5 / 1000 = 1e-3 A
    let params = [1.0e3_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &piperine_codegen::SimCtx::default(), &mut rhs);
    let expected = 2.0 * 0.5 / 1.0e3;
    assert!(
        (rhs[0] - expected).abs() < 1e-9,
        "D.5: residual should be 2*V/r = {expected}, got {}",
        rhs[0]
    );
}

#[test]
fn d5_user_fn_call_to_nonbuiltin_is_inlined_not_silently_zero() {
    // Non-trivial user fn: `g * x` where `g` is a param. After D.5 the
    // call is inlined and the substitution of `g` (a Param ref) into the
    // body must produce the right residual.
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        fn amp (x : Real, g : Real) -> Real { return g * x; }

        mod Gain ( inout p : Electrical, inout n : Electrical ) {
            param g : Real = 2.0;
        }
        analog Gain { I(p, n) <+ amp(V(p, n), g); }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src)
        .expect("PHDL with non-trivial user fn parses");
    let ir = piperine_codegen::ppr_to_ir(&elab);
    let dev = piperine_codegen::ir_analog_to_device(&ir, "Gain")
        .expect("D.5: user fn with `g * x` must compile");
    let params = [2.0_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &piperine_codegen::SimCtx::default(), &mut rhs);
    let expected = 2.0 * 0.5;
    assert!(
        (rhs[0] - expected).abs() < 1e-9,
        "D.5: residual should be g*V = {expected}, got {}",
        rhs[0]
    );
}

#[test]
fn d5_user_fn_missing_still_errors() {
    // A call to an unknown function must still fail loudly (GAPS §A.8).
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        mod Bad ( inout p : Electrical, inout n : Electrical ) {
            param r : Real = 1.0e3;
        }
        analog Bad { I(p, n) <+ no_such_fn(V(p, n)); }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src)
        .expect("PHDL parses");
    let ir = piperine_codegen::ppr_to_ir(&elab);
    let result = piperine_codegen::ir_analog_to_device(&ir, "Bad");
    let err = result.err().expect("D.5: missing fn must fail loudly");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("no_such_fn") || msg.contains("unknown") || msg.contains("unsupported"),
        "D.5: error should name the missing fn, got: {msg}"
    );
}

// Verify the canonical spec Diode model (Appendix A) now compiles after
// D.5 (user-fn inlining). Before D.5, the call to `thermal_voltage` was
// rejected by `validate_ir_contrib`.
#[test]
fn d5_spec_diode_with_user_fn_compiles() {
    use piperine_codegen::{ir_analog_to_device, ppr_to_ir, SimCtx};

    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }
        mod Diode ( inout a : Electrical, inout c : Electrical ) {
            param is_sat : Real = 1.0e-14;
            param temp   : Real = 300.0;
        }
        analog Diode { I(a, c) <+ is_sat * (exp(V(a, c) / thermal_voltage(temp)) - 1.0); }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src)
        .expect("Diode model parses + elaborates");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Diode")
        .expect("D.5: Diode (with `thermal_voltage` user fn) compiles");

    // Closed-form: I = is_sat * (exp(V / 8.617e-5 * temp) - 1)
    let params = [1.0e-14_f64, 300.0_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    let vt: f64 = 8.617e-5 * 300.0;
    let expected: f64 = 1.0e-14_f64 * ((0.5_f64 / vt).exp() - 1.0);
    assert!(
        (rhs[0] - expected).abs() < 1e-9,
        "D.5 spec Diode: residual = {}, want {} (vt = {})",
        rhs[0], expected, vt
    );
}

// ── D.2 — `idt` / `idtmod` integration operators ───────────────────────────────
//
// Old behavior: only `ddt` was lowered to a companion-model charge
// stamp; `idt` and `idtmod` were rejected by `build_reactive_contributions`
// (`ir_analog_to_device.rs:105-110`). After the fix they lower to the
// companion model the same way: `state_next = state_old + x * dt`
// (and modular wrap for `idtmod`). The reactive residual contribution
// is `I <+ Q(V)` where `Q = state_old + x * V` (or `Q = wrap(x * V, modulus)`).

#[test]
fn d2_idt_in_contribution_compiles_with_reactive_support() {
    // Inductor with I(a,c) <+ idt(V(a,c)) / L → I = ∫V/L dt (an
    // inductor's defining equation). The companion form: Q = ∫V dt,
    // and the residual reads the current through `dt`-scaled dQ/dV.
    use piperine_codegen::{ir_analog_to_device, ppr_to_ir, SimCtx};
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Inductor ( inout p : Electrical, inout n : Electrical ) {
            param L : Real = 1.0e-6;
        }
        analog Inductor { I(p, n) <+ idt(V(p, n)) / L; }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src).expect("PHDL parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Inductor")
        .expect("D.2: idt must compile (was rejected before)");
    assert!(
        dev.has_reactive(),
        "D.2: idt must produce reactive (charge) contributions"
    );
    // The DC residual should be zero (open circuit at DC, since the
    // inductor looks like a short-circuit in transient but open at DC
    // when the integral is the state).
    let params = [1.0e-6_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    assert!(
        rhs[0].abs() < 1e-12,
        "D.2: DC residual should be 0 (no state ref evaluated), got {}",
        rhs[0]
    );
    // The charge function should give Q = V / L = 0.5 / 1e-6 = 5e5.
    let mut q = [0.0; 2];
    dev.eval_charge(&v, &params, &SimCtx::default(), &mut q);
    let expected = 0.5 / 1.0e-6;
    assert!(
        (q[0] - expected).abs() < 1e-3 * expected,
        "D.2: charge Q = V/L = {expected}, got {}",
        q[0]
    );
}

// ── D.1 — `V(p,n) <- expr` ideal voltage source / force ────────────────────────
//
// Old behavior: `ir_analog_to_device.rs:206-211` rejected `Contrib` with
// `IrNature::Potential` ("potential contribution...") and the
// `Force` arm was similarly rejected. After the fix, force statements
// lower to the MNA voltage-source branch-current unknown (GAPS §H.4):
// `V+ − V− − expr = 0` plus branch current stamps on `V+`/`V−`.
//
// This test exercises the codegen path: it must compile (no fail-loud
// rejection), and the JIT must emit a working `force_residual` function
// that, given a node-voltage vector, writes the row equation
// `V+ − V− − expr` for the branch-current unknown. The actual MNA
// stamping is verified end-to-end by the H.4 solver test; here we
// confirm the device was built with a force function.

#[test]
fn d1_voltage_force_compiles_with_force_residual() {
    use piperine_codegen::{ir_analog_to_device, ppr_to_ir, SimCtx};
    // Spec Appendix A: VSource (a, c) { param dc : Real = 0.0; }
    //                  analog VSource { V(a, c) <- dc; }
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod VSource ( inout p : Electrical, inout n : Electrical ) {
            param dc : Real = 1.5;
        }
        analog VSource { V(p, n) <- dc; }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src)
        .expect("VSource parses + elaborates");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "VSource")
        .expect("D.1: V(p,n) <- dc must now compile (was rejected pre-D.1)");
    // The device must report having a force residual function.
    assert!(
        dev.has_force(),
        "D.1: VSource must report having a force-residual function"
    );

    // Sanity-check the residual: at any node voltage, `force_residual`
    // writes `V+ − V− − expr` for the branch-current row. With `expr = dc = 1.5`
    // and nodes `(1.2, 0.4)`, the row entry should be `1.2 − 0.4 − 1.5 = -0.7`.
    let params = [1.5_f64];
    let v = [1.2, 0.4];
    let mut rhs = [0.0; 1]; // 1 force → 1 row
    dev.eval_force(&v, &params, &SimCtx::default(), &mut rhs);
    assert!(
        (rhs[0] - (-0.7)).abs() < 1e-12,
        "D.1: force_residual row should be V+ − V− − dc = -0.7, got {}",
        rhs[0]
    );
}

#[test]
fn d1_op_amp_with_force_compiles() {
    // Spec Appendix B.5: OpAmp with V(out) <- gain * V(inp, inn).
    use piperine_codegen::{ir_analog_to_device, ppr_to_ir, SimCtx};
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod OpAmp ( input inp : Electrical, input inn : Electrical, inout out : Electrical ) {
            param gain : Real = 1.0e6;
        }
        analog OpAmp { V(out) <- gain * V(inp, inn); }
    "#;
    let elab = piperine_lang::parse_and_elaborate(src).expect("OpAmp parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "OpAmp")
        .expect("D.1: OpAmp with `V(out) <- gain * V(inp, inn)` compiles");
    assert!(dev.has_force(), "D.1: OpAmp must have a force function");
}
