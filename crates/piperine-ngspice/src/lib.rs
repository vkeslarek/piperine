mod backend;
mod hardware;
mod tasks;
pub mod expr_serializer;

pub use backend::NgspiceBackend;

use std::path::PathBuf;
use piperine_circuit::HardwareRegistry;
use piperine_interpreter::{Plugin, SimulatorBackend, SystemTaskRegistry};
use piperine_coordinator::pool::{ProcessPool, PoolConfig};

/// The ngspice plugin — registers all ngspice-backed hardware definitions,
/// system tasks, and provides the ngspice simulator backend.
///
/// Register this plugin at startup before any other plugin:
/// ```rust
/// runtime.register_plugin(Box::new(NgspicePlugin::default()));
/// ```
#[derive(Default)]
pub struct NgspicePlugin {
    /// Override path to the `piperine-worker` binary. `None` = auto-discover.
    pub worker_binary: Option<PathBuf>,
}

impl Plugin for NgspicePlugin {
    fn name(&self) -> &str { "ngspice" }

    fn register_hardware(&self, registry: &mut HardwareRegistry) {
        use hardware::*;
        registry.register(Box::new(SpiceResistor));
        registry.register(Box::new(SpiceVoltageSource));
        registry.register(Box::new(SpiceCurrentSource));
        registry.register(Box::new(SpiceCapacitor));
        registry.register(Box::new(SpiceBSourceV::new()));
        registry.register(Box::new(SpiceBSourceI::new()));
    }

    fn register_tasks(&self, registry: &mut SystemTaskRegistry) {
        use tasks::*;
        registry.register(Box::new(OperatingPointTask));
        registry.register(Box::new(TransientTask));
        registry.register(Box::new(VoltageTask));
        registry.register(Box::new(CurrentTask));
        registry.register(Box::new(DisplayTask));
    }

    fn simulator_backend(&self) -> Option<Box<dyn SimulatorBackend>> {
        let config = PoolConfig { size: 1, worker_binary: self.worker_binary.clone() };
        let mut pool = match ProcessPool::spawn(config) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("piperine-ngspice: failed to spawn worker: {e}");
                return None;
            }
        };
        let handle = pool.take_first();
        Some(Box::new(NgspiceBackend::new(handle.cmd, handle.resp)))
    }
}
