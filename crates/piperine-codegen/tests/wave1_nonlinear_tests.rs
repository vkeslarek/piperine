//! Wave 1 stress tests: nonlinear & piecewise analog contributions evaluated
//! numerically through the direct IR → Cranelift emitter.
//!
//! These lock in the fix for the old `ir_expr_to_phdl` round-trip, which
//! silently collapsed several `IrExpr` variants to `0.0` (ternary `Select`) or
//! to the *wrong* operator (`**` and comparisons both became `+`).  Each test
//! drives the compiled `JitAnalogDevice` and checks the residual current and
//! Jacobian conductance against the closed-form expectation.

use piperine_ams::Document;
use piperine_codegen::{ams_to_ir, ir_analog_to_device, IrProgram};

/// Parse a Verilog-A snippet and lower it to IR.
fn ir(src: &str) -> IrProgram {
    let doc = Document::parse(src).expect("VA parses");
    ams_to_ir(&doc)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

// ── Nonlinear: diode exponential I-V ──────────────────────────────────────────

#[test]
fn diode_exponential_iv_residual_and_jacobian() {
    let prog = ir(r#"
        module diode(a, c);
            inout a, c;
            electrical a, c;
            parameter real is_sat = 1.0e-14;
            parameter real vt = 0.025852;
            analog begin
                I(a, c) <+ is_sat * (exp(V(a, c) / vt) - 1.0);
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "diode").expect("diode compiles");

    // params follow declaration order: [is_sat, vt]; ports: [a, c].
    let is_sat = 1.0e-14;
    let vt = 0.025852;
    let params = [is_sat, vt];
    let v = [0.5, 0.0]; // V(a,c) = 0.5

    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &mut rhs);
    let i_expected = is_sat * ((0.5 / vt).exp() - 1.0);
    assert!(close(rhs[0], i_expected), "I(a) = {}, want {i_expected}", rhs[0]);
    assert!(close(rhs[1], -i_expected), "I(c) = {}, want {}", rhs[1], -i_expected);
    assert!(i_expected > 1.0e-7, "diode should conduct meaningfully at 0.5V");

    // dI/dV = is_sat/vt * exp(V/vt)
    let mut jac = [0.0; 4];
    dev.eval_jacobian(&v, &params, &mut jac);
    let g = is_sat / vt * (0.5 / vt).exp();
    assert!(close(jac[0], g), "dI(a)/dV(a) = {}, want {g}", jac[0]);
    assert!(close(jac[1], -g), "dI(a)/dV(c) = {}, want {}", jac[1], -g);
}

// ── Piecewise: ternary Select (old path silently produced 0.0) ────────────────

#[test]
fn ternary_piecewise_resistor_selects_correct_branch() {
    let prog = ir(r#"
        module piecewise(a, c);
            inout a, c;
            electrical a, c;
            parameter real gon = 1.0e-2;
            parameter real goff = 1.0e-5;
            analog begin
                I(a, c) <+ (V(a, c) >= 0.0) ? V(a, c) * gon : V(a, c) * goff;
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "piecewise").expect("piecewise compiles");
    let gon = 1.0e-2;
    let goff = 1.0e-5;
    let params = [gon, goff];

    // Forward bias: V >= 0 → on-conductance branch.
    let mut rhs = [0.0; 2];
    dev.eval_residual(&[0.4, 0.0], &params, &mut rhs);
    assert!(close(rhs[0], 0.4 * gon), "forward I = {}, want {}", rhs[0], 0.4 * gon);

    // Reverse bias: V < 0 → off-conductance branch.
    let mut rhs2 = [0.0; 2];
    dev.eval_residual(&[-0.4, 0.0], &params, &mut rhs2);
    assert!(close(rhs2[0], -0.4 * goff), "reverse I = {}, want {}", rhs2[0], -0.4 * goff);

    // Crucially, neither branch is the old silent 0.0.
    assert!(rhs[0].abs() > 1e-12 && rhs2[0].abs() > 1e-12);

    // Jacobian picks up the selected branch's conductance.
    let mut jac = [0.0; 4];
    dev.eval_jacobian(&[0.4, 0.0], &params, &mut jac);
    assert!(close(jac[0], gon), "forward dI/dV = {}, want {gon}", jac[0]);
}

// ── Power law: `**` operator (old path silently became `+`) ───────────────────

#[test]
fn power_law_contribution_uses_pow_not_add() {
    let prog = ir(r#"
        module plaw(a, c);
            inout a, c;
            electrical a, c;
            parameter real k = 3.0e-3;
            analog begin
                I(a, c) <+ k * V(a, c) ** 2;
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "plaw").expect("plaw compiles");
    let k = 3.0e-3;
    let params = [k];
    let vv = 0.7;

    let mut rhs = [0.0; 2];
    dev.eval_residual(&[vv, 0.0], &params, &mut rhs);
    let i_expected = k * vv.powi(2);
    assert!(close(rhs[0], i_expected), "I = {}, want {i_expected}", rhs[0]);
    // Old code: k * (V + 2) = 3e-3 * 2.7 = 8.1e-3, very different from 1.47e-3.
    assert!((rhs[0] - k * (vv + 2.0)).abs() > 1e-4, "must not be the old `+` lowering");

    // dI/dV = 2*k*V
    let mut jac = [0.0; 4];
    dev.eval_jacobian(&[vv, 0.0], &params, &mut jac);
    assert!(close(jac[0], 2.0 * k * vv), "dI/dV = {}, want {}", jac[0], 2.0 * k * vv);
}
