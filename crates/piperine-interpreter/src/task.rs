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

    /// Execute the task with positional arguments only (legacy interface).
    ///
    /// `arguments`: evaluated positional argument values, left to right.
    /// `simulator`: mutable access to the simulator backend.
    fn call(
        &self,
        arguments: Vec<Value>,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError>;

    /// Execute the task with both positional and named arguments.
    ///
    /// Default implementation ignores named args and delegates to `call`.
    /// Override to support `$func(mandatory, optional_name = val)` syntax.
    fn call_named(
        &self,
        positional: Vec<Value>,
        _named: HashMap<String, Value>,
        simulator: &mut dyn SimulatorBackend,
    ) -> Result<Option<Value>, InterpreterError> {
        self.call(positional, simulator)
    }
}

/// Registry of all known system tasks and functions.
///
/// `default()` pre-populates all stdlib tasks (`$display`, `$fatal`, etc.).
/// Plugins then add backend-specific tasks via `Plugin::register_tasks()`.
pub struct SystemTaskRegistry {
    tasks: HashMap<String, Box<dyn SystemTask>>,
}

impl Default for SystemTaskRegistry {
    fn default() -> Self {
        let mut reg = Self { tasks: HashMap::new() };
        crate::stdlib::register_stdlib(&mut reg);
        reg
    }
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
