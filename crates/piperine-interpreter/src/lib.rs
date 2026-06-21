pub mod value;
pub mod error;
pub mod backend;
pub mod plugin;
pub mod task;
pub mod interpreter;

pub use value::Value;
pub use error::InterpreterError;
pub use backend::{SimulatorBackend, AnalogCompilerBackend};
pub use plugin::Plugin;
pub use task::{SystemTask, SystemTaskRegistry};
pub use interpreter::{Interpreter, Scope};

use piperine_common::{EventAction, SimEventKind};
use piperine_parser::ast::{Expr, Stmt};

/// Allows the simulator backend to call back into the interpreter to run
/// always-block statement bodies during an active analysis.
pub trait InterpreterCallbacks: Send {
    fn fire_event(
        &mut self,
        kind: SimEventKind,
        time: f64,
        crossing_id: u32,
        handlers: &piperine_circuit::elaboration::AlwaysHandlerSet,
    ) -> EventAction;
}
