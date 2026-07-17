//! [`Error`] — everything that can fail while driving a simulation through
//! the host API: applying staged overrides, lowering, building the circuit,
//! or solving.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("staging error: {0}")]
    Elaboration(#[from] piperine_lang::ElabError),
    #[error("lowering error: {0}")]
    Lowering(#[from] piperine_codegen::ir::LowerErrors),
    #[error("codegen error: {0}")]
    Codegen(#[from] piperine_codegen::CodegenError),
    #[error("solver error: {0}")]
    Solver(#[from] piperine_solver::prelude::Error),
    #[error("{0}")]
    Measurement(String),
}
