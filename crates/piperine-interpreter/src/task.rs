use std::collections::HashMap;
use std::fmt;
use crate::value::Value;
use crate::error::InterpreterError;
use crate::backend::SimulatorBackend;

/// A callable system task or function (`$name`).
///
/// Tasks return `None` (void). Functions return `Some(Value)`.
/// Register implementations via `SystemTaskRegistry::register()`.
///
/// Implement this trait to add new `$xxx` calls — ngspice analyses,
/// measurement functions, display routines, assertion handlers, etc.
pub trait SystemTask: fmt::Debug + Send + Sync {
    /// Name WITHOUT the `$` prefix (e.g., `"op"`, `"display"`, `"V"`).
    fn name(&self) -> &str;

    /// Execute the task.
    ///
    /// `arguments`: evaluated argument values, left to right.
    /// `simulator`: mutable access to the simulator backend.
    fn call(
        &self,
        arguments: Vec<Value>,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError>;
}

/// Registry of all known system tasks and functions.
///
/// Populated at startup by plugins via `Plugin::register_tasks()`.
#[derive(Default)]
pub struct SystemTaskRegistry {
    tasks: HashMap<String, Box<dyn SystemTask>>,
}

impl SystemTaskRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, task: Box<dyn SystemTask>) {
        self.tasks.insert(task.name().to_string(), task);
    }

    pub fn get(&self, name: &str) -> Option<&dyn SystemTask> {
        self.tasks.get(name).map(|b| b.as_ref())
    }
}
