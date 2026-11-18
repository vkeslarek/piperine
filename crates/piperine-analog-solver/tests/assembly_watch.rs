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
use piperine_analog_solver::device::{AnalogPlan, Assembly, Rising};
use system::{at_rest, at_time, close};

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, Module, Partial, Shape, cell, part, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, WatchSpec};
use piperine_abi::thunk::module;

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
fn an_instance_lists_its_terms_and_var_watches_and_reads_a_term() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let d = instance("top.d1", m.diode, vec![Discrete(0), Discrete(1), Discrete(2), at(&n[0]), Ground, Discrete(3)]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    let (names_terms, names_vars) = asm.observable_names("top.d1").expect("the instance");
    assert!(!names_terms.is_empty() && names_vars.contains(&"gd"), "{names_terms:?} {names_vars:?}");
    assert_eq!(asm.observable_names("other"), None);

    let term = names_terms[0].to_string();
    assert_eq!(asm.term_value("top.d1", &term), None, "not before an evaluation");
    for (slot, v) in [(0, 1e-14), (1, 1.0), (2, 0.0), (3, 300.15)] {
        asm.set_discrete(slot, &f64_bytes(v));
    }
    asm.assemble_dc(&[0.6], at_rest()).expect("assemble");
    assert!(asm.term_value("top.d1", &term).expect("evaluated") > 0.0);
    assert_eq!(asm.term_value("top.d1", "nope"), None);
    assert_eq!(asm.term_value("other", &term), None);
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

#[test]
fn accept_fails_with_e0509_on_an_invalid_cap() {
    // SCX-03 AC 9 (§17.2 rule 5 and its resolution): a step cap that is not finite and positive
    // fails the accept with E0509, naming the instance and the watch. W1 "b" = c2.
    let m = modules();
    let (net, n) = netlist(1, &[]);
    for bad in [0.0, -1e-9, f64::NAN, f64::INFINITY] {
        let c = instance("top.cap", m.capper, vec![Discrete(0), Discrete(1), at(&n[0])]);
        let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        asm.set_discrete(0, &f64_bytes(2e-6));
        asm.set_discrete(1, &f64_bytes(bad));
        let err = asm.accept(&[0.5], at_rest()).expect_err("an invalid cap").to_string();
        assert!(err.contains("E0509") && err.contains("top.cap") && err.contains("`b`"), "cap {bad}: {err}");
    }
    // the smallest positive cap still bounds the step
    let c = instance("top.cap", m.capper, vec![Discrete(0), Discrete(1), at(&n[0])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(2e-6));
    asm.set_discrete(1, &f64_bytes(1e-12));
    assert_eq!(asm.accept(&[0.5], at_rest()).expect("a positive cap").cap, 1e-12);
}

/// A comparator whose crossing tolerances are its own params.
/// pins: ttol=0, etol=1 (PARAM F64); inp=2, inn=3 (PORT continuous); up=4 (PRIVATE event)
/// watches: W0 CROSS "rise" → up, ttol and etol from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0←0) (0←1)
struct TolComparator;
static TOL_COMPARATOR: OnceLock<ShapeRef> = OnceLock::new();
static TOL_MODULE: OnceLock<Module> = OnceLock::new();

fn tol_comparator_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let ttol = s.pin(PinSpec::discrete("ttol", role::PARAM, &F64));
    let etol = s.pin(PinSpec::discrete("etol", role::PARAM, &F64));
    let inp = s.pin(PinSpec::continuous("inp", role::PORT));
    let inn = s.pin(PinSpec::continuous("inn", role::PORT));
    let up = s.pin(PinSpec::event("up", role::PRIVATE));
    let w0 = s.watch(WatchSpec { name: "rise", origin: "assembly_watch.rs", kind: watch::CROSS, layout: &F64, event: up, ttol, etol });
    s.function(FunctionSpec {
        name: "e",
        reads: vec![Cell { kind: cell::PIN, index: inp, part: part::VALUE }, Cell { kind: cell::PIN, index: inn, part: part::VALUE }],
        writes: vec![Cell { kind: cell::WATCH, index: w0, part: part::VALUE }],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for TolComparator {
    fn prototype() -> &'static Shape<'static> {
        TOL_COMPARATOR.get_or_init(tol_comparator_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TolComparator, Arc::clone(TOL_COMPARATOR.get_or_init(tol_comparator_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn tol_comparator() -> &'static Module {
    TOL_MODULE.get_or_init(|| module::<TolComparator>("TolComparator"))
}

#[test]
fn accept_fails_with_e0510_on_an_invalid_tolerance() {
    // SCX-03 AC 8 (§17.3 resolution): a `ttol` or `etol` that is not finite and positive fails
    // the accept with E0510, naming the instance, the watch and which tolerance
    let (net, n) = netlist(2, &[]);
    for (which, slot) in [("ttol", 0), ("etol", 1)] {
        for bad in [0.0, -1e-12, f64::NAN, f64::INFINITY] {
            let c = instance("top.cmp", tol_comparator(), vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1]), Absent]);
            let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
            asm.set_discrete(0, &f64_bytes(1e-12));
            asm.set_discrete(1, &f64_bytes(1e-6));
            asm.set_discrete(slot, &f64_bytes(bad));
            let err = asm.accept(&[0.2, 0.0], at_rest()).expect_err("an invalid tolerance").to_string();
            assert!(
                err.contains("E0510") && err.contains("top.cmp") && err.contains("`rise`") && err.contains(which),
                "{which} = {bad}: {err}"
            );
        }
    }
    // positive and finite tolerances accept
    let c = instance("top.cmp", tol_comparator(), vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1]), Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(1e-12));
    asm.set_discrete(1, &f64_bytes(1e-6));
    asm.accept(&[0.2, 0.0], at_rest()).expect("valid tolerances");
}

#[test]
fn rising_reports_a_sign_change_without_accepting() {
    // SCX-01 (§17.3): the locator reads a solution before it is accepted. W0 = inp − inn "up"
    // goes from −0.1 to 0.2: rising, with both values and the DEFAULT tolerances; W1 = −W0 falls:
    // not rising. Nothing is accepted: asked twice, it answers the same, and the accept at 0.2
    // still fires `up`
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let c = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(0), Absent, Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assert!(asm.has_crossings());
    assert!(asm.rising(&[-0.1, 0.0], at_time(0.0)).expect("rising").is_empty(), "no accepted solution yet");
    asm.accept(&[-0.1, 0.0], at_time(0.0)).expect("accept");
    let up = Rising { instance: 0, watch: 0, before: -0.1, now: 0.2, ttol: None, etol: None };
    assert_eq!(asm.rising(&[0.2, 0.0], at_time(1e-6)).expect("rising"), vec![up.clone()]);
    assert_eq!(asm.rising(&[0.2, 0.0], at_time(1e-6)).expect("rising"), vec![up], "nothing moved");
    assert!(asm.rising(&[-0.05, 0.0], at_time(1e-6)).expect("rising").is_empty(), "still ≤ 0");
    assert_eq!(asm.accept(&[0.2, 0.0], at_time(1e-6)).expect("accept").fired, vec![(0, 3)], "the accept still fires up");

    // a watch's own tolerances come with it
    let c = instance("top.cmp", tol_comparator(), vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1]), Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(1e-12));
    asm.set_discrete(1, &f64_bytes(1e-6));
    asm.accept(&[0.0, 0.1], at_time(0.0)).expect("accept");
    let got = asm.rising(&[0.3, 0.1], at_time(1e-6)).expect("rising");
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].before, got[0].ttol, got[0].etol), (-0.1, Some(1e-12), Some(1e-6)));
    close(got[0].now, 0.2, 1e-15, "e = inp − inn");

    // a design with no CROSS watch has none
    let r = instance("r1", m.resistor, vec![Discrete(0), at(&n[0]), Ground]);
    assert!(!Assembly::new(AnalogPlan::bind(vec![r], &net).expect("bind")).has_crossings());
}

