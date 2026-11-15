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

/// An instance whose `tag` param (pin 0) is `tag`, for the recording fixtures.
pub fn tagged(path: &str, module: &'static Module, tag: u64, pins: Vec<AnalogBinding>) -> AnalogInstance {
    let bytes = (tag as f64).to_le_bytes();
    let handle = Handle::create(module, &[(0, &bytes)]).unwrap_or_else(|e| panic!("create {path}: {e}"));
    AnalogInstance { path: path.into(), handle, pins }
}

/// The eight bytes of an `F64`.
pub fn f64_bytes(v: f64) -> [u8; 8] {
    v.to_le_bytes()
}

/// The `F64` of eight bytes.
pub fn f64_of(b: &[u8]) -> f64 {
    f64::from_le_bytes(b[..8].try_into().expect("8 bytes"))
}
