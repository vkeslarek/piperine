//! `LIMIT` inputs (feature `solver-abi-analog`, SAN-04, T5): a reader of a
//! limit is linearized at the held iterate with Jacobian `g·p` and right side
//! `g·(x − x̂)` (C6); a limit beyond `1e-6 + 1e-4·|x|` vetoes convergence
//! (C7); `OWNS_CORRECTION` gets no correction and keeps the veto (C8); the
//! diode of abi.md §11.2 reaches the unlimited operating point from a 5 V
//! start; `LAST` and `AGE` follow the table of abi.md §4.1; a limit no
//! function writes is answered `LAST` (A3).
//!
//! Numerically sensitive: stamps are computed here by hand.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use circuit::{at, f64_bytes, f64_of, instance, netlist, tagged};
use devices::{G_LIMIT, K_B, Q, VT, calls, modules, pnjlim};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{Ambient, AnalogPlan, Assembly};
use system::{at_rest, at_time, close, dense, matrix_positions, size, solve_dc_with, tight};

/// A clamp between unknowns 0 and 1 whose held iterate is `x + off`.
fn clamp(off: f64) -> Assembly {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let c = instance("top.lim", m.clamp, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(off));
    asm
}

#[test]
fn a_limit_reader_is_linearized_at_the_held_iterate() {
    let (xa, xk, off) = (0.9, 0.2, -0.3);
    let mut asm = clamp(off);
    let stamps = asm.assemble_dc(&[xa, xk], at_rest()).expect("assemble");
    // By hand: x = 0.7, x̂ = 0.4; the reader reports G·x̂² with g = 2·G·x̂;
    // p = (1, −1). Value at x: G·x̂² + g·(x − x̂); Jacobian g·p.
    let x = xa - xk;
    let xh = x + off;
    let g = 2.0 * G_LIMIT * xh;
    let v = G_LIMIT * xh * xh + g * (x - xh);
    let rhs = -(v - (g * xa - g * xk));
    let (a, b) = dense(&stamps, 2);
    let rel = 1e-14;
    close(a[0][0], g, rel, "(a, a)");
    close(a[0][1], -g, rel, "(a, k)");
    close(a[1][0], -g, rel, "(k, a)");
    close(a[1][1], g, rel, "(k, k)");
    close(b[0], rhs, rel, "rhs(a)");
    close(b[1], -rhs, rel, "rhs(k)");
    assert_eq!(matrix_positions(&stamps).len(), 4);

    // With x̂ = x the correction vanishes: plain Norton at x.
    let mut asm = clamp(0.0);
    let stamps = asm.assemble_dc(&[xa, xk], at_rest()).expect("assemble");
    let g = 2.0 * G_LIMIT * x;
    let (a, b) = dense(&stamps, 2);
    close(a[0][0], g, rel, "(a, a) unlimited");
    close(b[0], -(G_LIMIT * x * x - (g * xa - g * xk)), rel, "rhs(a) unlimited");
}

#[test]
fn an_active_limit_vetoes_convergence() {
    // x = 0.7: the tolerance is 1e-6 + 1e-4·0.7 = 7.1e-5.
    for (off, vetoed) in [(0.0, false), (7.0e-5, false), (-7.0e-5, false), (7.2e-5, true), (-7.2e-5, true)] {
        let mut asm = clamp(off);
        asm.assemble_dc(&[0.9, 0.2], at_rest()).expect("assemble");
        let active = asm.limiting_active();
        assert_eq!(!active.is_empty(), vetoed, "offset {off}: {active:?}");
        if vetoed {
            assert_eq!(active[0].instance, "top.lim");
            assert_eq!(active[0].input, "x");
            close(active[0].proposed, 0.7, 1e-15, "x");
            close(active[0].limited, 0.7 + off, 1e-15, "x̂");
        }
    }
}

#[test]
fn owns_correction_gets_no_correction_and_keeps_the_veto() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let (xa, xk) = (0.9, 0.2);
    for (off, vetoed) in [(0.0, false), (0.5, true)] {
        let o = instance("own", m.owner, vec![Discrete(0), at(&n[0]), at(&n[1])]);
        let mut asm = Assembly::new(AnalogPlan::bind(vec![o], &net).expect("bind"));
        asm.set_discrete(0, &f64_bytes(off));
        let stamps = asm.assemble_dc(&[xa, xk], at_rest()).expect("assemble");
        // The function's own value and total partials, stamped as written.
        let v = G_LIMIT * (xa - xk) + 1e-3;
        let rhs = -(v - (G_LIMIT * xa - G_LIMIT * xk));
        let (a, b) = dense(&stamps, 2);
        assert_eq!(a, vec![vec![G_LIMIT, -G_LIMIT], vec![-G_LIMIT, G_LIMIT]], "offset {off}");
        close(b[0], rhs, 1e-14, "rhs(a)");
        close(b[1], -rhs, 1e-14, "rhs(k)");
        assert_eq!(!asm.limiting_active().is_empty(), vetoed, "offset {off}");
    }
}

