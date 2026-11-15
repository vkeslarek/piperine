//! Circuit helpers of the analog assembly's targets: a netlist of numbered
//! nodes and branch unknowns, and an instance built from a fixture module.

use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_analog_solver::device::{AnalogBinding, AnalogInstance};
use piperine_analog_solver::netlist::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};

/// A netlist holding `nodes` node unknowns, then one branch unknown per name
/// of `branches`, in that order.
pub fn netlist(nodes: usize, branches: &[&str]) -> (Netlist, Vec<AnalogReference>) {
    let mut net = Netlist::new();
    let mut refs: Vec<AnalogReference> =
        (0..nodes).map(|i| net.connect_node(NodeIdentifier::Anonymous(i))).collect();
    refs.extend(branches.iter().map(|b| net.connect_branch(BranchIdentifier::from(*b))));
    (net, refs)
}

/// An instance of `module` at `path` with these bindings.
pub fn instance(path: &str, module: &'static Module, pins: Vec<AnalogBinding>) -> AnalogInstance {
    let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {path}: {e}"));
    AnalogInstance { path: path.into(), handle, pins }
}

/// `Unknown(r)`.
pub fn at(r: &AnalogReference) -> AnalogBinding {
    AnalogBinding::Unknown(r.clone())
}
