//! `Interpreter::new` and the hand-built circuits (feature `ir-interp`, INT-01): an `Ir` that
//! fails `check()` is rejected with its entries; the crate spawns no process, reads no file and
//! holds no `unsafe`; the four M1 circuits are well formed.

use std::path::PathBuf;
use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{IrBuilder, NodeRef};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/circuits.rs"]
mod circuits;

use circuits::Circuits;
use devices::Devices;

#[test]
fn an_ir_that_fails_check_is_rejected() {
    // A resistor instance with no param bound and a node binding missing breaks `check()`.
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![]);
    let ir = b.finish(top);
    let expected = ir.check();
    assert!(!expected.is_empty(), "the fixture must fail check()");
    let rejected = Interpreter::new(Arc::new(ir)).expect_err("a broken IR is rejected");
    assert_eq!(rejected.entries, expected);
    assert!(rejected.entries.iter().all(|e| !e.what.is_empty() && !e.origin.file.is_empty()));
}

#[test]
fn a_checked_ir_is_accepted() {
    for ir in [Circuits::divider(), Circuits::diode(), Circuits::rc_lowpass(), Circuits::rc_held()] {
        assert_eq!(ir.check(), vec![]);
        assert!(Interpreter::new(Arc::new(ir)).is_ok());
    }
}

#[test]
fn the_held_circuit_holds_out_at_zero() {
    let ir = Circuits::rc_held();
    let out = ir.node_at("RcHeld.out").expect("node out");
    assert_eq!(out.init().map(|r| r.get()), Some(0.0));
    let r1 = ir.instance_at("RcHeld.r1").expect("r1");
    assert_eq!(r1.binding(1), Some(NodeRef::Node(out.id())));
}

#[test]
fn the_crate_spawns_nothing_and_reads_no_file() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src];
    let mut seen = 0;
    while let Some(path) = files.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("read src dir") {
                files.push(entry.expect("dir entry").path());
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            seen += 1;
            let text = std::fs::read_to_string(&path).expect("read a source");
            for needle in ["std::process", "std::fs", "unsafe"] {
                assert!(!text.contains(needle), "{} contains `{needle}`", path.display());
            }
        }
    }
    assert!(seen > 0, "no source scanned");
}

mod bound {
    use super::*;
        use piperine_ir::Ir;
    use piperine_solver::prelude::{
        AcSweepAnalysisOptions, CircuitInstance, Context, NodeIdentifier, Tolerances, TransientAnalysisOptions,
    };

    /// The solver node of the design node at `path`: node `i` of the IR is `Anonymous(i)`.
    pub fn node(ir: &Ir, path: &str) -> NodeIdentifier {
        NodeIdentifier::Anonymous(ir.node_at(path).unwrap_or_else(|| panic!("no node {path}")).id().index())
    }

    pub fn built(ir: Ir) -> (Ir, CircuitInstance) {
        let interp = Interpreter::new(Arc::new(ir.clone())).expect("a checked IR");
        let circuit = interp.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
        (ir, circuit)
    }

    pub fn dc(circuit: &mut CircuitInstance, id: &NodeIdentifier) -> f64 {
        let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
        result.get_node(id).expect("the node is solved")
    }

    pub fn tolerances() -> Tolerances {
        Tolerances::default()
    }

    /// Newton's own stopping tolerances tightened: at the defaults a converged diode sits a few
    /// 1e-8 (relative) from the fixed point, the solver's residual test, not the circuit.
    pub fn tight() -> Context {
        Context { tolerances: Tolerances { reltol: 1e-12, vntol: 1e-14, abstol: 1e-16, ..Tolerances::default() }, ..Context::default() }
    }

    pub fn ac_magnitude(circuit: &mut CircuitInstance, id: &NodeIdentifier, hz: f64) -> f64 {
        let opts = AcSweepAnalysisOptions { start_frequency: hz, stop_frequency: hz, steps: 1, logarithmic: false };
        let mut ac = circuit.ac(Context::default()).expect("ac");
        let sweep = ac.solve_sweep(opts).expect("sweep");
        sweep.get(0).expect("one point").get_node(id).expect("the node").norm()
    }

    pub fn follow(circuit: &mut CircuitInstance, id: &NodeIdentifier, stop: f64) -> Vec<(f64, f64)> {
        let opts = TransientAnalysisOptions::new(stop, stop / 400.0);
        let result = circuit.transient(opts, Context::default()).expect("transient").solve().expect("solve");
        result.iter().map(|s| (s.time(), s.get_node(id).expect("the node"))).collect()
    }
}

#[test]
fn the_divider_reads_seven_and_a_half_volts() {
    let (ir, mut circuit) = bound::built(Circuits::divider());
    let mid = bound::node(&ir, "Divider.mid");
    let v = bound::dc(&mut circuit, &mid);
    assert!((v - 7.5).abs() < 1e-9, "v(mid) = {v}");
}

#[test]
fn a_diode_converges_to_the_shockley_point() {
    let (ir, mut circuit) = bound::built(Circuits::diode());
    let a = bound::node(&ir, "DiodeCircuit.a");
    let vd = circuit.dc(bound::tight()).expect("dc").solve().expect("solve").get_node(&a).expect("v(a)");
    // the source is 0.8 V behind 1 ohm: (0.8 - vd) = is*(exp(vd/nvt) - 1) + gmin*vd
    let (is, t, gmin) = (1e-14, bound::tolerances().temperature, bound::tolerances().gmin);
    let nvt = 1.380649e-23 * t / 1.602176634e-19;
    let residue = |v: f64| (0.8 - v) - (is * ((v / nvt).exp() - 1.0) + gmin * v);
    assert!(residue(vd).abs() <= 1e-9 * (0.8 - vd).abs(), "v(a) = {vd}, residue {}", residue(vd));
    let id = (0.8 - vd) / 1.0;
    let shockley = is * ((vd / nvt).exp() - 1.0) + gmin * vd;
    assert!((id - shockley).abs() <= 1e-9 * id.abs(), "{id} vs {shockley}");
}