#[test]
fn a_nan_cross_watch_is_unknown_and_never_fires() {
    // FIX-05 AC 3 (abi.md A4): an unknown value fires neither at its solution nor at the next.
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let c = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(0), Absent, Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    // NaN first, then a positive value: nothing before, so nothing fires.
    let acc = asm.accept(&[f64::NAN, 0.0], at_time(0.0)).expect("a NaN watch is accepted");
    assert_eq!(acc.fired, vec![], "NaN fires nothing");
    let acc = asm.accept(&[0.2, 0.0], at_time(1e-6)).expect("accept");
    assert_eq!(acc.fired, vec![], "NaN then positive: the next solution does not fire either");
    // negative, NaN, positive: the unknown between them fires at neither step.
    let c = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(0), Absent, Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assert_eq!(asm.accept(&[-0.1, 0.0], at_time(0.0)).expect("accept").fired, vec![]);
    assert_eq!(asm.accept(&[f64::NAN, 0.0], at_time(1e-6)).expect("accept").fired, vec![], "at the NaN");
    assert_eq!(asm.accept(&[0.3, 0.0], at_time(2e-6)).expect("accept").fired, vec![], "and at the next");
    // known again: the other watch (W1 = -W0) rises as usual, from -0.3 to 0.1.
    assert_eq!(asm.accept(&[-0.1, 0.0], at_time(3e-6)).expect("accept").fired, vec![(0, 4)]);
}

#[test]
fn an_infinite_cross_watch_fails_the_solve() {
    // FIX-05 AC 4: plus or minus infinity fails the solve naming the watch (abi.md A4).
    let m = modules();
    let (net, n) = netlist(2, &[]);
    for inf in [f64::INFINITY, f64::NEG_INFINITY] {
        let c = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(0), Absent, Absent]);
        let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        let err = asm.accept(&[inf, 0.0], at_time(0.0)).expect_err("an infinite watch fails").to_string();
        assert!(err.contains("watch `up`") && err.contains("not finite") && err.contains(&format!("{inf}")), "{err}");
    }
}

#[test]
fn a_tolerance_the_discrete_side_has_not_written_is_the_default() {
    // §17.3 rule 5: at the operating point's first solve the tolerance pins have not run yet, so
    // they are unknown, not zero: no E0510 and the default tolerances. A pin written with 0 is
    // still E0510 (`accept_fails_with_e0510_on_an_invalid_tolerance`).
    let (net, n) = netlist(2, &[]);
    let c = instance("top.cmp", tol_comparator(), vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1]), Absent]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    asm.accept(&[0.0, 0.1], at_rest()).expect("unwritten tolerances are unknown, not invalid");
    let got = asm.rising(&[0.3, 0.1], at_time(1e-6)).expect("rising");
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].ttol, got[0].etol), (None, None), "the default tolerances");
}
