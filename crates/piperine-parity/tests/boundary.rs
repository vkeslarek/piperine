//! The two boundaries of a compiled mixed design on the solver (feature `ir-rust`, RS-13 AC 8
//! to 10; checks C92, C93, C94): analog to digital here (T45), digital to analog in the second
//! half of this file (T46). The circuits are the compiled bodies of the buck slice placed under
//! a top and run through `Compiled::circuit`; no interpreter, no bench. Expected values are
//! written from `examples/buck/SPEC.md` §4.1 (the ADC) and from the analytic response of the
//! RC networks; every voltage comparison goes through `ToleranceContract::TRAN_VOLTAGE`, and no
//! time tolerance is written anywhere.
#![cfg(unix)]

use piperine_ir::{BodyId, CarrierRole, Constant, IrBuilder, Real, ValueType};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;

use devices::Devices;
use digital::{Buck, Dx, Ty};
use mixed::{Built, Wire};

const BIT: ValueType = ValueType::Bool;
/// A true and a false `Bit`: `{ v @0 }` (abi.md §3.1).
const HIGH: [u8; 1] = [1];
const LOW: [u8; 1] = [0];
/// One nanosecond in ticks (the solver's one femtosecond).
const NS: i64 = 1_000_000;

fn real(v: f64) -> Constant {
    Constant::Real(Real::new(v))
}

#[test]
fn the_adc_commits_the_rounded_code_after_tconv() {
    // SPEC §4.1: 10-bit, Vref 1.6 V, mid-tread. Five held voltages: 0, one LSB, mid-scale, full
    // scale and above full scale
    const VREF: f64 = 1.6;
    const TCONV: f64 = 500e-9;
    const TSTROBE: f64 = 5e-9;
    let lsb = VREF / 1024.0;
    let volts = [0.0, lsb, VREF / 2.0, VREF, 2.0];
    let expected_code = |v: f64| -> u16 { (v / VREF * 1024.0 + 0.5).floor().clamp(0.0, 1023.0) as u16 };
    assert_eq!(volts.map(expected_code), [0, 1, 512, 1023, 1023]);

    let mut w = Wire::new();
    let (osc, adc, vsource) = (Buck::osc(&mut w.b), Buck::adc(&mut w.b), Devices::vsource(&mut w.b));
    // a sample strobe once per 1 us: the oscillator rises at 0.5 us and again at 1.5 us
    w.place("osc", osc, vec![real(1e6)], &[("sample", BIT)]);
    for (i, v) in volts.iter().enumerate() {
        let node = format!("a{i}");
        w.place_analog(&format!("v{i}"), vsource, &[&node, "gnd"], &[*v, 0.0]);
        let (code, done) = (format!("code{i}"), format!("done{i}"));
        w.place_with(
            &format!("adc{i}"),
            adc,
            &[&node],
            vec![real(VREF), real(TCONV), real(TSTROBE)],
            &[("sample", BIT), (&code, Ty::u(10)), (&done, BIT)],
        );
    }
    // 1.4 us: the first conversion ends at 1.0 us and the second sample is not reached
    let seen = Built::new(w.finish()).run(1.4e-6, 1e-8);
    let sample = 500 * NS;
    assert_eq!(seen.transient("sample")[0], (sample, HIGH.to_vec()), "the strobe rises at 0.5 us");
    for (i, v) in volts.iter().enumerate() {
        let code = expected_code(*v);
        // the code appears after `tconv` from the rising edge, and not before: a code of 0 is
        // the value it already holds, so it commits nothing visible
        let changes = seen.transient(&format!("code{i}"));
        let wanted: Vec<(i64, Vec<u8>)> = if code == 0 { vec![] } else { vec![(sample + 500 * NS, code.to_le_bytes().to_vec())] };
        assert_eq!(changes, wanted, "code of {v} V");
        // `done` is true from `tconv` for `tstrobe`
        assert_eq!(
            seen.transient(&format!("done{i}")),
            vec![(sample + 500 * NS, HIGH.to_vec()), (sample + 505 * NS, LOW.to_vec())],
            "done at {v} V"
        );
    }
}

