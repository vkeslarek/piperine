//! Analysis drivers: one driver per analysis (`dc.rs`, `ac.rs`, `transient.rs`,
//! `noise.rs`, `tf.rs`); the homotopy/Newton/stepper strategies compose in
//! `convergence.rs`. The shared run configuration (`Context`, `Tolerances`,
//! `Policy`) lives in `crate::analyses` and is re-exported here unchanged;
//! the data contracts these drivers exchange with elements live in
//! `crate::analysis`.

pub use crate::analyses::{Context, Policy, Tolerances};

pub mod ac;
pub mod convergence;
pub mod config;
pub mod dc;
pub mod noise;
pub mod pss;
pub mod sens;
pub mod solve;
pub mod tf;
pub mod transient;
pub mod uic;
