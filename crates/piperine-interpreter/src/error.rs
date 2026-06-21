use thiserror::Error;

#[derive(Debug, Error)]
pub enum InterpreterError {
    #[error("undefined variable `{name}`")]
    UndefinedVariable { name: String },

    #[error("type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("undefined system task `${name}`")]
    UndefinedSystemTask { name: String },

    #[error("simulator error: {0}")]
    SimulatorError(String),

    #[error("fatal error: {message}")]
    Fatal { message: String, exit_code: u32 },

    #[error("run failed: {message}")]
    RunFailed { message: String },

    #[error("{0}")]
    Other(String),
}
