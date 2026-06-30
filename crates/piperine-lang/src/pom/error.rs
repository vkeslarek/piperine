//! POM errors.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ReflectError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("attribute `{0}` is not settable")]
    NotSettable(String),
    #[error("type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },
    #[error("index {index} out of range (len {len})")]
    OutOfRange { index: usize, len: usize },
    #[error("{0}")]
    Other(String),
}