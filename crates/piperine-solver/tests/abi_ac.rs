//! The AC driver on devices (SAI-11): the parallel RLC of
//! `parity_baseline.rs` built from `Device`s reads the same pinned peak, and
//! nothing discrete runs or changes during a sweep.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use abi_devices::{add_capacitor, add_resistor, current_ac, inductor, node_id};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Module, Shape, Statement, global, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::layout::BYTE;
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{AcSweepAnalysisOptions, CircuitBuilder, Context};

static REACTIONS: AtomicUsize = AtomicUsize::new(0);

/// A device with a reaction on `START` that counts its runs: pins start=0 (GLOBAL START),
/// log=1 (PRIVATE BYTE); reaction 0 on [start] writes [S0 {log}].
struct Spy;
static SPY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SPY_MODULE: OnceLock<Module> = OnceLock::new();

fn spy_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
    let log = s.pin(PinSpec::discrete("log", role::PRIVATE, &BYTE));
    s.reaction(ReactionSpec {
        name: "run",
        origin: "abi_ac.rs",
        on: vec![start],
        writes: vec![Statement { pin: log, offset: 0, len: 2, flags: 0, min_after: 0.0 }],
        ..ReactionSpec::default()
    });
    s.freeze()
}

impl Device for Spy {
    fn prototype() -> &'static Shape<'static> {
        SPY_SHAPE.get_or_init(spy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Spy, Arc::clone(SPY_SHAPE.get_or_init(spy_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        REACTIONS.fetch_add(1, Ordering::SeqCst);
        cx.commit(0, &[1, 1], 0.0)?;
        Ok(Flow::Continue)
    }
}

#[test]
fn an_rlc_peak_matches_the_baseline_on_devices() {
    let mut b = CircuitBuilder::new("rlc");
    let gnd = b.ground();
    let node = b.node("node");
    b.device(current_ac(), "iac", &[("i", ParamValue::Real(1.0))], &[Port::Node(node.clone())]);
    add_resistor(&mut b, "r1", 10_000.0, &node, &gnd);
    add_capacitor(&mut b, "c1", 1e-9, &node, &gnd);
    b.device(inductor(), "l1", &[("l", ParamValue::Real(1e-3))], &[Port::Node(node.clone()), Port::Node(gnd)]);
    let spy = SPY_MODULE.get_or_init(|| module::<Spy>("Spy"));
    b.device(spy, "spy", &[], &[]);
    let mut circuit = b.build().expect("build");

    let opts = AcSweepAnalysisOptions { start_frequency: 1e4, stop_frequency: 1e6, steps: 200, logarithmic: true };
    // The bias point is an operating point: `START` fires there. The sweep is
    // measured from after it.
    let mut ac = circuit.ac(Context::default()).expect("ac");
    let reactions_before = REACTIONS.load(Ordering::SeqCst);
    let nets_before: Vec<Vec<u8>> = {
        let bound = ac.system.circuit.bound().expect("bound");
        (0..bound.digital.plan().nets.len())
            .map(|i| bound.digital.read(piperine_digital_solver::net::NetId(i as u32)).to_vec())
            .collect()
    };
    let res = ac.solve_sweep(opts).expect("sweep");

    let mut peak_mag = 0.0_f64;
    let mut peak_freq = 0.0_f64;
    for i in 0..res.len() {
        let step = res.get(i).unwrap();
        let v = step.get_node(&node_id(&node)).unwrap();
        if v.norm() > peak_mag {
            peak_mag = v.norm();
            peak_freq = step.frequency;
        }
    }
    assert!((peak_mag - 9_817.187_756_927_691).abs() < 1e-6, "peak_mag = {peak_mag}");
    assert!((peak_freq - 160_705.281_826_163_85).abs() < 1e-3, "peak_freq = {peak_freq}");

    // No reaction ran and no pin changed during the sweep.
    assert_eq!(REACTIONS.load(Ordering::SeqCst), reactions_before, "a reaction ran");
    let bound = circuit.bound().expect("bound");
    for (i, before) in nets_before.iter().enumerate() {
        let now = bound.digital.read(piperine_digital_solver::net::NetId(i as u32));
        assert_eq!(now, before.as_slice(), "net {i} changed");
    }
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    partials::verify_every_function("Spy", SPY_MODULE.get_or_init(|| module::<Spy>("Spy")));
}
