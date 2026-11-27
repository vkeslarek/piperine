//! Save/probe selection (ABI-34) on the `Device` trait:
//! `TransientAnalysisOptions::probe_selection` filters the recorded
//! `device_state` to the requested observables per device instance: a term's
//! value (the `state` slice) and a `VAR` watch (the `vars` slice), by instance
//! path and name. Memory cost is `O(requested × steps)`, not `O(devices × steps)`.
//!
//! Also covers ABI-33 wiring (the field + builder) and the backward-compat
//! contract:
//! - empty selection + `record_device_state = false` = nothing recorded.
//! - `record_device_state = true` = "record everything" shorthand.
//! - non-empty selection = record only requested observables.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::partials::PartialsCheck;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, role, watch};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::Port;
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, ProbeSelection, Solver, TransientAnalysisOptions,
};

const ORIGIN: &str = "probe_selection.rs";

/// A conductor to ground that declares two observables: its term `state[0]`
/// (the current `1 mS · V(p)`) and its `VAR` watch `var[0]` (`10 · V(p)`).
/// Driven by a ramp both advance with the time.
/// pins: p=0 (PORT); terms: T0 FLOW{p, GROUND} "state[0]"; watches: W0 VAR "var[0]"
/// fn 0 "i": reads [p] writes [T0] partials (0<-0); fn 1 "show": reads [p] writes [W0]
struct Stub;
static STUB_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static STUB: OnceLock<Module> = OnceLock::new();

impl Device for Stub {
    fn prototype() -> &'static Shape<'static> {
        STUB_SHAPE.get_or_init(ProbeSelectionCases::stub_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Stub, Arc::clone(STUB_SHAPE.get_or_init(ProbeSelectionCases::stub_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let vp = cx.f64(0);
        if k == 1 {
            cx.set(0, 10.0 * vp);
        } else {
            cx.set(0, 1.0e-3 * vp);
            cx.jac().copy_from_slice(&[1.0e-3]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// ABI-33 wiring: `with_probe_selection` stashes a non-empty selection,
/// and the field is readable back from options.
#[test]
fn with_probe_selection_builder_stores_requests() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7).with_probe_selection(
        ProbeSelection::new()
            .request("stub", "state[0]")
            .request("stub", "var[0]"),
    );
    assert_eq!(opts.probe_selection.requests.len(), 2);
    assert!(opts.probe_selection.contains("stub", "state[0]"));
    assert!(opts.probe_selection.contains("stub", "var[0]"));
}

/// ABI-34 backward-compat: empty selection + `record_device_state = false`
/// records nothing — `device_state` is empty in every step. This is the
/// pre-feature default-off behavior preserved exactly.
#[test]
fn empty_selection_and_no_global_flag_records_nothing() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let res = ProbeSelectionCases::run_with(opts);
    assert!(res.len() > 1, "transient produced steps");
    for step in res.iter() {
        assert!(
            step.device_state("stub").is_none(),
            "no device_state should be recorded when off"
        );
    }
}

/// ABI-34 shorthand: `record_device_state = true` records every device's
/// full `(state, vars)` bank — the existing "all observables" mode.
#[test]
fn record_device_state_true_records_full_banks() {
    let opts = TransientAnalysisOptions {
        record_device_state: true,
        ..TransientAnalysisOptions::new(1e-6, 1e-7)
    };
    let res = ProbeSelectionCases::run_with(opts);
    let last = res.last().expect("at least one step");
    let banks = last
        .device_state("stub")
        .expect("full banks recorded under record_device_state");
    assert_eq!(banks.0.len(), 1, "state bank has 1 slot");
    assert_eq!(banks.1.len(), 1, "vars bank has 1 slot");
    assert!(banks.0[0] > 0.0, "state advances per step");
    assert!(banks.1[0] > 0.0, "vars advance per step");
}

/// ABI-34 core: a `ProbeSelection` requesting only `state[0]` on the stub
/// records the state slice but not the vars slice. The recorded state
/// advances per step.
#[test]
fn probe_selection_records_only_requested_state_observable() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7).with_probe_selection(
        ProbeSelection::new().request("stub", "state[0]"),
    );
    let res = ProbeSelectionCases::run_with(opts);
    let last = res.last().expect("at least one step");
    let banks = last
        .device_state("stub")
        .expect("stub is in the selection");
    assert_eq!(banks.0.len(), 1, "state[0] recorded");
    assert!(
        banks.1.is_empty(),
        "var[0] not requested — vars slice empty"
    );
    assert!(banks.0[0] > 0.0, "state advances per step");
}

/// ABI-34 core: a `ProbeSelection` requesting only `var[0]` on the stub
/// records the vars slice but not the state slice.
#[test]
fn probe_selection_records_only_requested_var_observable() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7).with_probe_selection(
        ProbeSelection::new().request("stub", "var[0]"),
    );
    let res = ProbeSelectionCases::run_with(opts);
    let last = res.last().expect("at least one step");
    let banks = last
        .device_state("stub")
        .expect("stub is in the selection");
    assert!(
        banks.0.is_empty(),
        "state[0] not requested — state slice empty"
    );
    assert_eq!(banks.1.len(), 1, "var[0] recorded");
    assert!(banks.1[0] > 0.0, "vars advance per step");
}

