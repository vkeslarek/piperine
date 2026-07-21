use piperine_solver::prelude::*;
use piperine_solver::abi::{AnalogDevice, DigitalDevice, Introspect, SolverDomain};

struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for Resistor {
    fn load_dc(
        &mut self,
        _state: &piperine_solver::abi::DcAnalysisState,
        _ctx: &Context,
    ) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            piperine_solver::abi::Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            piperine_solver::abi::Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            piperine_solver::abi::Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            piperine_solver::abi::Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
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

struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for Vdc {
    fn load_dc(
        &mut self,
        _state: &piperine_solver::abi::DcAnalysisState,
        _ctx: &Context,
    ) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
        let branch = self.branch.clone();
        vec![
            piperine_solver::abi::Stamp::Matrix(self.n1.clone(), branch.clone(), 1.0),
            piperine_solver::abi::Stamp::Matrix(branch.clone(), self.n1.clone(), 1.0),
            piperine_solver::abi::Stamp::Matrix(self.n2.clone(), branch.clone(), -1.0),
            piperine_solver::abi::Stamp::Matrix(branch.clone(), self.n2.clone(), -1.0),
            piperine_solver::abi::Stamp::Rhs(branch, self.v),
        ]
    }
}

impl DigitalDevice for Vdc {}

impl Introspect for Vdc {}

impl Element for Vdc {
    fn name(&self) -> &str { "v" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

fn create_divider(r1_val: f64, r2_val: f64) -> CircuitInstance {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let v_branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let v1 = Vdc { v: 10.0, n1: top.clone(), n2: gnd.clone(), branch: v_branch };
    let r1 = Resistor { r: r1_val, n1: top.clone(), n2: mid.clone() };
    let r2 = Resistor { r: r2_val, n1: mid.clone(), n2: gnd.clone() };

    let elements: Vec<Box<dyn Element>> = vec![Box::new(v1), Box::new(r1), Box::new(r2)];
    CircuitInstance::from_devices_and_netlist("test", elements, netlist)
}

#[test]
fn test_solver_entry_point() {
    let mut solver1 = Solver::new(create_divider(1000.0, 1000.0)).build();
    let mut solver2 = Solver::new(create_divider(1000.0, 3000.0)).build(); // test AC2

    let mut dc1 = solver1.dc().unwrap();
    let res1 = dc1.solve().unwrap();
    let mid1 = res1.get_node(&NodeIdentifier::Anonymous(2)).unwrap();
    assert!((mid1 - 5.0).abs() < 1e-9);

    let mut dc2 = solver2.dc().unwrap();
    let res2 = dc2.solve().unwrap();
    let mid2 = res2.get_node(&NodeIdentifier::Anonymous(2)).unwrap();
    assert!((mid2 - 7.5).abs() < 1e-9);
}

struct BadElement {}
impl AnalogDevice for BadElement {}
impl DigitalDevice for BadElement {}
impl Introspect for BadElement {}
impl Element for BadElement {
    fn name(&self) -> &str { "bad" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::empty() }
    fn setup(&mut self, _ctx: &Context) -> std::result::Result<(), Error> {
        Err(Error::simple(SolverDomain::Element, "setup error"))
    }
}

#[test]
fn test_solver_setup_error_propagation() {
    let netlist = Netlist::new();
    let elements: Vec<Box<dyn Element>> = vec![Box::new(BadElement {})];
    let circuit = CircuitInstance::from_devices_and_netlist("bad", elements, netlist);
    
    let mut solver = Solver::new(circuit).build();
    let result = solver.dc();
    assert!(result.is_err());
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("setup error"));
}