#[test]
fn an_rc_low_pass_is_at_minus_three_db_at_its_corner() {
    let (ir, mut circuit) = bound::built(Circuits::rc_lowpass());
    let out = bound::node(&ir, "RcLowpass.out");
    let m = bound::ac_magnitude(&mut circuit, &out, 100e3);
    assert!((m - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9, "|v(out)| = {m}");
}

#[test]
fn a_held_node_starts_the_transient() {
    let (ir, mut circuit) = bound::built(Circuits::rc_held());
    let out = bound::node(&ir, "RcHeld.out");
    let rc = 1000.0 * Circuits::FARADS;
    let steps = bound::follow(&mut circuit, &out, rc);
    let (t0, v0) = steps[0];
    assert_eq!(t0, 0.0);
    assert!(v0.abs() < 1e-9, "out at the first step: {v0}");
    // no two records share a time; the last one is the trajectory point at RC
    for pair in steps.windows(2) {
        assert!(pair[1].0 > pair[0].0, "records at {:e} then {:e}", pair[0].0, pair[1].0);
    }
    let (t, v) = *steps.last().expect("steps");
    assert!((t - rc).abs() < 1e-12 * rc, "the run ends at RC: {t:e}");
    assert!((v - (1.0 - (-1.0_f64).exp())).abs() < 1e-3, "out at RC: {v}");
}

#[test]
fn one_module_per_body_with_behaviour_or_a_held_node() {
    let interp = Interpreter::new(Arc::new(Circuits::rc_held())).expect("checked");
    let by_name = |n: &str| interp.ir().bodies().find(|b| b.name() == n).expect(n).id();
    for body in ["Vsource", "Resistor", "Capacitor", "RcHeld"] {
        assert!(interp.module(by_name(body)).is_some(), "{body} has a module");
    }
    // the divider's top has neither an `analog` block nor a held node
    let plain = Interpreter::new(Arc::new(Circuits::divider())).expect("checked");
    let top = plain.ir().top().body().id();
    assert!(plain.module(top).is_none());
    assert!(plain.ir().bodies().filter(|b| plain.module(b.id()).is_some()).count() == 2);
}

#[test]
fn a_circuit_binds_instances_nodes_and_params() {
    let (ir, mut circuit) = bound::built(Circuits::divider());
    let bound = circuit.bound().expect("devices are bound");
    let mut paths = bound.names.instances.clone();
    paths.sort();
    assert_eq!(paths, ["Divider.r1", "Divider.r2", "Divider.v1"]);
    // one solver node per design node, none for ground
    assert_eq!(circuit.nets().iter().filter(|n| n.label().starts_with("v(")).count(), ir.nodes().count());
    // the params hold their bound values: r2 = 3 k, so mid = 7.5 V; and `set` addresses them by path
    let mid = bound::node(&ir, "Divider.mid");
    assert!((bound::dc(&mut circuit, &mid) - 7.5).abs() < 1e-9);
    circuit.set("Divider.r2", "r", piperine_solver::prelude::ParamValue::Real(1000.0)).expect("set");
    assert!((bound::dc(&mut circuit, &mid) - 5.0).abs() < 1e-9);
}

#[test]
fn an_unset_param_keeps_its_default() {
    // The diode's `cj0` is bound at its default 0: the circuit passes no value for it, and its
    // pin holds the default; `is`, bound off its default, holds the bound value.
    let (_, mut circuit) = bound::built(Circuits::diode());
    // the pins take their defaults and the bound values when the first analysis starts
    circuit.dc(piperine_solver::prelude::Context::default()).expect("dc").solve().expect("solve");
    let bound = circuit.bound().expect("bound");
    let read = |param: &str| {
        let net = bound.names.params[&("DiodeCircuit.d1".to_string(), param.to_string())];
        let bytes = bound.digital.read(net);
        f64::from_le_bytes(bytes[..8].try_into().expect("an F64 pin"))
    };
    assert_eq!(read("cj0"), 0.0);
    assert_eq!(read("n"), 1.0);
    assert_eq!(read("is"), 1e-14);
}

#[test]
fn a_set_that_breaks_a_where_is_refused() {
    use piperine_solver::prelude::ParamValue;
    let (ir, mut circuit) = bound::built(Circuits::divider());
    let mid = bound::node(&ir, "Divider.mid");
    assert!((bound::dc(&mut circuit, &mid) - 7.5).abs() < 1e-9);
    // `r` has `where r > 0`: a negative value is refused and the next dc reads what it read
    let refused = circuit.set("Divider.r1", "r", ParamValue::Real(-1.0));
    assert!(refused.is_err(), "r = -1 breaks `where r > 0`");
    assert!((bound::dc(&mut circuit, &mid) - 7.5).abs() < 1e-9);
    let zero = circuit.set("Divider.r1", "r", ParamValue::Real(0.0));
    assert!(zero.is_err(), "r = 0 breaks `where r > 0`");
    // a legal value is accepted and changes the result: r1 = 3 k, r2 = 3 k, so mid = 5 V
    circuit.set("Divider.r1", "r", ParamValue::Real(3000.0)).expect("a legal value");
    assert!((bound::dc(&mut circuit, &mid) - 5.0).abs() < 1e-9);
}
