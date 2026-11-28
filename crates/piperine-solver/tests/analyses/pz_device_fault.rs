//! A device that cannot be evaluated at the pole-zero assembly fails the
//! analysis with an error naming the instance (never a poisoned matrix).

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::pz_device_fault::{AcFault};

use piperine_abi::raw::Module;
use piperine_analog_solver::netlist::{AnalogVariable, BranchIdentifier, NodeIdentifier};
use piperine_solver::abi::Port;
use piperine_solver::prelude::{CircuitBuilder, Context, PoleZeroOptions};

#[test]
fn a_device_fault_in_the_pole_zero_assembly_is_an_error_naming_the_instance() {
    let mut b = CircuitBuilder::new("fault");
    let gnd = b.ground();
    let n = b.node("n");
    // A source-driven node so the analysis reaches the AC probes.
    b.device(
        FaultyDevice::piperine_abi_test_vsrc(),
        "v1",
        &[],
        &[Port::Node(n.clone()), Port::Node(gnd)],
    );
    b.device(FaultyDevice::module_fault(), "bad1", &[], &[Port::Node(n)]);
    let mut circuit = b.build().expect("build");
    let options = PoleZeroOptions {
        input_source: BranchIdentifier::new("v1", "f"),
        output: AnalogVariable::Node(NodeIdentifier::Anonymous(0)),
        output_ref: None,
    };
    let err = circuit.pz(options, Context::default()).err().expect("the fault surfaces");
    assert!(err.to_string().contains("bad1"), "{err}");
}

/// The device whose pole-zero run faults, and the module that declares it.
struct FaultyDevice;

impl FaultyDevice {

    /// The module fault helper `module_fault`.
    fn module_fault() -> &'static Module {
        AcFault::module()
    }

    /// The piperine abi test vsrc helper `piperine_abi_test_vsrc`.
    fn piperine_abi_test_vsrc() -> &'static Module {
        circuit::VoltageSource::module()
    }
}
