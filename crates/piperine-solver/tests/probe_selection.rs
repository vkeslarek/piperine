//! Save/probe selection (ABI-34): `TransientAnalysisOptions::probe_selection`
//! filters `collect_device_banks` to record only the requested observables
//! per device — not every device's full `(state, vars)` bank every step.
//! Memory cost is `O(requested × steps)`, not `O(devices × steps)`.
//!
//! Also covers ABI-33 wiring (the field + builder) and the backward-compat
//! contract:
//! - empty selection + `record_device_state = false` = nothing recorded.
//! - `record_device_state = true` = "record everything" shorthand.
//! - non-empty selection = record only requested observables.

use piperine_solver::abi::*;
use piperine_solver::prelude::{Context, ProbeSelection, Solver, TransientAnalysisOptions};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A device that owns a small runtime bank and declares it as an
/// observable. The bank is `[step_count]` — a single state slot — bumped
/// on each `load_transient` so the recording visibly advances per step.
struct StatefulStub {
    label: &'static str,
    state: Vec<f64>,
    vars: Vec<f64>,
    loads: Arc<AtomicUsize>,
}

impl StatefulStub {
    fn new(label: &'static str, loads: Arc<AtomicUsize>) -> Self {
        Self {
            label,
            state: vec![0.0],
            vars: vec![0.0],
            loads,
        }
    }
}

impl AnalogDevice for StatefulStub {
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.state[0] += 1.0;
        self.vars[0] += 10.0;
        Vec::new()
    }

    fn load_dc(
        &mut self,
        _s: &DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        Vec::new()
    }
}

impl DigitalDevice for StatefulStub {}

impl Introspect for StatefulStub {
    fn list_observables(&self) -> Vec<ObservableDescriptor> {
        vec![
            ObservableDescriptor {
                name: "state[0]".to_string(),
                kind: ObservableKind::State,
                cost: 0.2,
            },
            ObservableDescriptor {
                name: "var[0]".to_string(),
                kind: ObservableKind::Var,
                cost: 0.1,
            },
        ]
    }
}

impl Element for StatefulStub {
    fn name(&self) -> &str {
        self.label
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }

    fn runtime_banks(&self) -> (&[f64], &[f64]) {
        (&self.state, &self.vars)
    }
}

/// A trivial transient driver: source-free circuit, only the stub's load
/// runs each step. The stub is the only device — the recorded device_state
/// map reflects exactly what `collect_device_banks` chose to put there.
fn run_with(opts: TransientAnalysisOptions) -> piperine_solver::prelude::TransientAnalysisResult {
    let loads = Arc::new(AtomicUsize::new(0));
    let dev = StatefulStub::new("stub", loads);
    let netlist = Netlist::new();
    let circuit = CircuitInstance::from_devices_and_netlist(
        "probe-sel",
        vec![Box::new(dev)],
        netlist,
    );
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    solver.tran().expect("transient solves").solve().expect("transient runs")
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
    let res = run_with(opts);
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
    let res = run_with(opts);
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
    let res = run_with(opts);
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
    let res = run_with(opts);
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
    let res = run_with(opts);
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

/// ABI-34 acceptance: a 100-step transient with 10 devices and a
/// `ProbeSelection` requesting exactly one observable on one device records
/// only that device's requested slice. The other 9 devices are absent from
/// every step's `device_state` map. Memory is O(1 device), not O(10).
fn run_with_devices(
    opts: TransientAnalysisOptions,
    devices: Vec<Box<dyn Element>>,
) -> piperine_solver::prelude::TransientAnalysisResult {
    let netlist = Netlist::new();
    let circuit = CircuitInstance::from_devices_and_netlist("probe-sel", devices, netlist);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    solver.tran().expect("transient solves").solve().expect("transient runs")
}

#[test]
fn ten_devices_one_observable_records_only_that_device() {
    const N: usize = 10;
    let loads = Arc::new(AtomicUsize::new(0));
    let mut devices: Vec<Box<dyn Element>> = Vec::with_capacity(N);
    for i in 0..N {
        let label = Box::leak(format!("dev{i}").into_boxed_str());
        devices.push(Box::new(StatefulStub::new(label, loads.clone())));
    }
    let opts = TransientAnalysisOptions::new(1e-5, 1e-7)
        .with_dt_max(1e-7)
        .with_probe_selection(ProbeSelection::new().request("dev3", "state[0]"));
    let res = run_with_devices(opts, devices);
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
