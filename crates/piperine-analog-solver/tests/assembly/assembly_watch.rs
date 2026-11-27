//! Watches and the accept step (feature `solver-abi-analog`, SAN-10, T9):
//! a `VAR` watch is readable at an accepted solution (C16); a `CROSS` watch
//! fires on a rising sign change between accepted solutions only (C17);
//! `accept` reports the smallest `CAP` (C18); a `TRACE` record keeps its
//! Newton iteration and survives a restore (C19); `samples` reads the
//! continuous pins reactions read.

use piperine_device_fixtures::analog::{FixtureModules};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{AssemblySolves};
use piperine_device_fixtures::examples::{K_B, Q};
use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly, Rising};

use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, Module, Partial, Shape, cell, part, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, WatchSpec};
use piperine_abi::thunk::module;

#[test]
fn a_var_watch_is_readable_at_an_accepted_solution() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let d = Circuits::instance("top.d1", m.diode, vec![Discrete(0), Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground, Discrete(3)]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    let (is, t) = (1e-14, 300.15);
    for (slot, v) in [(0, is), (1, 1.0), (2, 0.0), (3, t)] {
        assembly.set_discrete(slot, &Circuits::real_bytes(v));
    }
    assembly.assemble_dc(&[0.6], AssemblySolves::at_rest()).expect("assemble");
    assert_eq!(assembly.var("top.d1", "gd"), None, "not before an accepted solution");

    assembly.accept(&[0.6], AssemblySolves::at_rest()).expect("accept");
    let nvt = K_B * t / Q;
    let gd = is * (0.6 / nvt).exp() / nvt;
    AssemblySolves::close(Circuits::real_of(assembly.var("top.d1", "gd").expect("gd")), gd, 1e-14, "gd at the accepted vd");
    assert_eq!(assembly.var("top.d1", "nope"), None);
    assert_eq!(assembly.var("other", "gd"), None);
}

#[test]
fn an_instance_lists_its_terms_and_var_watches_and_reads_a_term() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let d = Circuits::instance("top.d1", m.diode, vec![Discrete(0), Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground, Discrete(3)]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    let (names_terms, names_vars) = assembly.observable_names("top.d1").expect("the instance");
    assert!(!names_terms.is_empty() && names_vars.contains(&"gd"), "{names_terms:?} {names_vars:?}");
    assert_eq!(assembly.observable_names("other"), None);

    let term = names_terms[0].to_string();
    assert_eq!(assembly.term_value("top.d1", &term), None, "not before an evaluation");
    for (slot, v) in [(0, 1e-14), (1, 1.0), (2, 0.0), (3, 300.15)] {
        assembly.set_discrete(slot, &Circuits::real_bytes(v));
    }
    assembly.assemble_dc(&[0.6], AssemblySolves::at_rest()).expect("assemble");
    assert!(assembly.term_value("top.d1", &term).expect("evaluated") > 0.0);
    assert_eq!(assembly.term_value("top.d1", "nope"), None);
    assert_eq!(assembly.term_value("other", &term), None);
}

#[test]
fn a_cross_watch_fires_on_a_rising_sign_change() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let c = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(0), Absent, Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
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
        let acc = assembly.accept(&[e, 0.0], AssemblySolves::at_time(k as f64 * 1e-6)).expect("accept");
        assert_eq!(acc.fired, fired, "e = {e}: {why}");
    }
}

#[test]
fn accept_reports_the_smallest_cap() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let c = Circuits::instance("cap", m.cap_watch_device, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(2e-6));
    assembly.set_discrete(1, &Circuits::real_bytes(5e-6));
    assert_eq!(assembly.accept(&[0.5], AssemblySolves::at_rest()).expect("accept").cap, 3e-6, "min(2e-6·1.5, 5e-6)");
    assert_eq!(assembly.accept(&[2.0], AssemblySolves::at_rest()).expect("accept").cap, 5e-6, "min(6e-6, 5e-6)");

    let r = Circuits::instance("r1", m.resistor, vec![Discrete(0), Circuits::at(&n[0]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![r], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1e3));
    assert_eq!(assembly.accept(&[0.5], AssemblySolves::at_rest()).expect("accept").cap, f64::INFINITY, "no CAP watch");
}

