//! `DELAY` terms (feature `solver-abi-analog`, SAN-05, SAN-03 AC 5, T6): in a
//! transient stage a term reads its accepted history at `t − td` (C9), and,
//! inside the current step, interpolates toward the current value; in an
//! equilibrium solve it is its value; a `td` beyond `max` fails naming
//! instance and term (C10); a restore returns the history.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use circuit::{at, f64_bytes, instance, netlist};
use devices::modules;
use piperine_analog_solver::device::AnalogBinding::Discrete;
use piperine_analog_solver::device::{AnalogPlan, Assembly, JumpPosition, Stage};
use piperine_analog_solver::math::integration::TrBdf2Phase;
use system::{at_rest, at_time, close, dense, matrix_positions};

const US: f64 = 1e-6;
const STAGE: Stage = Stage { phase: TrBdf2Phase::Bdf2, h: 0.5 * US, prev_h: 0.5 * US, jump: JumpPosition::None };

/// A delay line from unknown 0 (`a`) to the row of unknown 1 (`o`), with a
/// ramp `v(a) = t/µs` accepted at 0, 1, 2 and 3 µs.
fn ramp(td: f64, max: f64) -> Assembly {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let d = instance("top.dl", m.delay_line, vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(td));
    asm.set_discrete(1, &f64_bytes(max));
    for k in 0..4 {
        let t = k as f64 * US;
        asm.accept(&[k as f64, 0.0], at_time(t)).expect("accept");
    }
    asm
}

#[test]
fn a_delay_term_reads_its_history_at_t_minus_td() {
    // At 4 µs with td = 1.5 µs: t − td = 2.5 µs, between two accepted
    // points, so v = 2.5 from history and the current value weighs 0.
    let mut asm = ramp(1.5 * US, 10.0 * US);
    let stamps = asm.assemble_tran(&[4.0, 0.0], at_time(4.0 * US), STAGE).expect("assemble");
    let (a, b) = dense(&stamps, 2);
    close(b[1], -2.5, 1e-12, "rhs(o) = −v(a)(2.5 µs)");
    assert_eq!(a[1][0], 0.0, "no dependence on the current v(a)");
    assert_eq!(matrix_positions(&stamps), [(1, 0)].into(), "the position stays declared");

    // At 3.5 µs with td = 0.25 µs: t − td = 3.25 µs lies in the current step,
    // halfway from the last accepted point (3 µs, 3) to now (3.5 µs, v(a)).
    let mut asm = ramp(0.25 * US, 10.0 * US);
    let stamps = asm.assemble_tran(&[3.5, 0.0], at_time(3.5 * US), STAGE).expect("assemble");
    let (a, b) = dense(&stamps, 2);
    close(a[1][0], 0.5, 1e-12, "the current value's weight");
    // Value 0.5·3 + 0.5·3.5 = 3.25; Norton right side −(3.25 − 0.5·3.5).
    close(b[1], -1.5, 1e-12, "rhs(o)");
}

#[test]
fn a_delay_term_is_its_value_in_an_equilibrium_solve() {
    let mut asm = ramp(1.5 * US, 10.0 * US);
    let stamps = asm.assemble_dc(&[0.7, 0.0], at_rest()).expect("assemble");
    let (a, b) = dense(&stamps, 2);
    assert_eq!(a[1][0], 1.0, "∂v/∂v(a) at full weight");
    assert_eq!(b[1], 0.0, "−(v − 1·v(a)) with v = v(a)");
}

#[test]
fn a_delay_beyond_its_max_fails_naming_the_term() {
    let mut asm = ramp(2.0 * US, 1.0 * US);
    let err = asm.assemble_tran(&[4.0, 0.0], at_time(4.0 * US), STAGE).expect_err("td > max").to_string();
    for part in ["top.dl", "term `d`", "fixture:delay:T0", "max"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
    let mut asm = ramp(1.0 * US, 1.0 * US);
    let stamps = asm.assemble_tran(&[4.0, 0.0], at_time(4.0 * US), STAGE).expect("td = max is allowed");
    close(dense(&stamps, 2).1[1], -3.0, 1e-12, "v(a)(3 µs), kept by the pruning");
}

#[test]
fn a_restore_returns_the_delay_history() {
    let mut asm = ramp(1.5 * US, 10.0 * US);
    let x = [4.5, 0.0];
    let before = asm.assemble_tran(&x, at_time(4.5 * US), STAGE).expect("assemble");
    let snap = asm.snapshot();
    // A rejected step: its accepted point must not stay in the history.
    asm.accept(&[100.0, 0.0], at_time(3.5 * US)).expect("accept");
    let moved = asm.assemble_tran(&x, at_time(5.0 * US), STAGE).expect("assemble");
    assert_ne!(dense(&moved, 2).1, dense(&before, 2).1);
    asm.restore(&snap);
    let after = asm.assemble_tran(&x, at_time(4.5 * US), STAGE).expect("assemble");
    assert_eq!(dense(&after, 2), dense(&before, 2));
}
