//! The analog half of bind (feature `solver-abi-analog`, SAN-01, SAN-02,
//! T1): `AnalogPlan::bind` takes analog functions only (F2), resolves term
//! targets to rows (a `GROUND` side has none), names the entry of a broken
//! rule, and builds the matrix pattern as terms × partials, through a
//! `LIMIT`'s `PROPOSED` partials (C13). It also holds the fixture devices to
//! the contract: every shape validates and every declared partial agrees with
//! the finite-difference verifier of abi.md §8.

use piperine_device_fixtures::analog::{FixtureModules};
use piperine_device_fixtures::assembly::{Circuits, AssemblySolves};
use std::collections::HashSet;

use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_abi::validate::{Rule, Validator};
use piperine_abi::verify::Verdict;
use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use piperine_analog_solver::netlist::AnalogReference;

#[test]
fn the_pattern_is_terms_times_partials() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(4, &[]);
    // The diode reaches its pins only through the limit's PROPOSED partials.
    let diode = Circuits::instance(
        "d1",
        m.diode,
        vec![Discrete(0), Discrete(1), Discrete(2), Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(3)],
    );
    let plan = AnalogPlan::bind(vec![diode], &net).expect("bind");
    let (a, k) = (PlanCircuits::idx(&n[0]), PlanCircuits::idx(&n[1]));
    let want = HashSet::from([(a, a), (a, k), (k, a), (k, k)]);
    assert_eq!(PlanCircuits::positions(&plan), want);
    let mut assembly = Assembly::new(plan);
    for (slot, value) in [(0, 1e-14), (1, 1.0), (2, 1e-12), (3, 300.15)] {
        assembly.set_discrete(slot, &Circuits::real_bytes(value));
    }
    for bias in [0.0, 0.3, 2.0] {
        let stamps = assembly.assemble_dc(&[bias, 0.0, 0.0, 0.0], AssemblySolves::at_rest()).expect("assemble");
        assert_eq!(AssemblySolves::matrix_positions(&stamps), want, "the diode at bias {bias}");
        assembly.end_iteration();
    }

    // The cubic's partials vanish at zero bias; its positions are declared,
    // and an equilibrium solve stamps every one of them at any bias.
    let cubic = Circuits::instance("c1", m.cubic, vec![Discrete(0), Circuits::at(&n[2]), Circuits::at(&n[3])]);
    let plan = AnalogPlan::bind(vec![cubic], &net).expect("bind");
    let (p, q) = (PlanCircuits::idx(&n[2]), PlanCircuits::idx(&n[3]));
    let want = HashSet::from([(p, p), (p, q), (q, p), (q, q)]);
    assert_eq!(PlanCircuits::positions(&plan), want);
    let mut assembly = Assembly::new(plan);
    assembly.set_discrete(0, &Circuits::real_bytes(2.0));
    for (bias, zero) in [(0.0, true), (0.5, false), (0.0, true)] {
        let stamps = assembly.assemble_dc(&[0.0, 0.0, bias, 0.0], AssemblySolves::at_rest()).expect("assemble");
        assert_eq!(AssemblySolves::matrix_positions(&stamps), want, "at bias {bias}");
        let (a, _) = AssemblySolves::dense(&stamps, 4);
        assert_eq!(a[p][p] == 0.0, zero, "the partial at bias {bias} is {}", a[p][p]);
    }
}

#[test]
fn a_discrete_function_is_not_taken() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let ff = Circuits::instance("ff", m.flip_flop, vec![Discrete(0), Discrete(1), Discrete(2), Absent]);
    let cmp = Circuits::instance("cmp", m.comparator, vec![Circuits::at(&n[0]), Circuits::at(&n[1]), Discrete(2), Absent, Absent]);
    let diode = Circuits::instance("d1", m.diode, vec![Discrete(3), Discrete(4), Discrete(5), Circuits::at(&n[0]), Ground, Discrete(6)]);
    let plan = AnalogPlan::bind(vec![ff, cmp, diode], &net).expect("bind");
    assert_eq!(plan.instances(), 3);
    assert_eq!(plan.path(0), "ff");
    assert!(plan.analog_functions(0).is_empty(), "the flip-flop's edge function writes an event pin");
    assert_eq!(plan.analog_functions(1), vec![0], "the comparator's function writes watches");
    assert_eq!(plan.analog_functions(2), vec![0, 1, 2, 3, 4]);
    assert!(plan.pattern().iter().all(|(r, _)| r.idx() == n[0].idx()), "only the diode's anode row");
}

