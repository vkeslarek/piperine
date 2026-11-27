//! The device calls of the ABI: the generic `extern "C"` thunks over a Rust `Device`, the solver-side
//! `Handle` that calls them, the factory path, the worked examples of abi.md section 11 through both,
//! the finite-difference verifier of declared partials, and the allocation-free eval. Each former target
//! is a module of this one.

mod caller;
mod eval_allocation;
mod examples;
mod factory;
mod thunks;
mod verify;