#[test]
fn a_trace_record_survives_a_rejected_step() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let t = Circuits::instance("top.tr", m.tracer, vec![Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![t], &net).expect("bind"));
    let snap = assembly.snapshot();
    assembly.assemble_dc(&[0.7], AssemblySolves::at_rest()).expect("iteration 0");
    assembly.end_iteration();
    assembly.assemble_dc(&[0.9], AssemblySolves::at_rest()).expect("iteration 1");
    assembly.end_iteration();
    assembly.assemble_dc(&[0.2], AssemblySolves::at_rest()).expect("iteration 2: not written");
    assembly.restore(&snap);

    let recs = assembly.drain_traces();
    let got: Vec<(String, String, u64, f64, u8)> =
        recs.iter().map(|r| (r.instance.clone(), r.watch.clone(), r.iteration, Circuits::real_of(&r.bytes[8..]), r.bytes[0])).collect();
    assert_eq!(
        got,
        vec![("top.tr".into(), "hi".into(), 0, 0.7, 1), ("top.tr".into(), "hi".into(), 1, 0.9, 1)],
        "each written record, with its iteration, after the restore"
    );
    assert!(assembly.drain_traces().is_empty(), "drained once");
}

#[test]
fn samples_are_the_continuous_pins_reactions_read() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(3, &[]);
    let s1 = Circuits::instance("sh1", m.sample_hold, vec![Circuits::at(&n[2]), Absent, Discrete(0)]);
    let r = Circuits::instance("r1", m.resistor, vec![Discrete(1), Circuits::at(&n[0]), Ground]);
    let s2 = Circuits::instance("sh2", m.sample_hold, vec![Ground, Absent, Discrete(2)]);
    let s3 = Circuits::instance("sh3", m.sample_hold, vec![Circuits::at(&n[1]), Absent, Discrete(3)]);
    let assembly = Assembly::new(AnalogPlan::bind(vec![s1, r, s2, s3], &net).expect("bind"));
    assert_eq!(assembly.samples(&[0.1, 0.2, 0.3]), vec![0.3, 0.0, 0.2]);
}

#[test]
fn accept_fails_with_e0509_on_an_invalid_cap() {
    // SCX-03 AC 9 (§17.2 rule 5 and its resolution): a step cap that is not finite and positive
    // fails the accept with E0509, naming the instance and the watch. W1 "b" = c2.
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    // (`+∞` is no longer one of them: it caps nothing, see `a_cap_of_infinity_caps_nothing`)
    for bad in [0.0, -1e-9, f64::NAN] {
        let c = Circuits::instance("top.cap", m.cap_watch_device, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(2e-6));
        assembly.set_discrete(1, &Circuits::real_bytes(bad));
        let err = assembly.accept(&[0.5], AssemblySolves::at_rest()).expect_err("an invalid cap").to_string();
        assert!(err.contains("E0509") && err.contains("top.cap") && err.contains("`b`"), "cap {bad}: {err}");
    }
    // the smallest positive cap still bounds the step
    let c = Circuits::instance("top.cap", m.cap_watch_device, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(2e-6));
    assembly.set_discrete(1, &Circuits::real_bytes(1e-12));
    assert_eq!(assembly.accept(&[0.5], AssemblySolves::at_rest()).expect("a positive cap").cap, 1e-12);
}

#[test]
fn a_cap_of_infinity_caps_nothing() {
    // abi.md, the `CAP` row: a watch of `+∞` is a `bound_step` not executed at this solution, so it
    // caps nothing and fails nothing. The capper's watches are W0 = a·(1 + v) and W1 = b (pins 0
    // and 1): with `a = 2e-6`, `b = +∞` and `v = 0.5` only W0 binds, 2e-6·1.5 = 3e-6; with both
    // infinite no watch binds and the cap stays the identity of the minimum, `+∞`.
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let c = Circuits::instance("top.cap", m.cap_watch_device, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(2e-6));
    assembly.set_discrete(1, &Circuits::real_bytes(f64::INFINITY));
    assert_eq!(assembly.accept(&[0.5], AssemblySolves::at_rest()).expect("an infinite cap fails nothing").cap, 3e-6, "min(2e-6·1.5, +∞)");
    assembly.set_discrete(0, &Circuits::real_bytes(f64::INFINITY));
    assert_eq!(assembly.accept(&[0.5], AssemblySolves::at_rest()).expect("no watch binds").cap, f64::INFINITY, "min(+∞·1.5, +∞)");
}

