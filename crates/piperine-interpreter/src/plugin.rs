use crate::task::SystemTaskRegistry;
use crate::backend::{SimulatorBackend, AnalogCompilerBackend};
use piperine_circuit::HardwareRegistry;

/// A Piperine plugin — the primary extension mechanism.
///
/// Plugins bring hardware definitions, system tasks, and simulator/compiler
/// backends into the runtime. The main binary registers plugins at startup;
/// all capabilities flow from the registered set.
pub trait Plugin: Send + Sync {
    /// Unique plugin identifier (e.g., `"ngspice"`, `"openvaf"`, `"xyce"`).
    fn name(&self) -> &str;

    /// Register hardware element definitions this plugin provides.
    ///
    /// Called once before elaboration. Implementations call
    /// `registry.register(Box::new(MyElement))` for each element type.
    fn register_hardware(&self, _registry: &mut HardwareRegistry) {}

    /// Register system tasks (`$xxx`) this plugin provides.
    ///
    /// Called once before interpretation begins. Implementations call
    /// `registry.register(Box::new(MyTask))` for each task.
    fn register_tasks(&self, _registry: &mut SystemTaskRegistry) {}

    /// Provide a live simulator backend session.
    ///
    /// Return `Some(backend)` if this plugin owns the simulator.
    /// Only the first plugin that returns `Some` is used — register simulator
    /// plugins before device-library plugins. Return `None` if not applicable.
    fn simulator_backend(&self) -> Option<Box<dyn SimulatorBackend>> { None }

    /// Provide an analog compiler backend (Phase 2).
    ///
    /// Return `Some(compiler)` if this plugin can compile Verilog-A modules.
    /// Used by the `$pre_osdi` system task and the elaborator for analog modules.
    fn analog_compiler(&self) -> Option<Box<dyn AnalogCompilerBackend>> { None }
}
