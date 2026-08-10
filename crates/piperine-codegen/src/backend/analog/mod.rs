//! Analog backend — T18/T19/T20 (LOV-16): IR `region analog` -> a residual
//! function via Cranelift, its Jacobian ([`jacobian`]) via
//! `piperine_ir::diff`, and reactive/companion state ([`reactive`]).
//!
//! `AnalogBackend::compile` JITs one analog [`Block`]'s op sequence through
//! `backend::Engine` (the shared engine in the parent module).
//! `contribute_through`/`force_across` become residual rows in
//! [`AnalogOutputs`]; structured `If`/`Match` emit real Cranelift branches
//! (`backend::OpCtx::compile_if`/`compile_match`) — there is no flattening
//! pass, matching the IR's own "control stays structured" invariant (D9).

pub mod jacobian;
pub mod reactive;
mod backend;

pub use backend::{AnalogBackend, AnalogOutputs, CompiledAnalogBlock};
