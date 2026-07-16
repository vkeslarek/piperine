use piperine_solver::prelude::*;

struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl Element for Resistor {
    fn name(&self) -> &str { "r" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }

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

struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl Element for Vdc {
    fn name(&self) -> &str { "v" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }

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

#[test]
fn prelude_voltage_divider() {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let v_branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let v1 = Vdc { v: 10.0, n1: top.clone(), n2: gnd.clone(), branch: v_branch };
    let r1 = Resistor { r: 1000.0, n1: top.clone(), n2: mid.clone() };
    let r2 = Resistor { r: 1000.0, n1: mid.clone(), n2: gnd.clone() };

    let elements: Vec<Box<dyn Element>> = vec![Box::new(v1), Box::new(r1), Box::new(r2)];
    let mut circuit = CircuitInstance::from_devices_and_netlist("test", elements, netlist);

    let ctx = Context::default();
    let mut dc = circuit.dc(ctx).unwrap();
    let res = dc.solve().unwrap();

    let mid_val = res.get_node(&NodeIdentifier::Anonymous(2)).unwrap();
    assert!((mid_val - 5.0).abs() < 1e-9);
    
    // Use SolverStats and Error to verify they are available
    let _stats: &SolverStats = &res.stats;
}