/// The RC network behind the comparator and the switch test: a source `V` through the switch
/// (`ron` closed, `roff` open) into the node `x`, which has `R2` and `C` to ground.
///
/// ```text
///   vs --[ switch ]-- x --+-- R2 -- gnd         C dv/dt = (V - v) * g_switch - v / R2
///                         +-- C  -- gnd
/// ```
///
/// With the switch conductance `g` held, the node follows `v(t) = v_inf + (v0 - v_inf) *
/// exp(-(t - t0) / tau)`, `v_inf = V * g / (g + 1/R2)` and `tau = C / (g + 1/R2)`: the Thevenin
/// form of the first-order network.
struct Network {
    v: f64,
    ron: f64,
    roff: f64,
    r2: f64,
    c: f64,
}

impl Network {
    /// The settled voltage and time constant with the switch conductance `g`.
    fn phase(&self, g: f64) -> (f64, f64) {
        let total = g + 1.0 / self.r2;
        (self.v * g / total, self.c / total)
    }

    /// The node voltage at `t` seconds when the gate rises at `on` and, if given, falls at
    /// `off`; the operating point before `on` is the settled voltage with the switch open
    /// (the capacitor is open at DC).
    fn at(&self, t: f64, on: f64, off: Option<f64>) -> f64 {
        let (open_v, open_tau) = self.phase(1.0 / self.roff);
        let (closed_v, closed_tau) = self.phase(1.0 / self.ron);
        if t <= on {
            return open_v;
        }
        let closed = |t: f64| closed_v + (open_v - closed_v) * (-(t - on) / closed_tau).exp();
        match off {
            Some(off) if t > off => {
                let start = closed(off);
                open_v + (start - open_v) * (-(t - off) / open_tau).exp()
            }
            _ => closed(t),
        }
    }
}

const NETWORK: Network = Network { v: 1.0, ron: 1e3, roff: 1e6, r2: 9e3, c: 0.5e-9 };

/// Places the source, the switch, the load, the capacitor and the oscillator that drives the
/// gate: the gate rises at 3 us and falls at 6 us (one period of 1 / f = 6 us, half a period
/// from the start, from low).
fn network(w: &mut Wire) {
    let (osc, switch) = (Buck::osc(&mut w.b), Buck::switch(&mut w.b));
    let (vsource, resistor, capacitor) = (Devices::vsource(&mut w.b), Devices::resistor(&mut w.b), Devices::capacitor(&mut w.b));
    w.place("osc", osc, vec![real(1.0 / 6e-6)], &[("gate", BIT)]);
    w.place_analog("v1", vsource, &["vs", "gnd"], &[NETWORK.v, 0.0]);
    w.place_with("sw", switch, &["vs", "x"], vec![real(NETWORK.ron), real(NETWORK.roff)], &[("gate", BIT)]);
    w.place_analog("r2", resistor, &["x", "gnd"], &[NETWORK.r2]);
    w.place_analog("c1", capacitor, &["x", "gnd"], &[NETWORK.c]);
}

#[test]
fn the_comparator_trips_at_its_thresholds() {
    // `A2D` of std/convert.msl: vth = 0.6 V, hyst = 50 mV, so it goes true at vth + hyst / 2 =
    // 0.625 V rising and false at vth - hyst / 2 = 0.575 V falling
    const UP: f64 = 0.6 + 0.05 / 2.0;
    const DOWN: f64 = 0.6 - 0.05 / 2.0;
    let mut w = Wire::new();
    network(&mut w);
    let a2d = Buck::comparator(&mut w.b);
    w.place_with("cmp", a2d, &["x"], vec![real(0.6), real(50e-3)], &[("d", BIT)]);
    // 8.9 us: the gate rises at 3 us, falls at 6 us and rises again at 9 us
    let seen = Built::new(w.finish()).run(8.9e-6, 1e-8);
    let gate = seen.transient("gate");
    assert_eq!(gate, vec![(3000 * NS, HIGH.to_vec()), (6000 * NS, LOW.to_vec())]);
    let (on, off) = (3e-6, Some(6e-6));
    // the output changes twice: true on the charge, false on the discharge
    let d = seen.transient("d");
    assert_eq!(d.len(), 2, "two trips: {d:?}");
    assert_eq!((d[0].1.clone(), d[1].1.clone()), (HIGH.to_vec(), LOW.to_vec()));
    for ((tick, _), threshold, what) in [(&d[0], UP, "rising"), (&d[1], DOWN, "falling")] {
        let t = *tick as f64 * 1e-15;
        // the analytic input at the instant of the trip is the threshold, under the voltage
        // contract: the trip is where the input equals vth +- hyst / 2
        let at = Point::new("comparator", "v(x)", Coordinate::Time(t));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, threshold, NETWORK.at(t, on, off));
        // and on the right side of it: the charge is above the threshold's instant, the
        // trips come in the order of the waveform
        let _ = what;
    }
    assert!(d[0].0 > 3000 * NS && d[0].0 < 6000 * NS, "the rise is during the charge: {d:?}");
    assert!(d[1].0 > 6000 * NS, "the fall is during the discharge: {d:?}");
}

