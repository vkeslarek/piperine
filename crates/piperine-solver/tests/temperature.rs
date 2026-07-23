//! Temperature protocol contract tests (ABI-19/20): the solver wires
//! `set_temperature` into the setup lifecycle, and a host running a
//! temperature sweep re-seeds every element through
//! `CircuitInstance::set_temperature`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use piperine_solver::abi::*;
use piperine_solver::prelude::*;

/// A recording element that captures every `set_temperature` value in
/// arrival order and records the global event sequence at each hook so the
/// test can prove `set_temperature` fired before the first `load_dc`.
struct RecordingDevice {
    temperatures: Arc<Mutex<Vec<f64>>>,
    event_seq: Arc<AtomicU64>,
    set_temp_seq: Arc<AtomicU64>,
    load_seq: Arc<AtomicU64>,
    n1: AnalogReference,
    n2: AnalogReference,
    r: f64,
}

impl RecordingDevice {
    fn new(
        n1: AnalogReference,
        n2: AnalogReference,
        temperatures: Arc<Mutex<Vec<f64>>>,
        event_seq: Arc<AtomicU64>,
        set_temp_seq: Arc<AtomicU64>,
        load_seq: Arc<AtomicU64>,
    ) -> Self {
        Self { temperatures, event_seq, set_temp_seq, load_seq, n1, n2, r: 1000.0 }
    }
}

impl AnalogDevice for RecordingDevice {
    fn set_temperature(&mut self, t: f64) {
        let seq = self.event_seq.fetch_add(1, Ordering::SeqCst);
        // Record only the most-recent set_temperature's sequence — the
        // ordering assertion is against the LAST set_temperature before
        // the FIRST load_dc.
        self.set_temp_seq.store(seq + 1, Ordering::SeqCst);
        self.temperatures.lock().expect("temperatures poisoned").push(t);
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        // Record the first load's sequence only.
        if self.load_seq.load(Ordering::SeqCst) == 0 {
            let seq = self.event_seq.fetch_add(1, Ordering::SeqCst);
            self.load_seq.store(seq + 1, Ordering::SeqCst);
        }
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for RecordingDevice {}
impl Introspect for RecordingDevice {}

impl Element for RecordingDevice {
    fn name(&self) -> &str { "r1" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// Ideal DC voltage source with its own branch-current unknown — biases the
/// recording resistor so the DC solver has something to solve.
struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for Vdc {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        let _ = alloc.branch("v", "i");
    }
    fn load_dc(
        &mut self,
        _s: &DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.v),
        ]
    }
}

impl DigitalDevice for Vdc {}
impl Introspect for Vdc {}
impl Element for Vdc {
    fn name(&self) -> &str { "v1" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// A pure-digital element with no analog side — proves `set_temperature`'s
/// default no-op leaves non-analog elements untouched.
struct PureDigital;
impl AnalogDevice for PureDigital {}
impl DigitalDevice for PureDigital {
    fn init(&mut self, _sink: &mut dyn EventSink) {}
}
impl Introspect for PureDigital {}
impl Element for PureDigital {
    fn name(&self) -> &str { "pure_digital" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}

/// ABI-19: when the solver enters setup for an analysis, it SHALL call
/// `set_temperature(t_nominal)` on every analog element after
/// `allocate_unknowns` (run by `CircuitBuilder::build`) and before the first
/// `load_*`. The DC solver's first solve drives both hooks; the recorder
/// captures the ordering.
#[test]
fn set_temperature_called_before_first_load() {
    let temperatures = Arc::new(Mutex::new(Vec::new()));
    let event_seq = Arc::new(AtomicU64::new(0));
    let set_temp_seq = Arc::new(AtomicU64::new(0));
    let load_seq = Arc::new(AtomicU64::new(0));

    let mut netlist = Netlist::new();
    let n_src = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let recorder = RecordingDevice::new(
        n_src.clone(),
        n_out.clone(),
        temperatures.clone(),
        event_seq.clone(),
        set_temp_seq.clone(),
        load_seq.clone(),
    );
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 1.0, n1: n_src.clone(), n2: gnd.clone(), branch }),
        Box::new(recorder),
    ];
    let circuit =
        CircuitInstance::from_devices_and_netlist("temp-order", elements, netlist);

    // Custom run temperature — the wiring must propagate this through
    // `setup_all` (called by the DC solver constructor).
    let run_temperature = 325.0;
    let ctx = Context {
        tolerances: Tolerances { temperature: run_temperature, ..Tolerances::default() },
    };
    let mut solver = Solver::new(circuit).with_context(ctx).build();
    let _ = solver.dc().unwrap().solve().unwrap();

    let temps = temperatures.lock().expect("temperatures poisoned").clone();
    assert!(!temps.is_empty(), "set_temperature must be called");
    assert!(
        temps.iter().any(|&t| (t - run_temperature).abs() < 1e-9),
        "run temperature {run_temperature} must be in observed temperatures {temps:?}"
    );

    let set_at = set_temp_seq.load(Ordering::SeqCst);
    let load_at = load_seq.load(Ordering::SeqCst);
    assert!(set_at > 0, "set_temperature must have fired");
    assert!(load_at > 0, "load_dc must have fired");
    assert!(
        set_at < load_at,
        "set_temperature (seq {set_at}) must precede the first load_dc (seq {load_at})"
    );
}

/// ABI-20: a temperature sweep point change calls `set_temperature(t_new)`
/// on every element and reports `Invalidation::Temperature` (recompute
/// constants → restamp). The next solve uses the new temperature.
#[test]
fn temperature_sweep_reseeds_every_element() {
    let temperatures = Arc::new(Mutex::new(Vec::new()));
    let event_seq = Arc::new(AtomicU64::new(0));
    let set_temp_seq = Arc::new(AtomicU64::new(0));
    let load_seq = Arc::new(AtomicU64::new(0));

    let mut netlist = Netlist::new();
    let n_src = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let n_out = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let recorder = RecordingDevice::new(
        n_src.clone(),
        n_out.clone(),
        temperatures.clone(),
        event_seq.clone(),
        set_temp_seq.clone(),
        load_seq.clone(),
    );
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 1.0, n1: n_src.clone(), n2: gnd.clone(), branch }),
        Box::new(recorder),
    ];
    let mut circuit =
        CircuitInstance::from_devices_and_netlist("temp-sweep", elements, netlist);

    let sweep_t = 380.0;
    let inv = circuit.set_temperature(sweep_t);
    assert_eq!(
        inv,
        piperine_solver::prelude::Invalidation::Temperature,
        "circuit-wide set_temperature must report Temperature invalidation"
    );

    let temps = temperatures.lock().expect("temperatures poisoned").clone();
    assert!(
        temps.iter().any(|&t| (t - sweep_t).abs() < 1e-9),
        "sweep temperature {sweep_t} must reach every element, observed {temps:?}"
    );
}

/// ABI-19 edge case: a device that does not override `set_temperature` is
/// unaffected — the default no-op leaves it untouched (backward compatible).
#[test]
fn default_set_temperature_is_noop() {
    let mut netlist = Netlist::new();
    let n_src = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 1.0, n1: n_src.clone(), n2: gnd.clone(), branch }),
        Box::new(PureDigital),
    ];
    let mut circuit =
        CircuitInstance::from_devices_and_netlist("default-noop", elements, netlist);

    let inv = circuit.set_temperature(350.0);
    assert_eq!(inv, piperine_solver::prelude::Invalidation::Temperature);
}