#[test]
fn a_term_to_ground_has_one_row() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &["dac.f"]);
    let (out, f) = (PlanCircuits::idx(&n[0]), PlanCircuits::idx(&n[1]));
    let dac = Circuits::instance("dac", m.digital_to_analog, vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let plan = AnalogPlan::bind(vec![dac], &net).expect("bind");
    // T0 FLOW {out, GROUND} ← f;  T1 FLOW {f, GROUND} ← out (and ← vref, a param).
    assert_eq!(PlanCircuits::positions(&plan), HashSet::from([(out, f), (f, out)]));

    // A resistor to ground: its rows are the anode's alone.
    let r = Circuits::instance("r1", m.resistor, vec![Discrete(0), Circuits::at(&n[0]), Ground]);
    let plan = AnalogPlan::bind(vec![r], &net).expect("bind");
    assert_eq!(PlanCircuits::positions(&plan), HashSet::from([(out, out)]));
}

#[test]
fn an_analog_bind_error_names_its_entry() {
    let m = FixtureModules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    // A term target bound to a discrete net.
    let r = Circuits::instance("top.r1", m.resistor, vec![Discrete(0), Discrete(1), Circuits::at(&n[0])]);
    let err = AnalogPlan::bind(vec![r], &net).expect_err("p is continuous").to_string();
    for part in ["top.r1", "term `i`", "abi.md:11", "plus is pin `p`", "Discrete(1)"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }

    // A limit written with a term and no OWNS_CORRECTION (abi.md A3).
    let bad = Circuits::instance("top.bad", m.bad_limit, vec![Circuits::at(&n[0]), Ground]);
    let err = AnalogPlan::bind(vec![bad], &net).expect_err("A3").to_string();
    for part in ["top.bad", "function both", "fixture:bad_limit:fn0", "OWNS_CORRECTION"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
}

#[test]
fn every_fixture_shape_is_valid() {
    let m = FixtureModules::all();
    for (name, module) in PlanCircuits::valid(&m) {
        let h = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        Validator::validate(h.shape()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let h = Handle::create(m.bad_limit, &[]).expect("create");
    assert_eq!(Validator::validate(h.shape()).expect_err("A3").rule, Rule::LimitCorrection);
}

#[test]
fn every_fixture_function_passes_the_partials_verifier() {
    let m = FixtureModules::all();
    let t = 300.15;
    // (fixture, function, the bytes of each read cell): points where no limit is active.
    let points: Vec<(&str, &'static Module, u32, Vec<Vec<u8>>)> = vec![
        ("resistor", m.resistor, 0, vec![PlanCircuits::real_bytes(1.2), PlanCircuits::real_bytes(0.3), PlanCircuits::real_bytes(1e3)]),
        ("diode limit", m.diode, 0, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(1e-14), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(t), PlanCircuits::real_bytes(0.0), vec![0], vec![1]]),
        ("diode current", m.diode, 1, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(1e-14), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(1e-12), PlanCircuits::real_bytes(t), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(0.0)]),
        ("diode shot", m.diode, 2, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(1e-14), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(t)]),
        ("diode flicker", m.diode, 3, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(1e-14), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(t), PlanCircuits::real_bytes(1e3)]),
        ("diode op", m.diode, 4, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(1e-14), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(t)]),
        ("capacitor", m.capacitor, 0, vec![PlanCircuits::real_bytes(2.0), PlanCircuits::real_bytes(-0.5), PlanCircuits::real_bytes(1e-9)]),
        ("comparator", m.comparator, 0, vec![PlanCircuits::real_bytes(0.4), PlanCircuits::real_bytes(0.1)]),
        ("dac", m.digital_to_analog, 0, vec![vec![128, 1], PlanCircuits::real_bytes(0.7), PlanCircuits::real_bytes(1e-3), PlanCircuits::real_bytes(2.5)]),
        ("cubic", m.cubic, 0, vec![PlanCircuits::real_bytes(0.7), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(2.0)]),
        ("source", m.source, 0, vec![PlanCircuits::real_bytes(1e-3)]),
        ("counted", m.counted, 0, vec![PlanCircuits::real_bytes(1.2), PlanCircuits::real_bytes(0.3), PlanCircuits::real_bytes(1e3)]),
        ("probe", m.probe, 0, vec![PlanCircuits::real_bytes(0.3), PlanCircuits::real_bytes(1e-6), PlanCircuits::real_bytes(0.0), vec![1], PlanCircuits::real_bytes(1e-12), PlanCircuits::real_bytes(1.0)]),
        ("faulty", m.faulty, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(0.0)]),
        ("plain_diode", m.plain_diode, 0, vec![PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(1e-14)]),
        ("vsource", m.voltage_source, 0, vec![PlanCircuits::real_bytes(3.0), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(2e-3), PlanCircuits::real_bytes(5.0), PlanCircuits::real_bytes(0.5)]),
        ("clamp limit", m.clamp, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(0.0)]),
        ("clamp i", m.clamp, 1, vec![PlanCircuits::real_bytes(0.4)]),
        ("owner", m.owner, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(0.0)]),
        ("reader", m.reader, 0, vec![PlanCircuits::real_bytes(0.4)]),
        ("tracker limit", m.tracker, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(0.5), vec![2], vec![0]]),
        ("tracker i", m.tracker, 1, vec![PlanCircuits::real_bytes(0.4)]),
        ("delay_line", m.delay_line, 0, vec![PlanCircuits::real_bytes(0.4), PlanCircuits::real_bytes(1e-9)]),
        ("trio", m.trio, 0, vec![PlanCircuits::real_bytes(0.4), PlanCircuits::real_bytes(0.5), PlanCircuits::real_bytes(0.6), PlanCircuits::real_bytes(2.0), PlanCircuits::real_bytes(3.0), PlanCircuits::real_bytes(4.0), PlanCircuits::real_bytes(1e-9)]),
        ("ac_vsource v", m.ac_voltage_source, 0, vec![PlanCircuits::real_bytes(3.0), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(2e-3), PlanCircuits::real_bytes(5.0), PlanCircuits::real_bytes(0.0)]),
        ("ac_vsource ac", m.ac_voltage_source, 1, vec![PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(0.0)]),
        ("noisy", m.noisy, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(1e3), PlanCircuits::real_bytes(0.0)]),
        ("split shared", m.split, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(1e3), PlanCircuits::real_bytes(1.0), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(0.0)]),
        ("split apart", m.split, 0, vec![PlanCircuits::real_bytes(0.9), PlanCircuits::real_bytes(0.2), PlanCircuits::real_bytes(1e3), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(0.0), PlanCircuits::real_bytes(0.0)]),
        ("flicker", m.flicker, 0, vec![PlanCircuits::real_bytes(0.4), PlanCircuits::real_bytes(0.0)]),
        ("tracer", m.tracer, 0, vec![PlanCircuits::real_bytes(0.7)]),
    ];
    for (name, module, k, input) in points {
        let h = Handle::create(module, &[]).expect("create");
        let verdict = Verdict::of_partials(&h, k, &input, 1e-6).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(verdict, Verdict::Exact, "{name}");
    }
}

