//! The boundaries of a compiled mixed design on the solver (feature `ir-rust`, RS-13 AC 8 to
//! 10; checks C92 to C94). Digital to analog (T46) is here: the compiled `Switch` of the buck
//! slice between a source and an RC network, its gate committed by a compiled reaction. The
//! circuit is placed under a top and run through `Compiled::circuit`; no interpreter, no
//! bench. The expected voltage is the analytic response of the first-order network, and every
//! voltage comparison goes through `ToleranceContract::TRAN_VOLTAGE`.
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
use digital::{Buck, Dx};
use mixed::{Built, Wire};

const BIT: ValueType = ValueType::Bool;
/// A known true and a known false `Bit`: `{ v @0, known @1 }` (abi.md §3.1).
const HIGH: [u8; 2] = [1, 1];

fn real(v: f64) -> Constant {
    Constant::Real(Real::new(v))
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
    // 8.9 us: the oscillator rises again at 9 us. The step is capped at 1 ns (the first step from a
    // jump is first order, and its error grows with the square of the step). That cap is tighter
    // than the harness's own policy: with the 10 ns cap that policy gives this run, the response
    // is off by more than TRAN_VOLTAGE 5.55 ns after t0 (`ir-rust` verification F-B7, GAPS.md §6):
    // the solver's restart after a digital-to-analog jump, reported to the solver, not a defect
    // of the compiled switch, whose terms are proven bit-equal
    let (seen, result) = built.run_with(8.9e-6, 1e-9);
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
