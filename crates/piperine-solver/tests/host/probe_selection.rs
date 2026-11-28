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
use piperine_device_fixtures::solver_devices::probe_selection::{StateAndVariableProbe};

use piperine_solver::abi::Port;
use piperine_solver::prelude::{
    CircuitBuilder, CircuitInstance, ProbeSelection, Solver, TransientAnalysisOptions,
};

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
    let res = ProbeRun::run_with(opts);
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
    let res = ProbeRun::run_with(opts);
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
    let res = ProbeRun::run_with(opts);
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
    let res = ProbeRun::run_with(opts);
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
    let res = ProbeRun::run_with(opts);
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
    let res = ProbeRun::run_with_devices(opts, &labels);
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
    let err = ProbeRun::tran_setup_result(opts).expect_err("unknown observable should fail");
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
    let err = ProbeRun::tran_setup_result(opts).expect_err("unknown device should fail");
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
    ProbeRun::tran_setup_result(opts).expect("valid selection must not fail setup");
}

/// A circuit run with a probe selection, with or without devices.
struct ProbeRun;

impl ProbeRun {

    /// A ramp source over `labels.len()` stubs on one node.
    fn circuit_of(labels: &[&str]) -> CircuitInstance {
        let mut b = CircuitBuilder::new("probe-sel");
        let gnd = b.ground();
        let src = b.node("src");
        b.device(circuit::RampSource::module(), "ramp", &[], &[Port::Node(src.clone()), Port::Node(gnd)]);
        for label in labels {
            b.device(StateAndVariableProbe::module(), label, &[], &[Port::Node(src.clone())]);
        }
        b.build().expect("build")
    }

    /// A transient of one stub: the recorded `device_state` map reflects exactly
    /// what the selection chose to put there.
    fn run_with(opts: TransientAnalysisOptions) -> piperine_solver::prelude::TransientAnalysisResult {
        let mut solver = Solver::new(ProbeRun::circuit_of(&["stub"])).with_tran_opts(opts).build();
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
        let mut solver = Solver::new(ProbeRun::circuit_of(labels)).with_tran_opts(opts).build();
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
        let mut solver = Solver::new(ProbeRun::circuit_of(&["stub"])).with_tran_opts(opts).build();
        solver.tran().map(|_| ())
    }
}