/// ABI-34 multi-step: across N accepted steps, only the requested
/// observable is recorded, and its value advances per step (selective
/// recording is per-step, not just at the boundary).
#[test]
fn selective_recording_advances_per_accepted_step() {
    let opts = TransientAnalysisOptions::new(5e-6, 1e-6).with_probe_selection(
        ProbeSelection::new().request("stub", "state[0]"),
    );
    let res = ProbeSelectionCases::run_with(opts);
    assert!(res.len() >= 3, "at least three steps");
    let first = res
        .get(1)
        .expect("step 1 exists")
        .device_state("stub")
        .expect("recorded")
        .0[0];
    let last = res
        .last()
        .expect("last step")
        .device_state("stub")
        .expect("recorded")
        .0[0];
    assert!(
        last > first,
        "selective recording advances across steps (first={first}, last={last})"
    );
}

#[test]
fn ten_devices_one_observable_records_only_that_device() {
    const N: usize = 10;
    let labels: Vec<String> = (0..N).map(|i| format!("dev{i}")).collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let opts = TransientAnalysisOptions::new(1e-5, 1e-7)
        .with_dt_max(1e-7)
        .with_probe_selection(ProbeSelection::new().request("dev3", "state[0]"));
    let res = ProbeSelectionCases::run_with_devices(opts, &labels);
    assert!(res.len() >= 100, "at least 100 steps recorded");

    for step in res.iter() {
        let recorded: Vec<&str> = step
            .device_state_keys()
            .collect();
        assert_eq!(
            recorded, ["dev3"],
            "only dev3 should be recorded, got {recorded:?}"
        );
        let banks = step
            .device_state("dev3")
            .expect("dev3 recorded");
        assert_eq!(banks.0.len(), 1, "state[0] requested");
        assert!(banks.1.is_empty(), "var[0] not requested");
    }
}

/// ABI-35: requesting an observable a device doesn't declare fails loud
/// at setup. The stub declares `state[0]` and `var[0]`; requesting
/// `nonexistent` on it surfaces a named error.
#[test]
fn unknown_observable_fails_loud_at_setup() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7)
        .with_probe_selection(ProbeSelection::new().request("stub", "nonexistent"));
    let err = ProbeSelectionCases::tran_setup_result(opts).expect_err("unknown observable should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("device `stub` has no observable `nonexistent`"),
        "expected named-observable error, got: {msg}"
    );
}

/// ABI-35: requesting a device label that doesn't exist in the circuit
/// fails loud at setup with a "device not found" error.
#[test]
fn unknown_device_fails_loud_at_setup() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7)
        .with_probe_selection(ProbeSelection::new().request("ghost", "state[0]"));
    let err = ProbeSelectionCases::tran_setup_result(opts).expect_err("unknown device should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("device `ghost` not found"),
        "expected named-device error, got: {msg}"
    );
}

