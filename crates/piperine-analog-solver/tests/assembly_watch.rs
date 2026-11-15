//! Watches and the accept step (feature `solver-abi-analog`, SAN-10, T9):
//! a `VAR` watch is readable at an accepted solution (C16); a `CROSS` watch
//! fires on a rising sign change between accepted solutions only (C17);
//! `accept` reports the smallest `CAP` (C18); a `TRACE` record keeps its
//! Newton iteration and survives a restore (C19); `samples` reads the
//! continuous pins reactions read.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use circuit::{at, f64_bytes, f64_of, instance, netlist};
use devices::{K_B, Q, modules};
use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use system::{at_rest, at_time, close};

#[test]
fn a_var_watch_is_readable_at_an_accepted_solution() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let d = instance("top.d1", m.diode, vec![Discrete(0), Discrete(1), Discrete(2), at(&n[0]), Ground, Discrete(3)]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    let (is, t) = (1e-14, 300.15);
    for (slot, v) in [(0, is), (1, 1.0), (2, 0.0), (3, t)] {
        asm.set_discrete(slot, &f64_bytes(v));
    }
    asm.assemble_dc(&[0.6], at_rest()).expect("assemble");
    assert_eq!(asm.var("top.d1", "gd"), None, "not before an accepted solution");

    asm.accept(&[0.6], at_rest()).expect("accept");
    let nvt = K_B * t / Q;
    let gd = is * (0.6 / nvt).exp() / nvt;
    close(f64_of(asm.var("top.d1", "gd").expect("gd")), gd, 1e-14, "gd at the accepted vd");
    assert_eq!(asm.var("top.d1", "nope"), None);
    assert_eq!(asm.var("other", "gd"), None);
}

#[test]
fn a_cross_watch_fires_on_a_rising_sign_change() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let c = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(0), Absent, Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    // W0 = inp − inn fires `up` (pin 3); W1 = −W0 fires `down` (pin 4).
    let (up, down) = (vec![(0, 3)], vec![(0, 4)]);
    let steps: [(f64, Vec<(usize, u32)>, &str); 6] = [
        (-0.1, vec![], "no previous accepted value"),
        (0.0, vec![], "≤ 0 to 0 is no rise"),
        (0.2, up.clone(), "0 to > 0 rises"),
        (0.3, vec![], "> 0 to > 0"),
        (-0.1, down, "W0 falls (no fire); W1 rises"),
        (0.4, up, "≤ 0 to > 0 rises again"),
    ];
    for (k, (e, fired, why)) in steps.into_iter().enumerate() {
        let acc = asm.accept(&[e, 0.0], at_time(k as f64 * 1e-6)).expect("accept");
        assert_eq!(acc.fired, fired, "e = {e}: {why}");
    }
}

#[test]
fn accept_reports_the_smallest_cap() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let c = instance("cap", m.capper, vec![Discrete(0), Discrete(1), at(&n[0])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(2e-6));
    asm.set_discrete(1, &f64_bytes(5e-6));
    assert_eq!(asm.accept(&[0.5], at_rest()).expect("accept").cap, 3e-6, "min(2e-6·1.5, 5e-6)");
    assert_eq!(asm.accept(&[2.0], at_rest()).expect("accept").cap, 5e-6, "min(6e-6, 5e-6)");

    let r = instance("r1", m.resistor, vec![Discrete(0), at(&n[0]), Ground]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![r], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(1e3));
    assert_eq!(asm.accept(&[0.5], at_rest()).expect("accept").cap, f64::INFINITY, "no CAP watch");
}

#[test]
fn a_trace_record_survives_a_rejected_step() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let t = instance("top.tr", m.tracer, vec![at(&n[0])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![t], &net).expect("bind"));
    let snap = asm.snapshot();
    asm.assemble_dc(&[0.7], at_rest()).expect("iteration 0");
    asm.end_iteration();
    asm.assemble_dc(&[0.9], at_rest()).expect("iteration 1");
    asm.end_iteration();
    asm.assemble_dc(&[0.2], at_rest()).expect("iteration 2: not written");
    asm.restore(&snap);

    let recs = asm.drain_traces();
    let got: Vec<(String, String, u64, f64, u8)> =
        recs.iter().map(|r| (r.instance.clone(), r.watch.clone(), r.iteration, f64_of(&r.bytes), r.bytes[8])).collect();
    assert_eq!(
        got,
        vec![("top.tr".into(), "hi".into(), 0, 0.7, 1), ("top.tr".into(), "hi".into(), 1, 0.9, 1)],
        "each written record, with its iteration, after the restore"
    );
    assert!(asm.drain_traces().is_empty(), "drained once");
}

#[test]
fn samples_are_the_continuous_pins_reactions_read() {
    let m = modules();
    let (net, n) = netlist(3, &[]);
    let s1 = instance("sh1", m.sample_hold, vec![at(&n[2]), Absent, Discrete(0)]);
    let r = instance("r1", m.resistor, vec![Discrete(1), at(&n[0]), Ground]);
    let s2 = instance("sh2", m.sample_hold, vec![Ground, Absent, Discrete(2)]);
    let s3 = instance("sh3", m.sample_hold, vec![at(&n[1]), Absent, Discrete(3)]);
    let asm = Assembly::new(AnalogPlan::bind(vec![s1, r, s2, s3], &net).expect("bind"));
    assert_eq!(asm.samples(&[0.1, 0.2, 0.3]), vec![0.3, 0.0, 0.2]);
}
