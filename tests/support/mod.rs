//! Shared helpers for the root test targets — declarations only.
//!
//! A directory under `tests/` is a helper module, never a target: cargo
//! discovers targets from `tests/*.rs` alone, and
//! `suite_hygiene::integration_targets` matches the same top-level shape. So
//! the machinery several golden targets share lives here instead of being
//! re-typed per target.

// Each target compiles the whole module but consumes only the domain it
// covers, so the parts it does not call are "dead" from that target's point of
// view. The exemption sits on the item it excuses (MD-33), not on the file.
#[allow(dead_code)]
pub mod ngspice;

// repo-walking helpers shared by suite_hygiene.rs and lrm_section_coverage.rs
// (T3/T4, lrm-conformance-wave2) — see the same MD-33 rationale above.
#[allow(dead_code)]
pub mod sources;
