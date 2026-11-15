//! The ABI engine (feature `solver-abi-digital`): the discrete rules of the
//! device ABI over byte nets, a commit queue and a levelized function order.
//! It lives beside the old engine and shares nothing with it.

pub mod dag;
pub mod net;
pub mod plan;
pub mod queue;
pub mod store;
pub mod tick;
