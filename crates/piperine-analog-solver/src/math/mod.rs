//! Numerical machinery, circuit-agnostic: sparse linear systems (`linear.rs`,
//! `faer.rs`), solution history (`circular_array.rs`), initial values
//! (`iv.rs`), scalar types (`num.rs`).

pub mod circular_array;
pub mod faer;
pub mod iv;
pub mod linear;
pub mod num;
pub mod singular;
pub mod integration;
pub mod newton_raphson;
