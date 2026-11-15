//! The runtime source: the few types generated code needs and Rust lacks. Each file is a module
//! of this crate, so its unit tests run under `cargo test` with no `rustc` call, and is embedded
//! as text to be written into every cache entry as `rt.rs`.

pub mod int;
pub mod real;

/// The text of `rt/real.rs`, as generated code compiles it.
pub const REAL: &str = include_str!("rt/real.rs");
/// The text of `rt/int.rs`, as generated code compiles it.
pub const INT: &str = include_str!("rt/int.rs");
