use thiserror::Error;

#[derive(Debug, Error)]
pub enum ElaborationError {
    #[error("unknown module `{name}` — no plugin registered a HardwareDefinition with this name")]
    UnknownModule { name: String },

    #[error("missing required parameter `{parameter}` on instance `{instance}`")]
    MissingParameter { parameter: String, instance: String },

    #[error("type error in parameter `{parameter}`: {detail}")]
    TypeError { parameter: String, detail: String },

    #[error("connection error on instance `{instance}`: {detail}")]
    ConnectionError { instance: String, detail: String },

    #[error("no testbench found — expected a module with an `initial` block")]
    NoTestbench,
}
