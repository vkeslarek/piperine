//! # piperine
//!
//! Thin re-export shell over [`piperine_api`]: the host library lives in
//! `crates/piperine-api` (MD-20); this crate keeps the `use piperine::…` name
//! for Rust hosts. Everything public in the api crate is re-exported here
//! unchanged.

pub use piperine_api::*;
