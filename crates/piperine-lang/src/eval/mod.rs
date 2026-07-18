//! # Evaluator
//!
//! One tree-walking interpreter over the `fn`-body grammar (SPEC Part I §9),
//! backing [`crate::elab::const_eval::ConstEnv`] through the pure
//! [`const_host::ConstHost`] (array dims, structural `for`/`if`, param
//! defaults, enum discriminants).
//!
//! [`interp::Interpreter`] and [`interp::Host`] hold everything context-
//! independent; a host supplies name resolution and assignment targets (see
//! the trait docs).

pub mod const_host;
pub mod error;
pub mod interp;
pub mod tasks;
pub use crate::value;

pub use error::EvalError;
pub use interp::{Callable, Flow, Host, Interpreter};
pub use tasks::{Task, TaskRegistry};
pub use value::{Closure, InvokeClosure, Object, Value};