#[test]
fn a_cap_of_minus_infinity_or_nan_is_e0509() {
    // Only `+∞` caps nothing: `-∞`, NaN, zero and a negative value stay `E0509` (§17.2 rule 5),
    // naming the instance and the watch.
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    for bad in [f64::NEG_INFINITY, f64::NAN, 0.0, -1e-9] {
        let c = Circuits::instance("top.cap", m.cap_watch_device, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(2e-6));
        assembly.set_discrete(1, &Circuits::real_bytes(bad));
        let err = assembly.accept(&[0.5], AssemblySolves::at_rest()).expect_err("an invalid cap").to_string();
        assert!(err.contains("E0509") && err.contains("top.cap") && err.contains("`b`"), "cap {bad}: {err}");
    }
}

/// A comparator whose crossing tolerances are its own params.
/// pins: ttol=0, etol=1 (PARAM F64); inp=2, inn=3 (PORT continuous); up=4 (PRIVATE event)
/// watches: W0 CROSS "rise" → up, ttol and etol from pins 0 and 1
/// fn 0: reads [inp, inn] writes [W0] partials (0←0) (0←1)
struct TolComparator;
static TOL_COMPARATOR: OnceLock<ShapeRef> = OnceLock::new();
static TOL_MODULE: OnceLock<Module> = OnceLock::new();

impl Device for TolComparator {
    fn prototype() -> &'static Shape<'static> {
        TOL_COMPARATOR.get_or_init(WatchCircuits::tol_comparator_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TolComparator, Arc::clone(TOL_COMPARATOR.get_or_init(WatchCircuits::tol_comparator_shape))))
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

#[test]
fn accept_fails_with_e0510_on_an_invalid_tolerance() {
    // SCX-03 AC 8 (§17.3 resolution): a `ttol` or `etol` that is not finite and positive fails
    // the accept with E0510, naming the instance, the watch and which tolerance
    let (net, n) = Circuits::netlist(2, &[]);
    for (which, slot) in [("ttol", 0), ("etol", 1)] {
        for bad in [0.0, -1e-12, f64::NAN, f64::INFINITY] {
            let c = Circuits::instance("top.cmp", WatchCircuits::tol_comparator(), vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1]), Absent]);
            let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
            assembly.set_discrete(0, &Circuits::real_bytes(1e-12));
            assembly.set_discrete(1, &Circuits::real_bytes(1e-6));
            assembly.set_discrete(slot, &Circuits::real_bytes(bad));
            let err = assembly.accept(&[0.2, 0.0], AssemblySolves::at_rest()).expect_err("an invalid tolerance").to_string();
            assert!(
                err.contains("E0510") && err.contains("top.cmp") && err.contains("`rise`") && err.contains(which),
                "{which} = {bad}: {err}"
            );
        }
    }
    // positive and finite tolerances accept
    let c = Circuits::instance("top.cmp", WatchCircuits::tol_comparator(), vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1]), Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1e-12));
    assembly.set_discrete(1, &Circuits::real_bytes(1e-6));
    assembly.accept(&[0.2, 0.0], AssemblySolves::at_rest()).expect("valid tolerances");
}

#[test]
fn rising_reports_a_sign_change_without_accepting() {
    // SCX-01 (§17.3): the locator reads a solution before it is accepted. W0 = inp − inn "up"
    // goes from −0.1 to 0.2: rising, with both values and the DEFAULT tolerances; W1 = −W0 falls:
    // not rising. Nothing is accepted: asked twice, it answers the same, and the accept at 0.2
    // still fires `up`
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let c = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(0), Absent, Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assert!(assembly.has_crossings());
    assert!(assembly.rising(&[-0.1, 0.0], AssemblySolves::at_time(0.0)).expect("rising").is_empty(), "no accepted solution yet");
    assembly.accept(&[-0.1, 0.0], AssemblySolves::at_time(0.0)).expect("accept");
    let up = Rising { instance: 0, watch: 0, before: -0.1, now: 0.2, ttol: None, etol: None };
    assert_eq!(assembly.rising(&[0.2, 0.0], AssemblySolves::at_time(1e-6)).expect("rising"), vec![up.clone()]);
    assert_eq!(assembly.rising(&[0.2, 0.0], AssemblySolves::at_time(1e-6)).expect("rising"), vec![up], "nothing moved");
    assert!(assembly.rising(&[-0.05, 0.0], AssemblySolves::at_time(1e-6)).expect("rising").is_empty(), "still ≤ 0");
    assert_eq!(assembly.accept(&[0.2, 0.0], AssemblySolves::at_time(1e-6)).expect("accept").fired, vec![(0, 3)], "the accept still fires up");

    // a watch's own tolerances come with it
    let c = Circuits::instance("top.cmp", WatchCircuits::tol_comparator(), vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1]), Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1e-12));
    assembly.set_discrete(1, &Circuits::real_bytes(1e-6));
    assembly.accept(&[0.0, 0.1], AssemblySolves::at_time(0.0)).expect("accept");
    let got = assembly.rising(&[0.3, 0.1], AssemblySolves::at_time(1e-6)).expect("rising");
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].before, got[0].ttol, got[0].etol), (-0.1, Some(1e-12), Some(1e-6)));
    AssemblySolves::close(got[0].now, 0.2, 1e-15, "e = inp − inn");

    // a design with no CROSS watch has none
    let r = Circuits::instance("r1", m.resistor, vec![Discrete(0), Circuits::at(&n[0]), Ground]);
    assert!(!Assembly::new(AnalogPlan::bind(vec![r], &net).expect("bind")).has_crossings());
}

