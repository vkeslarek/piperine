//! The analog half of bind (feature `solver-abi-analog`, SAN-01, SAN-02,
//! T1): `AnalogPlan::bind` takes analog functions only (F2), resolves term
//! targets to rows (a `GROUND` side has none), names the entry of a broken
//! rule, and builds the matrix pattern as terms × partials, through a
//! `LIMIT`'s `PROPOSED` partials (C13). It also holds the fixture devices to
//! the contract: every shape validates and every declared partial agrees with
//! the finite-difference verifier of abi.md §8.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use std::collections::HashSet;

use circuit::{at, f64_bytes, instance, netlist};
use devices::{Modules, modules};
use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_abi::validate::{Rule, validate};
use piperine_abi::verify::{Verdict, verify_partials};
use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use piperine_analog_solver::netlist::AnalogReference;

/// The pattern as a set of `(row, column)` unknown indices.
fn positions(plan: &AnalogPlan) -> HashSet<(usize, usize)> {
    let p = plan.pattern();
    let set: HashSet<_> = p.iter().map(|(r, c)| (r.idx().expect("row"), c.idx().expect("column"))).collect();
    assert_eq!(set.len(), p.len(), "the pattern lists each position once");
    set
}

fn idx(r: &AnalogReference) -> usize {
    r.idx().expect("an unknown")
}

#[test]
fn the_pattern_is_terms_times_partials() {
    let m = modules();
    let (net, n) = netlist(4, &[]);
    // The diode reaches its pins only through the limit's PROPOSED partials.
    let diode = instance(
        "d1",
        m.diode,
        vec![Discrete(0), Discrete(1), Discrete(2), at(&n[0]), at(&n[1]), Discrete(3)],
    );
    let plan = AnalogPlan::bind(vec![diode], &net).expect("bind");
    let (a, k) = (idx(&n[0]), idx(&n[1]));
    let want = HashSet::from([(a, a), (a, k), (k, a), (k, k)]);
    assert_eq!(positions(&plan), want);
    let mut asm = Assembly::new(plan);
    for (slot, value) in [(0, 1e-14), (1, 1.0), (2, 1e-12), (3, 300.15)] {
        asm.set_discrete(slot, &f64_bytes(value));
    }
    for bias in [0.0, 0.3, 2.0] {
        let stamps = asm.assemble_dc(&[bias, 0.0, 0.0, 0.0], system::at_rest()).expect("assemble");
        assert_eq!(system::matrix_positions(&stamps), want, "the diode at bias {bias}");
        asm.end_iteration();
    }

    // The cubic's partials vanish at zero bias; its positions are declared,
    // and an equilibrium solve stamps every one of them at any bias.
    let cubic = instance("c1", m.cubic, vec![Discrete(0), at(&n[2]), at(&n[3])]);
    let plan = AnalogPlan::bind(vec![cubic], &net).expect("bind");
    let (p, q) = (idx(&n[2]), idx(&n[3]));
    let want = HashSet::from([(p, p), (p, q), (q, p), (q, q)]);
    assert_eq!(positions(&plan), want);
    let mut asm = Assembly::new(plan);
    asm.set_discrete(0, &f64_bytes(2.0));
    for (bias, zero) in [(0.0, true), (0.5, false), (0.0, true)] {
        let stamps = asm.assemble_dc(&[0.0, 0.0, bias, 0.0], system::at_rest()).expect("assemble");
        assert_eq!(system::matrix_positions(&stamps), want, "at bias {bias}");
        let (a, _) = system::dense(&stamps, 4);
        assert_eq!(a[p][p] == 0.0, zero, "the partial at bias {bias} is {}", a[p][p]);
    }
}

#[test]
fn a_discrete_function_is_not_taken() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let ff = instance("ff", m.flip_flop, vec![Discrete(0), Discrete(1), Discrete(2), Absent]);
    let cmp = instance("cmp", m.comparator, vec![at(&n[0]), at(&n[1]), Discrete(2), Absent, Absent]);
    let diode = instance("d1", m.diode, vec![Discrete(3), Discrete(4), Discrete(5), at(&n[0]), Ground, Discrete(6)]);
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
    let m = modules();
    let (net, n) = netlist(1, &["dac.f"]);
    let (out, f) = (idx(&n[0]), idx(&n[1]));
    let dac = instance("dac", m.dac, vec![Discrete(0), Discrete(1), at(&n[0]), at(&n[1])]);
    let plan = AnalogPlan::bind(vec![dac], &net).expect("bind");
    // T0 FLOW {out, GROUND} ← f;  T1 FLOW {f, GROUND} ← out (and ← vref, a param).
    assert_eq!(positions(&plan), HashSet::from([(out, f), (f, out)]));

    // A resistor to ground: its rows are the anode's alone.
    let r = instance("r1", m.resistor, vec![Discrete(0), at(&n[0]), Ground]);
    let plan = AnalogPlan::bind(vec![r], &net).expect("bind");
    assert_eq!(positions(&plan), HashSet::from([(out, out)]));
}

#[test]
fn an_analog_bind_error_names_its_entry() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    // A term target bound to a discrete net.
    let r = instance("top.r1", m.resistor, vec![Discrete(0), Discrete(1), at(&n[0])]);
    let err = AnalogPlan::bind(vec![r], &net).expect_err("p is continuous").to_string();
    for part in ["top.r1", "term `i`", "abi.md:11", "plus is pin `p`", "Discrete(1)"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }

    // A limit written with a term and no OWNS_CORRECTION (abi.md A3).
    let bad = instance("top.bad", m.bad_limit, vec![at(&n[0]), Ground]);
    let err = AnalogPlan::bind(vec![bad], &net).expect_err("A3").to_string();
    for part in ["top.bad", "function both", "fixture:bad_limit:fn0", "OWNS_CORRECTION"] {
        assert!(err.contains(part), "`{err}` does not name {part}");
    }
}

