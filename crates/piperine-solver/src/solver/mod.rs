//! Analysis drivers: one driver per analysis (`dc.rs`, `ac.rs`, `transient.rs`,
//! `noise.rs`, `tf.rs`). The homotopy/Newton/stepper strategies
//! (`convergence.rs`), the config home (`config.rs`), and the shared run
//! configuration (`Context`, `Tolerances`, `Policy`) live in `crate::analyses`;
//! the shared types are re-exported here unchanged. The data contracts these
//! drivers exchange with elements live in `crate::analysis`.

pub use crate::analyses::{Context, Policy, Tolerances};

pub mod ac;
pub mod dc;
pub mod noise;
pub mod pss;
pub mod sens;
pub mod solve;
pub mod tf;
pub mod transient;
pub mod uic;
