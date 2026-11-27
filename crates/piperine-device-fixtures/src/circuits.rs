//! The circuits the solver's ABI tests assemble from the `circuit` devices: a node map, the divider family,
//! the `add_*` helpers that place a device on a `CircuitBuilder`, and a Newton strategy that counts its
//! damping calls.

use std::collections::HashMap;

use piperine_solver::abi::{AnalogReference, AnalogVariable, NodeIdentifier, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Net};

use crate::circuit::{Capacitor, Diode, OptionalResistor, Resistor, VoltageSource};

/// Name -> reference for the nodes a test builds.
pub struct NodeMap {
    nodes: HashMap<String, AnalogReference>,
}

impl NodeMap {
    /// An empty map.
    pub fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    /// Maps `name` to its reference.
    pub fn insert(&mut self, name: &str, reference: AnalogReference) {
        self.nodes.insert(name.to_string(), reference);
    }

    /// The reference of `name`.
    pub fn get(&self, name: &str) -> &AnalogReference {
        self.nodes.get(name).unwrap_or_else(|| panic!("node `{name}` not mapped"))
    }

    /// The identifier results are keyed by.
    pub fn id(&self, name: &str) -> NodeIdentifier {
        CircuitBuilding::node_id(self.get(name))
    }
}

/// A Newton strategy that counts its `damp_update` calls (one per iteration) and otherwise
/// behaves as [`NoDamping`].
pub struct Counting(pub std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl piperine_analog_solver::convergence::NewtonStrategy for Counting {
    fn damp_update(&self, prev: ndarray::ArrayView1<f64>, update: ndarray::ArrayViewMut1<f64>, context: &piperine_analog_solver::convergence::DampContext) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        piperine_analog_solver::convergence::NoDamping.damp_update(prev, update, context);
    }
    fn is_converged(
        &self,
        state: &piperine_analog_solver::math::circular_array::CircularArrayBuffer2<f64>,
        guess: &ndarray::ArrayView1<f64>,
        residual: &[f64],
        scale: &[f64],
        netlist: &piperine_analog_solver::netlist::Netlist,
        tolerances: &piperine_analog_solver::tolerances::Tolerances,
    ) -> bool {
        piperine_analog_solver::convergence::NoDamping.is_converged(state, guess, residual, scale, netlist, tolerances)
    }
    fn max_iter(&self, policy: &piperine_analog_solver::tolerances::Policy) -> usize {
        piperine_analog_solver::convergence::NoDamping.max_iter(policy)
    }
}

/// Places devices on a circuit builder and reads its nodes back.
pub struct CircuitBuilding;

impl CircuitBuilding {
    /// The `NodeIdentifier` behind a node reference (results are keyed by it).
    pub fn node_id(reference: &AnalogReference) -> NodeIdentifier {
        match reference.variable().as_ref() {
            AnalogVariable::Node(id) => id.clone(),
            other => panic!("not a node reference: {other:?}"),
        }
    }

    /// The solver `Net` of a mapped node.
    pub fn net_for(circuit: &CircuitInstance, map: &NodeMap, name: &str) -> Net {
        let var = map.get(name).variable().clone();
        circuit
            .nets()
            .into_iter()
            .find(|n| n.analog_variable().map(|v| **v == *var).unwrap_or(false))
            .unwrap_or_else(|| panic!("net `{name}` not solved"))
    }

    /// The divider family of `devices.rs`'s `divider_fixture`, on devices: `Top`
    /// (source over two resistors), `DiodeTop` (source, resistor, diode), `OptTop`
    /// (divider of `ROpt`s whose `w` is never given) and `RcTop` (source,
    /// resistor, capacitor). Nodes: `top`, `mid`.
    pub fn divider_fixture(top: &str) -> (CircuitInstance, NodeMap) {
        let mut b = CircuitBuilder::new(top);
        let gnd = b.ground();
        let top_node = b.node("top");
        let mid = b.node("mid");
        match top {
            "Top" => {
                CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top_node, &gnd);
                CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top_node, &mid);
                CircuitBuilding::add_resistor(&mut b, "r2", 1.0e3, &mid, &gnd);
            }
            "DiodeTop" => {
                CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top_node, &gnd);
                CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top_node, &mid);
                CircuitBuilding::add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
            }
            "OptTop" => {
                CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top_node, &gnd);
                b.device(OptionalResistor::module(), "r1", &[], &CircuitBuilding::nodes(&top_node, &mid));
                b.device(OptionalResistor::module(), "r2", &[], &CircuitBuilding::nodes(&mid, &gnd));
            }
            "RcTop" => {
                CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top_node, &gnd);
                CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top_node, &mid);
                CircuitBuilding::add_capacitor(&mut b, "c1", 1.0e-9, &mid, &gnd);
            }
            other => panic!("no circuit `{other}`"),
        }
        let circuit = b.build().expect("build");
        let mut map = NodeMap::new();
        map.insert("top", top_node);
        map.insert("mid", mid);
        (circuit, map)
    }

    /// The nodes helper `nodes`.
    fn nodes(p: &AnalogReference, n: &AnalogReference) -> [Port; 2] {
        [Port::Node(p.clone()), Port::Node(n.clone())]
    }

    /// The add resistor helper `add_resistor`.
    pub fn add_resistor(b: &mut CircuitBuilder, path: &str, r: f64, p: &AnalogReference, n: &AnalogReference) {
        b.device(Resistor::module(), path, &[("r", ParamValue::Real(r))], &CircuitBuilding::nodes(p, n));
    }

    /// The add capacitor helper `add_capacitor`.
    pub fn add_capacitor(b: &mut CircuitBuilder, path: &str, c: f64, p: &AnalogReference, n: &AnalogReference) {
        b.device(Capacitor::module(), path, &[("c", ParamValue::Real(c))], &CircuitBuilding::nodes(p, n));
    }

    /// The add diode helper `add_diode`.
    pub fn add_diode(b: &mut CircuitBuilder, path: &str, is_sat: f64, p: &AnalogReference, n: &AnalogReference) {
        b.device(Diode::module(), path, &[("is_sat", ParamValue::Real(is_sat))], &CircuitBuilding::nodes(p, n));
    }

    /// The add voltage source helper `add_voltage_source`.
    pub fn add_voltage_source(b: &mut CircuitBuilder, path: &str, dc: f64, p: &AnalogReference, n: &AnalogReference) {
        b.device(VoltageSource::module(), path, &[("dc", ParamValue::Real(dc))], &CircuitBuilding::nodes(p, n));
    }
}
