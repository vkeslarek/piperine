use piperine_solver::abi::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct LifecycleTestDevice {
    setup_calls: Arc<AtomicUsize>,
    destroy_calls: Arc<AtomicUsize>,
    fail_setup: bool,
}

impl AnalogDevice for LifecycleTestDevice {}

impl DigitalDevice for LifecycleTestDevice {}

impl Introspect for LifecycleTestDevice {}

impl Element for LifecycleTestDevice {
    fn name(&self) -> &str {
        "LifecycleTestDevice"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }

    fn setup(&mut self, _ctx: &Context) -> Result<()> {
        self.setup_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_setup {
            Err(Error::simple(SolverDomain::Element, "test setup error"))
        } else {
            Ok(())
        }
    }

    fn destroy(&mut self) {
        self.destroy_calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_lifecycle_hooks_called() {
    let setup_calls = Arc::new(AtomicUsize::new(0));
    let destroy_calls = Arc::new(AtomicUsize::new(0));

    let dev = LifecycleTestDevice {
        setup_calls: setup_calls.clone(),
        destroy_calls: destroy_calls.clone(),
        fail_setup: false,
    };

    let netlist = Netlist::new();
    let mut circuit = CircuitInstance::from_devices_and_netlist(
        "test_circuit",
        vec![Box::new(dev)],
        netlist,
    );

    let context = Context::default();

    // Setup is called in DC analysis construction
    let mut dc = circuit.dc(context.clone()).unwrap();
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
    let _ = dc.solve().unwrap();

    // Running another analysis on the same circuit should NOT call setup again
    let _ac = circuit.ac(context.clone()).unwrap();
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
    assert_eq!(destroy_calls.load(Ordering::SeqCst), 0);

    // Drop the circuit, which should call destroy
    drop(circuit);
    assert_eq!(destroy_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_setup_error_propagates() {
    let setup_calls = Arc::new(AtomicUsize::new(0));
    let destroy_calls = Arc::new(AtomicUsize::new(0));

    let dev = LifecycleTestDevice {
        setup_calls: setup_calls.clone(),
        destroy_calls: destroy_calls.clone(),
        fail_setup: true,
    };

    let netlist = Netlist::new();
    let mut circuit = CircuitInstance::from_devices_and_netlist(
        "test_circuit",
        vec![Box::new(dev)],
        netlist,
    );

    let context = Context::default();

    // DC analysis construction should fail due to setup error
    let result = circuit.dc(context);
    assert!(result.is_err());
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
}