/// `module Kick(in clk: Bit, out g: Bit = false)`:
///
/// ```text
/// on clk.rise() { g <- true; }
/// ```
fn kick(b: &mut IrBuilder) -> BodyId {
    const F: &str = "kick.msl";
    let body = b.body(Buck::spec("Kick", F, 1));
    b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
    b.carrier(body, Buck::carrier("g", BIT, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 30));
    let rise = Buck::rise(b, body, 0, F, 2, 8);
    let mut x = Dx::new(b, F, 2, 22);
    let yes = x.boolean(true);
    let commit = x.commit(1, yes, None);
    Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 2);
    body
}

#[test]
fn a_carrier_change_is_a_time_point_and_starts_the_response() {
    use piperine_solver::prelude::NodeIdentifier;
    let mut w = Wire::new();
    // the oscillator's first rising edge is the instant t0 = 3.0037 us, off the 1 ns grid the step
    // cap gives, so only a time point lands there; a compiled reaction commits
    // the gate high there, and the switch (closed: Ron) connects the source to the RC
    let (osc, switch, kick) = (Buck::osc(&mut w.b), Buck::switch(&mut w.b), kick(&mut w.b));
    let (vsource, resistor, capacitor) = (Devices::vsource(&mut w.b), Devices::resistor(&mut w.b), Devices::capacitor(&mut w.b));
    w.place("osc", osc, vec![real(1.0 / 6.0074e-6)], &[("clk", BIT)]);
    w.place("kick", kick, vec![], &[("clk", BIT), ("gate", BIT)]);
    w.place_analog("v1", vsource, &["vs", "gnd"], &[NETWORK.v, 0.0]);
    w.place_with("sw", switch, &["vs", "x"], vec![real(NETWORK.ron), real(NETWORK.roff)], &[("gate", BIT)]);
    w.place_analog("r2", resistor, &["x", "gnd"], &[NETWORK.r2]);
    w.place_analog("c1", capacitor, &["x", "gnd"], &[NETWORK.c]);
    let built = Built::new(w.finish());
    let x = NodeIdentifier::Anonymous(built.ir.node_at("Top.x").expect("a node x").id().index());
    // 8.9 us: the oscillator rises again at 9 us. The step is capped at 10 ns, the harness's own
    // policy: the first step after the jump is at most a tenth of the last step and of the gap to
    // the next scheduled time, floored at dt_min, so the response is as close as the 1 ns cap
    // held it (the older restart took a full controller step and needed the tighter cap; GAPS.md
    // F-B7).
    let (seen, result) = built.run_with(8.9e-6, 1e-8);
    // the gate is committed at t0 = 3.0037 us (half the period 6.0074 us), one change
    let t0_ticks = 3_003_700_000_i64;
    assert_eq!(seen.transient("gate"), vec![(t0_ticks, HIGH.to_vec())]);
    let t0 = t0_ticks as f64 * 1e-15;
    // the carrier change is a time point: the solver has an accepted solution at t0 itself
    assert!(result.iter().any(|s| (s.time() - t0).abs() < 1e-15), "a record at t0 = {t0:e} s (within a tick)");
    // from t0 the node follows the analytic response of the closed switch, and before it the
    // operating point with the switch open, at every accepted time
    let mut after = 0;
    for step in result.iter() {
        let t = step.time();
        let v = step.get_node(&x).expect("v(x)");
        let at = Point::new("switch_rc", "v(x)", Coordinate::Time(t));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, NETWORK.at(t, t0, None), v);
        if t > t0 {
            after += 1;
        }
    }
    assert!(after > 100, "the response is sampled many times after t0: {after}");
}
