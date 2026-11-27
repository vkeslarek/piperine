//! The power stage of the buck package runs from MSL source (feature `msl-digital`, T53 and T54;
//! checks C122, C124, C125, C127, C142, DIG-13 AC 4 to 7, DIG-14 AC 12): `GateDriver` under
//! scripted commands; `PowerSwitch` between a source and a resistor with its gate committed high
//! at `t0`; `Inductor` behind a stepped source and a resistor; `CurrentSense` at an operating
//! point; all from `tests/buck/power.msl`. The switch's `transition` is the stdlib module, hoisted
//! by the elaborator. Voltages are compared with their analytic values through
//! `ToleranceContract::{TRAN_VOLTAGE, DC_VOLTAGE}`; no time tolerance is written.
#![cfg(unix)]

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

use piperine_ir_fixtures::digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
use piperine_ir_fixtures::mixed::TopDesign;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

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
    // Both gates are drives with a delay. A carrier has its initial value, false, from (0, 0) on
    // (part II §5.1.1, §5.3.8), so the drive's first commit, false, comes due at `tpd` and
    // writes the value the gate already holds: "a commit of the value the carrier already holds
    // is not a change" (part II §5.3.5). Each gate's first change is therefore its first
    // command-driven one.
    let hs = vec![(TopDesign::rising_edge(1) + tpd, HIGH.to_vec()), (TopDesign::rising_edge(4) + tpd, LOW.to_vec()), (TopDesign::rising_edge(11) + tpd, HIGH.to_vec())];
    // `ls_gate = (ls_cmd & !hs_cmd) after tpd`: rises `tpd` after `ls_cmd` (edge 6); the low side
    // command with `hs_cmd` high at edge 8 is a command to neither: it falls `tpd` after (edge
    // 8); and it stays false when `ls_cmd` falls (edge 11)
    let ls = vec![(TopDesign::rising_edge(6) + tpd, HIGH.to_vec()), (TopDesign::rising_edge(8) + tpd, LOW.to_vec())];
    let (hs_seen, ls_seen) = (transient(&seen, "DriverBench.hs_gate"), transient(&seen, "DriverBench.ls_gate"));
    for gate in ["DriverBench.hs_gate", "DriverBench.ls_gate"] {
        let start = series(&seen, gate);
        assert_eq!(start.first(), Some(&(0, LOW.to_vec())), "{gate} holds its initial value, false, from the first instant");
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

/// The anonymous node `path` of a compiled bench.
fn node(built: &mixed::Built, path: &str) -> NodeIdentifier {
    NodeIdentifier::Anonymous(built.ir.node_at(path).unwrap_or_else(|| panic!("a node {path}")).id().index())
}

#[test]
fn the_inductor_follows_the_rl_response() {
    // `RlBench`: 1 V stepped at t0 = 1 us into R = 1 ohm in series with `Inductor { l: 1.5u, dcr:
    // 0.5 }` (DIG-13 AC 4): i(t) = V / (R + dcr) * (1 - exp(-(t - t0) (R + dcr) / l)), read as the
    // voltage across the 1 ohm resistor, so v(a, b) = R * i(t)
    let (v, r, l, dcr) = (1.0, 1.0, 1.5e-6, 0.5);
    let t0: f64 = 1e-6;
    let built = build(&project(), "RlBench");
    let (a, b) = (node(&built, "RlBench.a"), node(&built, "RlBench.b"));
    // a 1 ns step cap, as for the switch: the first step after the source's jump is first order
    let (seen, result) = built.run_with(6e-6, 1e-9);
    // the source's gate is committed high once, at t0 (the clock's first rising edge)
    assert_eq!(transient(&seen, "RlBench.g"), vec![((t0 * 1e15).round() as i64, HIGH.to_vec())]);
    let mut after = 0;
    for step in result.iter() {
        let t = step.time();
        let va = step.get_node(&a).expect("v(a)");
        let vb = step.get_node(&b).expect("v(b)");
        let golden = if t <= t0 { 0.0 } else { r * v / (r + dcr) * (1.0 - (-(t - t0) * (r + dcr) / l).exp()) };
        if t > t0 {
            after += 1;
        }
        let at = Point::new("rl", "R * i(r)", Coordinate::Time(t));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, golden, va - vb);
    }
    assert!(after >= 1000, "the response is sampled at every accepted time after t0: {after}");
}

#[test]
fn the_current_sense_scales_the_shunt_voltage() {
    // `SenseBench`: 1 V through the 5 mohm shunt into 1 ohm (DIG-13 AC 5): at the operating point
    // v(pos) = gain * v(a, b) and v(neg) = -gain * v(a, b), gain 20 (SPEC §2: 0.1 V/A)
    let built = build(&project(), "SenseBench");
    let ids: Vec<NodeIdentifier> = ["a", "b", "pos", "neg"].iter().map(|n| node(&built, &format!("SenseBench.{n}"))).collect();
    let mut circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let op = circuit.dc(Context::default()).expect("dc").solve().unwrap_or_else(|e| panic!("the operating point: {e}"));
    let read = |k: usize| op.get_node(&ids[k]).expect("a node voltage");
    let (gain, rshunt, rload) = (20.0, 5e-3, 1.0);
    // the shunt and the load divide 1 V: v(a, b) = 1 V * rshunt / (rshunt + rload)
    let shunt = 1.0 * rshunt / (rshunt + rload);
    let at = |q: &'static str| Point::new("current_sense", q, Coordinate::OperatingPoint);
    ToleranceContract::DC_VOLTAGE.assert(&at("v(a, b)"), shunt, read(0) - read(1));
    ToleranceContract::DC_VOLTAGE.assert(&at("v(pos)"), gain * shunt, read(2));
    ToleranceContract::DC_VOLTAGE.assert(&at("v(neg)"), -gain * shunt, read(3));
    // the sense is the shunt voltage, not the node voltage: v(pos) = gain * (v(a) - v(b))
    ToleranceContract::DC_VOLTAGE.assert(&at("v(pos) from the nodes"), gain * (read(0) - read(1)), read(2));
}