/// ABI-35 happy path: a `ProbeSelection` that names only valid
/// (device, observable) pairs must pass setup without error. Guards
/// against the validation being over-eager.
#[test]
fn valid_probe_selection_passes_setup() {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7)
        .with_probe_selection(
            ProbeSelection::new()
                .request("stub", "state[0]")
                .request("stub", "var[0]"),
        );
    ProbeSelectionCases::tran_setup_result(opts).expect("valid selection must not fail setup");
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("Stub", STUB.get_or_init(|| module::<Stub>("Stub")));
}

/// The helpers of the probe_selection tests.
pub struct ProbeSelectionCases;

impl ProbeSelectionCases {
    /// The cell of helper `cell_of`.
    fn cell_of(kind: u32, index: u32) -> Cell {
        Cell { kind, index, part: part::VALUE }
    }

    /// The stub shape helper `stub_shape`.
    fn stub_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let t0 = s.term(TermSpec {
            name: "state[0]",
            origin: ORIGIN,
            op: op::FLOW,
            plus: p,
            minus: piperine_abi::raw::GROUND,
            abstol: DEFAULT,
            max: DEFAULT,
        });
        let w0 = s.watch(WatchSpec {
            name: "var[0]",
            origin: ORIGIN,
            kind: watch::VAR,
            layout: &F64,
            event: DEFAULT,
            ttol: DEFAULT,
            etol: DEFAULT,
        });
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ProbeSelectionCases::cell_of(cell::PIN, p)],
            writes: vec![ProbeSelectionCases::cell_of(cell::TERM, t0)],
            partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "show",
            origin: ORIGIN,
            reads: vec![ProbeSelectionCases::cell_of(cell::PIN, p)],
            writes: vec![ProbeSelectionCases::cell_of(cell::WATCH, w0)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// A ramp source over `labels.len()` stubs on one node.
    fn circuit_of(labels: &[&str]) -> CircuitInstance {
        let mut b = CircuitBuilder::new("probe-sel");
        let gnd = b.ground();
        let src = b.node("src");
        b.device(circuit::RampSource::module(), "ramp", &[], &[Port::Node(src.clone()), Port::Node(gnd)]);
        for label in labels {
            b.device(STUB.get_or_init(|| module::<Stub>("Stub")), label, &[], &[Port::Node(src.clone())]);
        }
        b.build().expect("build")
    }

    /// A transient of one stub: the recorded `device_state` map reflects exactly
    /// what the selection chose to put there.
    fn run_with(opts: TransientAnalysisOptions) -> piperine_solver::prelude::TransientAnalysisResult {
        let mut solver = Solver::new(ProbeSelectionCases::circuit_of(&["stub"])).with_tran_opts(opts).build();
        solver.tran().expect("transient solves").solve().expect("transient runs")
    }

    /// ABI-34 acceptance: a 100-step transient with 10 devices and a
    /// `ProbeSelection` requesting exactly one observable on one device records
    /// only that device's requested slice. The other 9 devices are absent from
    /// every step's `device_state` map. Memory is O(1 device), not O(10).
    fn run_with_devices(
        opts: TransientAnalysisOptions,
        labels: &[&str],
    ) -> piperine_solver::prelude::TransientAnalysisResult {
        let mut solver = Solver::new(ProbeSelectionCases::circuit_of(labels)).with_tran_opts(opts).build();
        solver.tran().expect("transient solves").solve().expect("transient runs")
    }

    // ── ABI-35: fail-loud on unknown observable/device at setup ─────────────
    //
    // A `ProbeSelection` that requests a device that doesn't exist, or an
    // observable a known device doesn't declare, fails loud at solver setup
    // (before any step) — not silently records nothing. The error names the
    // offending device/observable so a host can pinpoint the typo.

    /// Build a transient solver against the stub circuit; returns the setup
    /// Result so the fail-loud validation paths can be asserted without
    /// driving a run.
    fn tran_setup_result(
        opts: TransientAnalysisOptions,
    ) -> piperine_solver::Result<()> {
        let mut solver = Solver::new(ProbeSelectionCases::circuit_of(&["stub"])).with_tran_opts(opts).build();
        solver.tran().map(|_| ())
    }
}
