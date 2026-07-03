//! [`BenchError`] — everything that can fail while running a `bench`
//! analysis: applying staged overrides, building the circuit, or solving.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BenchError {
    #[error("staging error: {0}")]
    Elaboration(#[from] piperine_lang::ElabError),
    #[error("lowering error: {0}")]
    Lowering(#[from] piperine_lang::LowerErrors),
    #[error("codegen error: {0}")]
    Codegen(#[from] piperine_codegen::CodegenError),
    #[error("solver error: {0}")]
    Solver(#[from] piperine_solver::error::Error),
    #[error("{0}")]
    Measurement(String),
    #[error("evaluation error: {0}")]
    Eval(#[from] piperine_lang::eval::EvalError),
}

impl From<BenchError> for piperine_lang::eval::EvalError {
    fn from(e: BenchError) -> Self {
        piperine_lang::eval::EvalError::Host(e.to_string())
    }
}
