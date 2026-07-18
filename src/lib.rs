//! # piperine
//!
//! Thin re-export shell over [`piperine_api`] — the external Rust host
//! interface lives in `crates/piperine-api` (MD-20); this crate preserves
//! the `use piperine::…` name for Rust hosts. Everything public in the api
//! crate (items and modules: `session`, `results`, `waveform`, `hooks`,
//! `error`, `prelude`) is re-exported here unchanged.

pub use piperine_api::*;
