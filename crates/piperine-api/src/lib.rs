//! # piperine-api
//!
//! The host library (MD-20): it turns a project and a run request into a result. It prints
//! nothing and never exits; the command line (`piperine-cli`) is a shell over it, and the root
//! package `piperine` re-exports it.
//!
//! - [`failure`]: every way a request fails, typed, with what a host needs to report it.
//! - [`backend`]: which backend runs a design, and the order of the places that may name it.

pub mod backend;
pub mod failure;
pub mod prelude;

pub use backend::Backend;
pub use failure::Failure;
