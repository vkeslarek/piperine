//! Shared support for `piperine-ir`'s integration tests: the differential
//! harness (T14, LOV-11) that runs one [`fixture::Fixture`] under two
//! independently-implemented [`harness::Evaluator`]s and reports the first
//! disagreement by name.
//!
//! A directory under `tests/` is a helper module, never a target — cargo
//! discovers targets from `tests/*.rs` alone (the same convention as root
//! `tests/support/`, `CLAUDE.md` "Where the tests are").

pub mod checked_eval;
pub mod fixture;
pub mod harness;
pub mod interpreter_eval;
