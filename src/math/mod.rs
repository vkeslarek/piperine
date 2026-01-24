use std::hash::Hash;

pub mod array;
pub mod constant;
pub mod deriv;
pub mod expression;
pub mod faer;
pub mod iv;
pub mod linear;
pub mod newton_raphson;
pub mod newton_raphson2;
pub mod num;
pub mod rand;
pub mod unit;
pub mod vector;
pub mod circular_array;
mod pnjlim;

pub trait Symbol: Clone + Eq + Hash {}

impl Symbol for usize {}
