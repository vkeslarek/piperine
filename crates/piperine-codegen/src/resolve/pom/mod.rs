//! Lower a POM `Design` (PPR/PHDL) straight into each module's resolved
//! [`LoweredBody`] (`body.rs`), with one module per construct family it
//! resolves: expressions (`expr.rs`), statements (`stmt.rs`), analog runtime
//! operators (`analog_ops.rs`), and module structure — symbols, ports,
//! functions (`structure.rs`).

pub mod analog_ops;
pub mod body;
pub mod expr;
pub mod stmt;
pub mod structure;

pub use body::{LoweredBody, LowerError, LowerErrors, lower_bodies};
pub(crate) use body::LowerCtx;
