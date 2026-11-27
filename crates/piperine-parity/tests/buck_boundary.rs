//! The analog to digital boundary from MSL source (feature `msl-digital`, T50 and T66; checks
//! C133, C135, C136, C179, DIG-14 AC 6, 8 and 9): the buck package's `Adc` on five held voltages
//! and the stdlib `A2D` comparator on an RC network driven by the stdlib `D2A`, elaborated from
//! `tests/buck/boundary.msl`. Expected
//! values are written from `examples/buck/SPEC.md` §4.1 (the ADC) and from the analytic response
//! of the RC network; every voltage comparison goes through `ToleranceContract::TRAN_VOLTAGE`,
//! and no time tolerance is written anywhere.
#![cfg(unix)]

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{HIGH, LOW, build, series, transient, with_buck};

/// One nanosecond in ticks (the solver's one femtosecond).
const NS: i64 = 1_000_000;

fn project() -> Project {
    with_buck("boundary.msl")
}

#[test]
fn the_adc_commits_the_rounded_code_after_tconv() {
    // SPEC §4.1: 10-bit, Vref 1.6 V, mid-tread. Five held voltages: 0, one LSB, mid-scale, full
    // scale and above full scale
    const VREF: f64 = 1.6;
    let lsb = VREF / 1024.0;
    let volts = [0.0, lsb, VREF / 2.0, VREF, 2.0];
    let expected_code = |v: f64| -> u16 { (v / VREF * 1024.0 + 0.5).floor().clamp(0.0, 1023.0) as u16 };
    assert_eq!(volts.map(expected_code), [0, 1, 512, 1023, 1023]);

    // 1.4 us: the first conversion ends at 1.0 us and the second sample is not reached
    let seen = build(&project(), "AdcBench").run(1.4e-6, 1e-8);
    let sample = 500 * NS;
    assert_eq!(transient(&seen, "AdcBench.sample")[0], (sample, HIGH.to_vec()), "the strobe rises at 0.5 us");
    for (i, v) in volts.iter().enumerate() {
        let code = expected_code(*v);
        // the code appears after `tconv` (500 ns) from the rising edge, and not before: a code of
        // 0 is the value it already holds, so it commits nothing visible
        let changes = transient(&seen, &format!("AdcBench.code{i}"));
        let wanted: Vec<(i64, Vec<u8>)> =
            if code == 0 { vec![] } else { vec![(sample + 500 * NS, code.to_le_bytes().to_vec())] };
        assert_eq!(changes, wanted, "code of {v} V");
        // `done` is true from `tconv` for `tstrobe` (5 ns)
        assert_eq!(
            transient(&seen, &format!("AdcBench.done{i}")),
            vec![(sample + 500 * NS, HIGH.to_vec()), (sample + 505 * NS, LOW.to_vec())],
            "done at {v} V"
        );
    }
}

/// The RC network of `ComparatorBench`: the stdlib `D2A` puts the rail `V` through `rout` into
/// `x`, `r2` and `c` from `x` to ground, the rail connected while the gate is high and grounded
/// while low (the `D2A` ramp of 20 ps is far below the time constant), the gate
/// rising at `on` and falling at `off`. With the drive voltage `u` (V or 0) the node follows
/// `v_inf + (v0 - v_inf) * exp(-(t - t_edge) / tau)`, `v_inf = u * r2 / (rout + r2)`,
/// `tau = c * rout * r2 / (rout + r2)`; the operating point (gate low, capacitor open) is 0 V.
struct Network {
    v: f64,
    rout: f64,
    r2: f64,
    c: f64,
    on: f64,
    off: f64,
}

impl Network {
    fn at(&self, t: f64) -> f64 {
        let tau = self.c * self.rout * self.r2 / (self.rout + self.r2);
        let vinf = self.v * self.r2 / (self.rout + self.r2);
        if t <= self.on {
            0.0
        } else if t <= self.off {
            vinf * (1.0 - (-(t - self.on) / tau).exp())
        } else {
            vinf * (1.0 - (-(self.off - self.on) / tau).exp()) * (-(t - self.off) / tau).exp()
        }
    }
}

#[test]
fn the_comparator_trips_at_its_thresholds() {
    // `A2D` of std/convert.msl: vth = 0.6 V, hyst = 50 mV, so it goes true at vth + hyst / 2 =
    // 0.625 V rising and false at vth - hyst / 2 = 0.575 V falling
    const UP: f64 = 0.6 + 0.05 / 2.0;
    const DOWN: f64 = 0.6 - 0.05 / 2.0;
    let net = Network { v: 1.0, rout: 1e3, r2: 9e3, c: 0.5e-9, on: 3e-6, off: 6e-6 };
    // 8.9 us: the gate rises at 3 us, falls at 6 us and rises again at 9 us
    let seen = build(&project(), "ComparatorBench").run(8.9e-6, 1e-8);
    let gate = transient(&seen, "ComparatorBench.gate");
    assert_eq!(gate, vec![(3000 * NS, HIGH.to_vec()), (6000 * NS, LOW.to_vec())]);
    // the output changes twice: true on the charge, false on the discharge
    let d = transient(&seen, "ComparatorBench.d");
    assert_eq!(d.len(), 2, "two trips: {d:?}");
    assert_eq!((d[0].1.clone(), d[1].1.clone()), (HIGH.to_vec(), LOW.to_vec()));
    for ((tick, _), threshold) in [(&d[0], UP), (&d[1], DOWN)] {
        let t = *tick as f64 * 1e-15;
        // the analytic input at the instant of the trip is the threshold, under the voltage
        // contract: the trip is where the input equals vth +- hyst / 2
        let at = Point::new("comparator", "v(x)", Coordinate::Time(t));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, threshold, net.at(t));
    }
    assert!(d[0].0 > 3000 * NS && d[0].0 < 6000 * NS, "the rise is during the charge: {d:?}");
    assert!(d[1].0 > 6000 * NS, "the fall is during the discharge: {d:?}");
}

#[test]
fn the_comparator_is_right_at_time_zero() {
    // the input is 1 V, above vth + hyst / 2 = 0.625 V, at the operating point: `d` is true at
    // time 0 (the comparator's `d` starts false, so the operating point's initial value makes
    // the change), and nothing changes it afterwards
    let seen = build(&project(), "HeldHighBench").run(2e-6, 1e-8);
    let all = series(&seen, "HeldHighBench.d");
    assert_eq!(all.last().map(|(t, b)| (*t, b.clone())), Some((0, HIGH.to_vec())), "d is true at time 0: {all:?}");
    assert_eq!(transient(&seen, "HeldHighBench.d"), vec![], "no change of d after time 0");
}