/// The circuits the plan tests build.
pub struct PlanCircuits;

impl PlanCircuits {
    /// The pattern as a set of `(row, column)` unknown indices.
    fn positions(plan: &AnalogPlan) -> HashSet<(usize, usize)> {
        let p = plan.pattern();
        let set: HashSet<_> = p.iter().map(|(r, c)| (r.idx().expect("row"), c.idx().expect("column"))).collect();
        assert_eq!(set.len(), p.len(), "the pattern lists each position once");
        set
    }

    /// The idx helper `idx`.
    fn idx(r: &AnalogReference) -> usize {
        r.idx().expect("an unknown")
    }

    /// The fixtures that must pass every rule (all but `bad_limit`).
    fn valid(m: &FixtureModules) -> [(&'static str, &'static Module); 26] {
        [
            ("resistor", m.resistor),
            ("diode", m.diode),
            ("capacitor", m.capacitor),
            ("flip_flop", m.flip_flop),
            ("comparator", m.comparator),
            ("dac", m.digital_to_analog),
            ("cubic", m.cubic),
            ("source", m.source),
            ("counted", m.counted),
            ("probe", m.probe),
            ("faulty", m.faulty),
            ("plain_diode", m.plain_diode),
            ("vsource", m.voltage_source),
            ("clamp", m.clamp),
            ("owner", m.owner),
            ("reader", m.reader),
            ("tracker", m.tracker),
            ("delay_line", m.delay_line),
            ("trio", m.trio),
            ("ac_vsource", m.ac_voltage_source),
            ("noisy", m.noisy),
            ("split", m.split),
            ("flicker", m.flicker),
            ("capper", m.cap_watch_device),
            ("tracer", m.tracer),
            ("sample_hold", m.sample_hold),
        ]
    }

    /// The f helper `f`.
    fn real_bytes(v: f64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }
}