#[test]
fn a_limited_diode_reaches_the_unlimited_operating_point() {
    let m = modules();
    let (net, n) = netlist(2, &["v1.f"]);
    let (vin, r, is) = (5.0, 1.0e3, 1.0e-14);
    let (top, mid, f) = (&n[0], &n[1], &n[2]);
    // n·k·T/q = VT: the limited diode and the plain one share one equation.
    let t = VT * Q / K_B;
    let solve = |limited: bool, start: f64| {
        let v = instance("v1", m.vsource, vec![Discrete(0), at(top), Ground, at(f)]);
        let r1 = instance("r1", m.resistor, vec![Discrete(1), at(top), at(mid)]);
        let d1 = if limited {
            instance("d1", m.diode, vec![Discrete(2), Discrete(3), Discrete(4), at(mid), Ground, Discrete(5)])
        } else {
            instance("d1", m.plain_diode, vec![Discrete(2), at(mid), Ground])
        };
        let mut asm = Assembly::new(AnalogPlan::bind(vec![v, r1, d1], &net).expect("bind"));
        // The plain diode binds slots 0 to 2 only; n, cj0 and T are the limited one's.
        let slots = [(0, vin), (1, r), (2, is), (3, 1.0), (4, 0.0), (5, t)];
        for (slot, value) in slots.into_iter().take(if limited { 6 } else { 3 }) {
            asm.set_discrete(slot, &f64_bytes(value));
        }
        let mut x0 = vec![0.0; size(&net)];
        x0[top.idx().expect("top")] = vin;
        x0[mid.idx().expect("mid")] = start;
        let x = solve_dc_with(&mut asm, &net, &x0, &tight());
        assert!(asm.limiting_active().is_empty(), "converged with no active limit");
        x[mid.idx().expect("mid")]
    };
    // Both solves stop at the tight tolerances: under the defaults each stops
    // within 1e-3 relative of the root, along its own Newton path.
    let limited = solve(true, 5.0);
    let plain = solve(false, 0.0);
    assert!((limited - plain).abs() <= 1e-9, "limited vd = {limited} vs unlimited {plain}");
    let i_r = (vin - limited) / r;
    let i_d = is * ((limited / VT).exp() - 1.0);
    assert!((i_r - i_d).abs() < 1e-9, "i(r) = {i_r} vs i(d) = {i_d}");
}

#[test]
fn age_and_last_follow_the_table_of_the_abi() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let tr = tagged("tr", m.tracker, 501, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![tr], &net).expect("bind"));
    let held = |c: &devices::Call| (f64_of(&c.reads[2]), c.reads[3][0]);

    asm.evaluate(&[0.75, 0.25], at_rest()).expect("evaluate");
    asm.end_iteration();
    asm.evaluate(&[1.0, 0.25], at_rest()).expect("evaluate");
    asm.accept(&[1.25, 0.25], at_rest()).expect("accept");
    asm.evaluate(&[1.25, 0.25], at_time(1e-9)).expect("evaluate");
    let c = calls(501);
    assert_eq!(c.len(), 4);
    assert_eq!(held(&c[0]), (0.0, 0), "none held before any solve");
    assert_eq!(held(&c[1]), (0.5, 2), "from the previous iteration");
    assert_eq!(held(&c[3]), (1.0, 1), "from the last accepted solution");

    // The diode's seed: STATIC with AGE = 1 starts again from vcrit; a
    // transient iteration limits against LAST.
    let t = VT * Q / K_B;
    let nvt = K_B * t / Q;
    let vcrit = nvt * (nvt / (std::f64::consts::SQRT_2 * 1e-14)).ln();
    let d = instance("d1", m.diode, vec![Discrete(0), Discrete(1), Discrete(2), at(&n[0]), Ground, Discrete(3)]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    for (slot, value) in [(0, 1e-14), (1, 1.0), (2, 0.0), (3, t)] {
        asm.set_discrete(slot, &f64_bytes(value));
    }
    asm.accept(&[0.3, 0.0], at_rest()).expect("accept at 0.3 V");
    let held = |asm: &mut Assembly, amb: Ambient| {
        asm.evaluate(&[1.5, 0.0], amb).expect("evaluate");
        asm.limiting_active()[0].limited
    };
    close(held(&mut asm, at_rest()), pnjlim(1.5, vcrit, nvt, vcrit), 1e-15, "STATIC, AGE 1: seeded");
    close(held(&mut asm, at_time(1e-9)), pnjlim(1.5, 0.3, nvt, vcrit), 1e-15, "AGE 1: against LAST");
}

#[test]
fn an_unevaluated_limit_answers_last() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let r = tagged("rd", m.reader, 601, vec![Discrete(0), at(&n[0])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![r], &net).expect("bind"));
    let stamps = asm.assemble_dc(&[0.8], at_rest()).expect("assemble");
    assert!(asm.limiting_active().is_empty(), "LIMITED = PROPOSED: no veto");
    asm.accept(&[0.8], at_rest()).expect("accept");
    asm.assemble_dc(&[0.9], at_rest()).expect("assemble");
    let c = calls(601);
    assert_eq!(c.len(), 1, "VALUE stays LAST, so the reader is not called again");
    assert_eq!(f64_of(&c[0].reads[0]), 0.0, "VALUE = LAST, none held");
    let (a, b) = dense(&stamps, 1);
    assert_eq!((a, b), (vec![vec![0.0]], vec![0.0]), "2·LAST with no correction and no column");
}
