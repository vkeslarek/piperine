//! The DC stamp bypass is opt-in (P6/CLN-12): the cache is used only when
//! **every** element in the circuit declares `ElementCapabilities::BYPASS_OK`.
//!
//! Before this gate the bypass applied to any circuit whose solution stopped
//! moving, including devices whose stamps are not a pure function of their
//! terminal voltages — a stale stamp can satisfy the convergence test and lock
//! in a wrong operating point.

use piperine_solver::abi::{AnalogDevice, DcAnalysisState, DigitalDevice, Introspect, Stamp};
use piperine_solver::prelude::*;

/// A linear resistor whose declared capabilities are chosen per test, so the
/// same stamps can be presented as opted-in or not.
struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    bypass_ok: bool,
}

impl AnalogDevice for Resistor {
    fn load_dc(&mut self, _state: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for Resistor {}
impl Introspect for Resistor {}

impl Element for Resistor {
    fn name(&self) -> &str {
        "r"
    }

    fn capabilities(&self) -> ElementCapabilities {
        let base = ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC;
        if self.bypass_ok { base | ElementCapabilities::BYPASS_OK } else { base }
    }
}

/// A DC voltage source. Always declares `BYPASS_OK` — its stamp is a constant.
struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for Vdc {
    fn load_dc(&mut self, _state: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let branch = self.branch.clone();
        vec![
            Stamp::Matrix(self.n1.clone(), branch.clone(), 1.0),
            Stamp::Matrix(branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), branch.clone(), -1.0),
            Stamp::Matrix(branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(branch, self.v),
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
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
            | ElementCapabilities::BYPASS_OK
    }
}

/// Solve the same 10 V divider **twice** on one held `DcSolver`, with the
/// resistors declaring `BYPASS_OK` per `resistors_opt_in` (the source always
/// does). Returns `(mid-node voltage, bypass hits on the second solve)`.
///
/// Two solves is what makes a hit possible: the second starts from the
/// converged solution, so its first assemble sees an unmoved solution vector —
/// exactly the warm-start case the cache exists for.
fn divider_twice(resistors_opt_in: bool) -> (f64, usize) {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 10.0, n1: top.clone(), n2: gnd.clone(), branch }),
        Box::new(Resistor { r: 1000.0, n1: top, n2: mid.clone(), bypass_ok: resistors_opt_in }),
        Box::new(Resistor { r: 1000.0, n1: mid, n2: gnd, bypass_ok: resistors_opt_in }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("bypass", elements, netlist);
    let mut dc = circuit.dc(Context::default()).expect("dc solver");

    let first = dc.solve().expect("first solve");
    let hits_after_first = first.stats.bypass_hits;
    let second = dc.solve().expect("warm re-solve");
    let mid_v = second.get_node(&NodeIdentifier::Anonymous(2)).expect("mid node");
    (mid_v, second.stats.bypass_hits.saturating_sub(hits_after_first))
}

#[test]
fn an_all_opted_in_circuit_still_bypasses() {
    let (mid, hits) = divider_twice(true);
    assert!((mid - 5.0).abs() < 1e-9, "divider solves to 5 V, got {mid}");
    assert!(hits > 0, "every element declared BYPASS_OK, so the warm re-solve must reuse stamps");
}

#[test]
fn one_element_without_the_flag_disables_the_bypass_for_the_whole_circuit() {
    let (mid, hits) = divider_twice(false);
    assert_eq!(
        hits, 0,
        "the resistors never opted in — the bypass must not reuse stamps for any element"
    );
    assert!(
        (mid - 5.0).abs() < 1e-9,
        "the gate changes only whether stamps are reused, not the answer: got {mid}"
    );
}