#[test]
fn a_nan_cross_watch_is_unknown_and_never_fires() {
    // FIX-05 AC 3 (abi.md A4): an unknown value fires neither at its solution nor at the next.
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let c = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(0), Absent, Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    // NaN first, then a positive value: nothing before, so nothing fires.
    let acc = assembly.accept(&[f64::NAN, 0.0], AssemblySolves::at_time(0.0)).expect("a NaN watch is accepted");
    assert_eq!(acc.fired, vec![], "NaN fires nothing");
    let acc = assembly.accept(&[0.2, 0.0], AssemblySolves::at_time(1e-6)).expect("accept");
    assert_eq!(acc.fired, vec![], "NaN then positive: the next solution does not fire either");
    // negative, NaN, positive: the unknown between them fires at neither step.
    let c = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(0), Absent, Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assert_eq!(assembly.accept(&[-0.1, 0.0], AssemblySolves::at_time(0.0)).expect("accept").fired, vec![]);
    assert_eq!(assembly.accept(&[f64::NAN, 0.0], AssemblySolves::at_time(1e-6)).expect("accept").fired, vec![], "at the NaN");
    assert_eq!(assembly.accept(&[0.3, 0.0], AssemblySolves::at_time(2e-6)).expect("accept").fired, vec![], "and at the next");
    // known again: the other watch (W1 = -W0) rises as usual, from -0.3 to 0.1.
    assert_eq!(assembly.accept(&[-0.1, 0.0], AssemblySolves::at_time(3e-6)).expect("accept").fired, vec![(0, 4)]);
}

#[test]
fn an_infinite_cross_watch_fails_the_solve() {
    // FIX-05 AC 4: plus or minus infinity fails the solve naming the watch (abi.md A4).
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    for inf in [f64::INFINITY, f64::NEG_INFINITY] {
        let c = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(0), Absent, Absent]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
        let err = assembly.accept(&[inf, 0.0], AssemblySolves::at_time(0.0)).expect_err("an infinite watch fails").to_string();
        assert!(err.contains("watch `up`") && err.contains("not finite") && err.contains(&format!("{inf}")), "{err}");
    }
}

#[test]
fn a_tolerance_the_discrete_side_has_not_written_is_the_default() {
    // §17.3 rule 5: at the operating point's first solve the tolerance pins have not run yet, so
    // they are unknown, not zero: no E0510 and the default tolerances. A pin written with 0 is
    // still E0510 (`accept_fails_with_e0510_on_an_invalid_tolerance`).
    let (net, n) = Circuits::netlist(2, &[]);
    let c = Circuits::instance("top.cmp", WatchCircuits::tol_comparator(), vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1]), Absent]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![c], &net).expect("bind"));
    assembly.accept(&[0.0, 0.1], AssemblySolves::at_rest()).expect("unwritten tolerances are unknown, not invalid");
    let got = assembly.rising(&[0.3, 0.1], AssemblySolves::at_time(1e-6)).expect("rising");
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].ttol, got[0].etol), (None, None), "the default tolerances");
}

/// The circuits the watch tests build.
pub struct WatchCircuits;

impl WatchCircuits {
    /// The tol comparator shape helper `tol_comparator_shape`.
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

    /// The tol comparator helper `tol_comparator`.
    fn tol_comparator() -> &'static Module {
        TOL_MODULE.get_or_init(|| module::<TolComparator>("TolComparator"))
    }
}
