//! Wave 2 stress tests: reactive (`ddt`) contributions lower to a real charge
//! function via the companion model — caps/inductors are no longer open
//! circuits.  Operators that are recognised in the IR but not yet lowered to
//! code fail loud instead of silently contributing 0.
//!
//! The charge codegen is validated deterministically at the device level:
//! `Q(V)` and `dQ/dV` are exactly what the transient (`+alpha*dQ/dV`) and AC
//! (`+jω*dQ/dV`) stamps consume.

use piperine_ams::Document;
use piperine_codegen::{ir_analog_to_device, IrProgram, SimCtx};

fn ir(src: &str) -> IrProgram {
    piperine_ams::ams_to_ir(&Document::parse(src).expect("VA parses"))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

// ── ddt: capacitor charge Q = C·V, dQ/dV = C ──────────────────────────────────

#[test]
fn capacitor_ddt_lowers_to_charge_function() {
    let prog = ir(r#"
        module cap(a, c);
            inout a, c;
            electrical a, c;
            parameter real cval = 2.0e-9;
            analog begin
                I(a, c) <+ cval * ddt(V(a, c));
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "cap").expect("capacitor compiles");

    assert!(dev.has_reactive(), "ddt must produce a reactive charge function");

    let cval = 2.0e-9;
    let params = [cval];
    let v = [0.8, 0.0]; // V(a,c) = 0.8

    // Q accumulates at the contribution terminals: +C·V at a, −C·V at c.
    let mut q = [0.0; 2];
    dev.eval_charge(&v, &params, &SimCtx::default(), &mut q);
    assert!(close(q[0], cval * 0.8), "Q(a) = {}, want {}", q[0], cval * 0.8);
    assert!(close(q[1], -cval * 0.8), "Q(c) = {}, want {}", q[1], -cval * 0.8);

    // dQ/dV = C (the capacitance), stamped as conductance C/dt (transient) and
    // susceptance ωC (AC).
    let mut qjac = [0.0; 4];
    dev.eval_charge_jacobian(&v, &params, &SimCtx::default(), &mut qjac);
    assert!(close(qjac[0], cval), "dQ(a)/dV(a) = {}, want {cval}", qjac[0]);
    assert!(close(qjac[1], -cval), "dQ(a)/dV(c) = {}, want {}", qjac[1], -cval);
    assert!(close(qjac[3], cval), "dQ(c)/dV(c) = {}, want {cval}", qjac[3]);

    // The purely-reactive contribution has zero resistive (DC) part.
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    assert!(close(rhs[0], 0.0) && close(rhs[1], 0.0), "DC residual must be 0, got {rhs:?}");
}

// ── Nonlinear charge: Q = Cj·V² (junction-like) keeps the chain rule ──────────

#[test]
fn nonlinear_ddt_charge_uses_chain_rule() {
    let prog = ir(r#"
        module varcap(a, c);
            inout a, c;
            electrical a, c;
            parameter real cj = 1.0e-9;
            analog begin
                I(a, c) <+ ddt(cj * V(a, c) * V(a, c));
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "varcap").expect("varcap compiles");
    assert!(dev.has_reactive());
    let cj = 1.0e-9;
    let params = [cj];
    let vv = 0.5;

    let mut q = [0.0; 2];
    dev.eval_charge(&[vv, 0.0], &params, &SimCtx::default(), &mut q);
    assert!(close(q[0], cj * vv * vv), "Q = {}, want {}", q[0], cj * vv * vv);

    // dQ/dV = 2·Cj·V
    let mut qjac = [0.0; 4];
    dev.eval_charge_jacobian(&[vv, 0.0], &params, &SimCtx::default(), &mut qjac);
    assert!(close(qjac[0], 2.0 * cj * vv), "dQ/dV = {}, want {}", qjac[0], 2.0 * cj * vv);
}

// ── Mixed resistive + reactive: only the ddt term becomes charge ──────────────

#[test]
fn mixed_resistor_capacitor_splits_cleanly() {
    let prog = ir(r#"
        module rc(a, c);
            inout a, c;
            electrical a, c;
            parameter real g = 1.0e-3;
            parameter real cval = 5.0e-12;
            analog begin
                I(a, c) <+ V(a, c) * g + cval * ddt(V(a, c));
            end
        endmodule
    "#);
    let dev = ir_analog_to_device(&prog, "rc").expect("rc compiles");
    let g = 1.0e-3;
    let cval = 5.0e-12;
    let params = [g, cval];
    let v = [0.6, 0.0];

    // Resistive residual = V·g (no charge leakage into DC).
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    assert!(close(rhs[0], 0.6 * g), "resistive I = {}, want {}", rhs[0], 0.6 * g);

    // Charge = only the cap term, Q = C·V; the V·g term must NOT appear.
    let mut q = [0.0; 2];
    dev.eval_charge(&v, &params, &SimCtx::default(), &mut q);
    assert!(close(q[0], cval * 0.6), "Q = {}, want {} (resistive part must cancel)", q[0], cval * 0.6);
}

// ── idt (and friends) recognised in IR but fail loud at codegen ───────────────

#[test]
fn idt_operator_is_recognised_and_lowered_to_companion() {
    // GAPS §D.2 — `idt` is now lowered to a reactive charge stamp, the
    // same shape as `ddt`. The reactive contribution is `Q = expr[StateRef→arg]`
    // (the part of the operator's output that scales linearly with V).
    // Full integration math (the state_prev/dt residual, the dt-scaled
    // Jacobian, modular wrap) is a follow-up in `load_transient`.
    let prog = ir(r#"
        module integ(a, c);
            inout a, c;
            electrical a, c;
            parameter real k = 1.0;
            analog begin
                I(a, c) <+ k * idt(V(a, c));
            end
        endmodule
    "#);
    let m = prog.modules.iter().find(|m| m.name == "integ").unwrap();
    let body = m.analog.as_ref().unwrap();
    assert!(!body.state_vars.is_empty(), "idt must allocate a state var in the IR");

    let dev = ir_analog_to_device(&prog, "integ")
        .expect("D.2: idt must now compile (was rejected pre-D.2)");
    assert!(
        dev.has_reactive(),
        "D.2: idt must produce a reactive (charge) contribution"
    );
}
