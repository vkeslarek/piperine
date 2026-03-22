//! Error types for piperine-spice

pub type Result<T> = std::result::Result<T, SpiceError>;

#[derive(Debug, thiserror::Error, Clone)]
pub enum SpiceError {
    #[error("Ngspice error: {0}")]
    NgspiceError(#[from] crate::ngspice::NgspiceError),

    #[error("Failed to spawn worker: {0}")]
    WorkerSpawnFailed(String),

    #[error("Worker communication failed: {0}")]
    WorkerCommunicationFailed(String),

    #[error("Worker error: {0}")]
    WorkerError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    #[error("Pool is shut down")]
    PoolShutdown,

    #[error("Timeout waiting for worker")]
    Timeout,
}
