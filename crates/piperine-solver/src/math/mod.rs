//! Numerical machinery, circuit-agnostic: sparse linear systems (`linear.rs`,
//! `faer.rs`), the Newton-Raphson engine (`newton_raphson.rs`), integration
//! schemes + LTE (`integration.rs`), solution history (`circular_array.rs`),
//! initial values (`iv.rs`), scalar types + constants (`num.rs`,
//! `constant.rs`).

pub mod circular_array;
pub mod constant;
pub mod faer;
pub mod integration;
pub mod iv;
pub mod linear;
pub mod newton_raphson;
pub mod num;
