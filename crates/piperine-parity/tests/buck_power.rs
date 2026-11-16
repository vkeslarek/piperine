//! The gate driver and the power switch of the buck package run from MSL source (feature
//! `msl-digital`, T53; checks C125, C127, C142, DIG-13 AC 6 and 7, DIG-14 AC 12): `GateDriver`
//! under scripted commands, and `PowerSwitch` between a source and a resistor with its gate
//! committed high at `t0`, from `tests/buck/power.msl`. The switch's `transition` is the stdlib
//! module, hoisted by the elaborator. Voltages are compared with the divider of the analytic
//! channel conductance through `ToleranceContract::TRAN_VOLTAGE`; no time tolerance is written.
#![cfg(unix)]

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::NodeIdentifier;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::rise;
use support::{HIGH, LOW, build, series, transient, with_buck};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

fn project() -> Project {
    with_buck("power.msl")
}

#[test]
fn the_gate_driver_delays_and_interlocks() {
    // the commands move at the rising edges 1, 4, 6, 8 and 11 (steps 0, 3, 5, 7 and 10); the
    // driver's `tpd` is 10 ns (SPEC §5.3: a propagation delay on both gates, and a command to
    // both switches is a command to neither)
    let tpd = 10 * NS;
    let seen = build(&project(), "DriverBench").run(80e-9, 1e-9);
    // `hs_gate = (hs_cmd & !ls_cmd) after tpd`: the high side command with the low side low rises
    // `tpd` after it (edge 1); falls `tpd` after the command does (edge 4); with `ls_cmd` high
    // and `hs_cmd` raised again (edge 8) the gate stays false; it rises `tpd` after `ls_cmd`
    // falls (edge 11), the high side command still high
    //
    // Both gates are drives with a delay, and a carrier is unknown until its first commit
    // (part I §5.1.1): the drive's first commit is made at (0, 0) and is due `tpd` later, so each
    // gate is unknown at the operating point and becomes known false at `tpd`, before any command
    // arrives.
    let first = (tpd, LOW.to_vec());
    let hs = vec![first.clone(), (rise(1) + tpd, HIGH.to_vec()), (rise(4) + tpd, LOW.to_vec()), (rise(11) + tpd, HIGH.to_vec())];
    // `ls_gate = (ls_cmd & !hs_cmd) after tpd`: rises `tpd` after `ls_cmd` (edge 6); the low side
    // command with `hs_cmd` high at edge 8 is a command to neither: it falls `tpd` after (edge
    // 8); and it stays false when `ls_cmd` falls (edge 11)
    let ls = vec![first, (rise(6) + tpd, HIGH.to_vec()), (rise(8) + tpd, LOW.to_vec())];
    let (hs_seen, ls_seen) = (transient(&seen, "DriverBench.hs_gate"), transient(&seen, "DriverBench.ls_gate"));
    for gate in ["DriverBench.hs_gate", "DriverBench.ls_gate"] {
        let start = series(&seen, gate);
        assert_eq!(start.first().map(|(t, b)| (*t, b[1])), Some((tpd, 1)), "{gate} holds no value before its first commit is due");
    }
    assert_eq!(hs_seen, hs, "the high gate");
    assert_eq!(ls_seen, ls, "the low gate");
    // never both gates true at once: the true intervals of the two do not overlap
    let intervals = |s: &[(i64, Vec<u8>)]| -> Vec<(i64, i64)> {
        let mut out = Vec::new();
        let mut from = None;
        for (t, b) in s {
            match (b[0] == 1, from) {
                (true, None) => from = Some(*t),
                (false, Some(f)) => {
                    out.push((f, *t));
                    from = None;
                }
                _ => {}
            }
        }
        out.extend(from.map(|f| (f, i64::MAX)));
        out
    };
    for (a, b) in intervals(&hs_seen).into_iter().flat_map(|h| intervals(&ls_seen).into_iter().map(move |l| (h, l))) {
        assert!(a.1 <= b.0 || b.1 <= a.0, "the gates overlap: {a:?} {b:?}");
    }
}

