//! Constraint-margin evaluation points in DC (dv-core T18, DVC-11): the
//! driver calls the `Element::evaluate_constraints` hook **only** at the
//! final converged DC solution — exactly once per solve, never per Newton
//! iteration, and never at a gmin-stepping or source-stepping homotopy
//! stage (design §C3: each intermediate is non-physical by construction).
//! Proven at the ABI level with a counting test element: a plain solve
//! and a homotopy-forced solve both evaluate exactly once, and `checks=off`
//! never evaluates at all.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, CheckPosture, DcAnalysisState, DigitalDevice,
    Element, ElementCapabilities, Introspect, Netlist, NodeIdentifier, Stamp,
};
use piperine_solver::prelude::{CircuitInstance, Context, Policy};

/// A stamp-free observer that counts its `evaluate_constraints` calls and
/// records the last evaluation coordinate. It declares `EMITS_VALIDATION`
/// (so the drivers evaluate it) but loads nothing into the MNA system —
/// the circuit it rides solves exactly as if it were absent.
struct CountingConstraints {
    calls: Arc<AtomicUsize>,
    last_t: Arc<std::sync::Mutex<f64>>,
}

impl AnalogDevice for CountingConstraints {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        Vec::new()
    }
}
impl DigitalDevice for CountingConstraints {}
impl Introspect for CountingConstraints {}
impl Element for CountingConstraints {
    fn name(&self) -> &str {
        "cc"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::EMITS_VALIDATION
    }
    fn evaluate_constraints(&mut self, _state: &piperine_solver::abi::CircularArrayBuffer2<f64>, t: f64) {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.last_t.lock().unwrap() = t;
    }
}

/// A pure voltage source forcing `node` to `volts` (branch-current unknown).
struct Vdc {
    volts: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}
impl AnalogDevice for Vdc {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.volts),
        ]
    }
}
impl DigitalDevice for Vdc {}
impl Introspect for Vdc {}
impl Element for Vdc {
    fn name(&self) -> &str {
        "v"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// A linear resistor (1/r conductance).
struct Res {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}
impl AnalogDevice for Res {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}
impl DigitalDevice for Res {}
impl Introspect for Res {}
impl Element for Res {
    fn name(&self) -> &str {
        "r"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// Shockley diode — a stiff nonlinear load plain Newton (starved of
/// iterations) fails on, so the convergence plan escalates to gmin
/// stepping (mirrors `checkpoint_restore.rs`'s homotopy fixture).
struct Diode {
    is: f64,
    vt: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}
impl AnalogDevice for Diode {
    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let v = |r: &AnalogReference| {
            r.idx()
                .and_then(|i| s.latest().and_then(|row| row.get(i).copied()))
                .unwrap_or(0.0)
        };
        let vd = v(&self.n1) - v(&self.n2);
        let ex = (vd / self.vt).exp();
        let id = self.is * (ex - 1.0);
        let gd = self.is / self.vt * ex;
        let ieq = id - gd * vd;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), gd),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), gd),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -gd),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -gd),
            Stamp::Rhs(self.n1.clone(), -ieq),
            Stamp::Rhs(self.n2.clone(), ieq),
        ]
    }
}
impl DigitalDevice for Diode {}
impl Introspect for Diode {}
impl Element for Diode {
    fn name(&self) -> &str {
        "d"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

fn counters() -> (Arc<AtomicUsize>, Arc<std::sync::Mutex<f64>>) {
    (Arc::new(AtomicUsize::new(0)), Arc::new(std::sync::Mutex::new(0.0)))
}

/// DVC-11: a converged DC solve evaluates the constraint hook exactly
/// once — at the final solution (coordinate 0.0), never per Newton
/// iteration (a linear solve takes iterations; the count stays 1).
#[test]
fn dc_evaluates_constraints_exactly_once_at_the_final_solution() {
    let (calls, last_t) = counters();
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { volts: 1.0, n1: node.clone(), n2: gnd, branch }),
        Box::new(CountingConstraints { calls: calls.clone(), last_t: last_t.clone() }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("gate", elements, netlist);
    circuit.dc(Context::default()).unwrap().solve().expect("DC solves");

    assert_eq!(calls.load(Ordering::SeqCst), 1, "evaluated exactly once — at the final solution, not per iteration");
    assert_eq!(*last_t.lock().unwrap(), 0.0, "the DC evaluation coordinate is 0.0");
}

/// DVC-11: a solve that requires gmin stepping evaluates the hook only at
/// the final converged solution — the homotopy's intermediate stages
/// (each a converged Newton of its own) are never evaluation points.
#[test]
fn dc_homotopy_stages_are_never_evaluated() {
    let (calls, _) = counters();
    let mut netlist = Netlist::new();
    let src = netlist.connect_node(NodeIdentifier::Anonymous(10));
    let anode = netlist.connect_node(NodeIdentifier::Anonymous(11));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { volts: 1.0, n1: src.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(Res { r: 1000.0, n1: src, n2: anode.clone() }),
        Box::new(Diode { is: 1e-14, vt: 0.025_852, n1: anode.clone(), n2: gnd }),
        Box::new(CountingConstraints { calls: calls.clone(), last_t: Arc::new(std::sync::Mutex::new(0.0)) }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("diode-homotopy", elements, netlist);

    // max_iter=4 starves plain Newton on the stiff diode so the plan
    // escalates to gmin stepping (proven by the recorded strategy).
    let mut dc = circuit.dc(Context::default()).unwrap();
    dc.policy = Policy { max_iter: 4, ..Policy::default() };
    let res = dc.solve().expect("gmin stepping converges");
    assert_eq!(
        res.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "the fixture must actually engage the homotopy"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only the final converged solution is an evaluation point — every homotopy stage is skipped"
    );
}

/// DVC-12 AC3: `checks=off` never evaluates — the hook count stays at
/// zero across the whole solve.
#[test]
fn dc_off_posture_never_evaluates() {
    let (calls, _) = counters();
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { volts: 1.0, n1: node.clone(), n2: gnd, branch }),
        Box::new(CountingConstraints { calls: calls.clone(), last_t: Arc::new(std::sync::Mutex::new(0.0)) }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("gate", elements, netlist);
    let off = Context { checks: CheckPosture::Off, ..Default::default() };
    circuit.dc(off).unwrap().solve().expect("off completes");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "off calls nothing");
}