/// The fixtures that must pass every rule (all but `bad_limit`).
fn valid(m: &Modules) -> [(&'static str, &'static Module); 26] {
    [
        ("resistor", m.resistor),
        ("diode", m.diode),
        ("capacitor", m.capacitor),
        ("flip_flop", m.flip_flop),
        ("comparator", m.comparator),
        ("dac", m.dac),
        ("cubic", m.cubic),
        ("source", m.source),
        ("counted", m.counted),
        ("probe", m.probe),
        ("faulty", m.faulty),
        ("plain_diode", m.plain_diode),
        ("vsource", m.vsource),
        ("clamp", m.clamp),
        ("owner", m.owner),
        ("reader", m.reader),
        ("tracker", m.tracker),
        ("delay_line", m.delay_line),
        ("trio", m.trio),
        ("ac_vsource", m.ac_vsource),
        ("noisy", m.noisy),
        ("split", m.split),
        ("flicker", m.flicker),
        ("capper", m.capper),
        ("tracer", m.tracer),
        ("sample_hold", m.sample_hold),
    ]
}

#[test]
fn every_fixture_shape_is_valid() {
    let m = modules();
    for (name, module) in valid(&m) {
        let h = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        validate(h.shape()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let h = Handle::create(m.bad_limit, &[]).expect("create");
    assert_eq!(validate(h.shape()).expect_err("A3").rule, Rule::LimitCorrection);
}

fn f(v: f64) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

#[test]
fn every_fixture_function_passes_the_partials_verifier() {
    let m = modules();
    let t = 300.15;
    // (fixture, function, the bytes of each read cell): points where no limit is active.
    let points: Vec<(&str, &'static Module, u32, Vec<Vec<u8>>)> = vec![
        ("resistor", m.resistor, 0, vec![f(1.2), f(0.3), f(1e3)]),
        ("diode limit", m.diode, 0, vec![f(0.6), f(0.0), f(1e-14), f(1.0), f(t), f(0.0), vec![0], vec![1]]),
        ("diode current", m.diode, 1, vec![f(0.6), f(1e-14), f(1.0), f(1e-12), f(t), f(0.0), f(0.0)]),
        ("diode shot", m.diode, 2, vec![f(0.6), f(1e-14), f(1.0), f(t)]),
        ("diode flicker", m.diode, 3, vec![f(0.6), f(1e-14), f(1.0), f(t), f(1e3)]),
        ("diode op", m.diode, 4, vec![f(0.6), f(1e-14), f(1.0), f(t)]),
        ("capacitor", m.capacitor, 0, vec![f(2.0), f(-0.5), f(1e-9)]),
        ("comparator", m.comparator, 0, vec![f(0.4), f(0.1)]),
        ("dac", m.dac, 0, vec![vec![128, 1], f(0.7), f(1e-3), f(2.5)]),
        ("cubic", m.cubic, 0, vec![f(0.7), f(0.2), f(2.0)]),
        ("source", m.source, 0, vec![f(1e-3)]),
        ("counted", m.counted, 0, vec![f(1.2), f(0.3), f(1e3)]),
        ("probe", m.probe, 0, vec![f(0.3), f(1e-6), f(0.0), vec![1], f(1e-12), f(1.0)]),
        ("faulty", m.faulty, 0, vec![f(0.9), f(0.2), f(0.0)]),
        ("plain_diode", m.plain_diode, 0, vec![f(0.6), f(0.0), f(1e-14)]),
        ("vsource", m.vsource, 0, vec![f(3.0), f(1.0), f(2e-3), f(5.0), f(0.5)]),
        ("clamp limit", m.clamp, 0, vec![f(0.9), f(0.2), f(0.0)]),
        ("clamp i", m.clamp, 1, vec![f(0.4)]),
        ("owner", m.owner, 0, vec![f(0.9), f(0.2), f(0.0)]),
        ("reader", m.reader, 0, vec![f(0.4)]),
        ("tracker limit", m.tracker, 0, vec![f(0.9), f(0.2), f(0.5), vec![2], vec![0]]),
        ("tracker i", m.tracker, 1, vec![f(0.4)]),
        ("delay_line", m.delay_line, 0, vec![f(0.4), f(1e-9)]),
        ("trio", m.trio, 0, vec![f(0.4), f(0.5), f(0.6), f(2.0), f(3.0), f(4.0), f(1e-9)]),
        ("ac_vsource v", m.ac_vsource, 0, vec![f(3.0), f(1.0), f(2e-3), f(5.0), f(0.0)]),
        ("ac_vsource ac", m.ac_vsource, 1, vec![f(1.0), f(0.0)]),
        ("noisy", m.noisy, 0, vec![f(0.9), f(0.2), f(1e3), f(0.0)]),
        ("split shared", m.split, 0, vec![f(0.9), f(0.2), f(1e3), f(1.0), f(0.0), f(0.0)]),
        ("split apart", m.split, 0, vec![f(0.9), f(0.2), f(1e3), f(0.0), f(0.0), f(0.0)]),
        ("flicker", m.flicker, 0, vec![f(0.4), f(0.0)]),
        ("tracer", m.tracer, 0, vec![f(0.7)]),
    ];
    for (name, module, k, input) in points {
        let h = Handle::create(module, &[]).expect("create");
        let verdict = verify_partials(&h, k, &input, 1e-6).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(verdict, Verdict::Exact, "{name}");
    }
}
