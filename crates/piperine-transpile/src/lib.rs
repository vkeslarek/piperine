//! The compiled backend (feature `ir-rust`, MD-63): an elaborated [`Ir`] printed as one Rust
//! source text, built by `rustc` into a `cdylib` that exports `piperine_abi_1`, cached by hash
//! and loaded into the solver. Each body is planned by `piperine-plan`, the plan the interpreter
//! runs, so both backends give the solver the same shapes. Module `transpiler` holds the
//! [`Transpiler`]: planning, printing and compiling a design.

mod abi_source;
mod cache;
mod compiled;
mod error;
mod load;
pub mod print;
pub mod rt;
mod rustc;
mod shape_check;
mod settings;
mod transpiler;

pub use abi_source::AbiSource;
pub use cache::{Cache, Entry, Key, Manifest};
pub use compiled::{Compiled, Report};
pub use error::Error;
pub use load::Loaded;
pub use rustc::Rustc;
pub use settings::Settings;
pub use transpiler::Transpiler;

#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/alone.rs"]
mod alone;