/// The channel conductance of `PowerSwitch` (`ron`, `roff`, `tsw`) at `t` for a gate raised at
/// `t0`: `1 / roff` before, linear in the ramp `k` through `tsw`, `1 / ron` after (SPEC §2).
fn conductance(t: f64, t0: f64, tsw: f64, ron: f64, roff: f64) -> f64 {
    let k = ((t - t0) / tsw).clamp(0.0, 1.0);
    k / ron + (1.0 - k) / roff
}

/// 1 V through the switch of conductance `g` into `r2`.
fn divider(g: f64, v: f64, r2: f64) -> f64 {
    v * g / (g + 1.0 / r2)
}

/// The compiled switch run to 3.1 us: the gate change at `t0`, the node, every accepted time.
struct Ramp {
    seen: mixed::Seen,
    result: piperine_solver::prelude::TransientAnalysisResult,
    x: NodeIdentifier,
}

const T0_TICKS: i64 = 3_003_700_000;
const RON: f64 = 1e3;
const ROFF: f64 = 1e6;
const TSW: f64 = 5e-9;
const R2: f64 = 9e3;

fn ramp() -> Ramp {
    let built = build(&project(), "SwitchBench");
    let x = NodeIdentifier::Anonymous(built.ir.node_at("SwitchBench.x").expect("a node x").id().index());
    // a 1 ns step cap: the first step from a jump is first order and its error grows with the
    // square of the step (the harness's own policy at 10 ns is not tight enough here, see
    // `boundary.rs`)
    let (seen, result) = built.run_with(3.1e-6, 1e-9);
    Ramp { seen, result, x }
}

#[test]
fn the_power_switch_ramps_its_conductance_in_tsw() {
    let r = ramp();
    // the gate is committed high once, at t0 = half the period 6.0074 us
    assert_eq!(transient(&r.seen, "SwitchBench.gate"), vec![(T0_TICKS, HIGH.to_vec())]);
    let t0 = T0_TICKS as f64 * 1e-15;
    // from t0 - tsw to t0 + 2 tsw, at every accepted time, the node is the divider of the
    // analytic conductance (C125): 1 / roff before t0, linear through tsw, 1 / ron after
    let mut inside = 0;
    for step in r.result.iter() {
        let t = step.time();
        if t < t0 - TSW || t > t0 + 2.0 * TSW {
            continue;
        }
        inside += 1;
        let v = step.get_node(&r.x).expect("v(x)");
        let at = Point::new("power_switch", "v(x)", Coordinate::Time(t));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, divider(conductance(t, t0, TSW, RON, ROFF), 1.0, R2), v);
    }
    assert!(inside >= 10, "the window is sampled many times: {inside}");
    // the operating point holds the open switch: 1 V · (1/roff) / (1/roff + 1/r2)
    let first = r.result.iter().next().expect("the operating point");
    let at = Point::new("power_switch", "v(x)", Coordinate::OperatingPoint);
    ToleranceContract::TRAN_VOLTAGE.assert(&at, divider(1.0 / ROFF, 1.0, R2), first.get_node(&r.x).expect("v(x)"));
    // the series of the gate is a net change at `t0`: nothing before it
    assert_eq!(series(&r.seen, "SwitchBench.gate").first().map(|(t, _)| *t), Some(0));
}

#[test]
fn a_gate_change_makes_two_time_points() {
    let r = ramp();
    let t0 = T0_TICKS as f64 * 1e-15;
    // C127: the solver has an accepted solution at t0 itself (the carrier change) and at
    // t0 + tsw (the far corner of the ramp), within a tick, though the step cap is 1 ns and
    // neither instant is on its grid
    for (what, want) in [("t0", t0), ("t0 + tsw", t0 + TSW)] {
        assert!(r.result.iter().any(|s| (s.time() - want).abs() < 1e-15), "a record at {what} = {want:e} s");
    }
}
