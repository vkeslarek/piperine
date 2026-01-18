use std::hash::Hash;

pub mod array;
pub mod constant;
pub mod deriv;
pub mod faer;
pub mod iv;
pub mod linear;
pub mod newton_raphson;
pub mod num;
pub mod rand;
pub mod unit;
pub mod vector;

pub trait Symbol: Clone + Eq + Hash {}
