//! Composed-surface contract test (SS-04, MD-01 amendment).
//!
//! `Element` is the conjunction `AnalogDevice + DigitalDevice + Introspect`;
//! the object is not split and there is no downcast — a pure-analog device is
//! `impl AnalogDevice for R { … }` + explicitly empty `DigitalDevice` /
//! `Introspect` blocks + `impl Element { name, capabilities }`, and the empty
//! blocks document that the device is deliberately digital-inert. This test
//! proves that single-concern implementation compiles against the composed
//! surface and solves through the real DC driver.

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, Introspect, Netlist, NodeIdentifier, Stamp,
};
use piperine_solver::prelude::{CircuitInstance, Context};

/// A pure-analog resistor: only the analog concern is implemented
/// non-trivially; the digital and introspection surfaces stay at their
/// defaults (explicitly empty blocks — MD-13 rule 5, no derive).
struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for Resistor {
    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
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
    fn name(&self) -> &str { "r" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// An ideal DC source, likewise analog-only.
struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for Vdc {
    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
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
    fn name(&self) -> &str { "v" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// The single-object contract: one `Element` bound covers all three concerns
/// — the solver speaks `Element`, never a downcast to a facet.
fn assert_composed<T: Element + ?Sized>(device: &T) -> &T {
    assert!(device.capabilities().contains(ElementCapabilities::ANALOG));
    device
}

#[test]
fn analog_only_double_solves_through_the_composed_surface() {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let v1 = Vdc { v: 10.0, n1: top.clone(), n2: gnd.clone(), branch };
    let r1 = Resistor { r: 1000.0, n1: top.clone(), n2: mid.clone() };
    let r2 = Resistor { r: 3000.0, n1: mid.clone(), n2: gnd.clone() };

    // One object, one `Element` bound — boxed as `dyn Element`, not a facet.
    let devices: Vec<Box<dyn Element>> =
        vec![Box::new(v1), Box::new(r1), Box::new(r2)];
    for dev in &devices {
        let _ = assert_composed(dev.as_ref());
    }

    let mut circuit = CircuitInstance::from_devices_and_netlist("divider", devices, netlist);
    let res = circuit.dc(Context::default()).unwrap().solve().unwrap();

    // 10 V · 3k/(1k+3k) = 7.5 V — the analog-only double drives the solve.
    let vmid = res.get_node(&NodeIdentifier::Anonymous(2)).unwrap();
    assert!((vmid - 7.5).abs() < 1e-9, "vmid = {vmid}");
}

#[test]
fn defaulted_concerns_are_inert() {
    let gnd = NodeIdentifier::Gnd;
    let mut netlist = Netlist::new();
    let n = netlist.connect_node(gnd);
    let r = Resistor { r: 1000.0, n1: n.clone(), n2: n };

    // Digital concern defaults: drives/reads nothing, snapshots nothing.
    let ports = r.boundary();
    assert!(ports.inputs.is_empty() && ports.outputs.is_empty());
    assert!(r.digital_hidden_snapshot().is_none());

    // Introspection concern defaults: no params, queries, terminals, opvars.
    assert!(r.list_params().is_empty());
    assert!(r.list_queries().is_empty());
    assert!(r.list_terminals().is_empty());
    assert!(r.read_opvars().is_empty());
    assert!(r.get_param("r").is_none());
}
