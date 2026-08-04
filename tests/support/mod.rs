//! Shared helpers for the root test targets — declarations only.
//!
//! A directory under `tests/` is a helper module, never a target: cargo
//! discovers targets from `tests/*.rs` alone, and
//! `suite_hygiene::integration_targets` matches the same top-level shape. So
//! the machinery several golden targets share lives here instead of being
//! re-typed per target.

pub mod ngspice;
