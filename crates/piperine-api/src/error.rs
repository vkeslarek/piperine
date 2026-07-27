//! [`Error`] — everything that can fail while driving a simulation through
//! the host API: applying staged overrides, lowering, building the circuit,
//! or solving.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("staging error: {0}")]
    Elaboration(#[from] piperine_lang::ElabError),
    #[error("lowering error: {0}")]
    Lowering(#[from] piperine_codegen::resolve::LowerErrors),
    #[error("codegen error: {0}")]
    Codegen(#[from] piperine_codegen::CodegenError),
    #[error("solver error: {0}")]
    Solver(#[from] piperine_solver::prelude::Error),
    #[error("{0}")]
    Measurement(String),
    /// A model-layer failure while loading or navigating a design
    /// (`crate::model`): a parse/elaboration diagnostic, a malformed selector,
    /// an ambiguous path. Displays the diagnostic bare so a host can surface it
    /// verbatim.
    #[error("{0}")]
    Model(String),
    /// A named thing the model was asked for does not exist: a module, an
    /// instance, a port, a selector that resolved to nothing. Split from
    /// [`Error::Model`] so a host can map "absent" to its own lookup-failure
    /// type (the Python binding raises `KeyError`) without matching on message
    /// text.
    #[error("{0}")]
    NotFound(String),
    #[error("plugin: {0}")]
    Plugin(String),
}
