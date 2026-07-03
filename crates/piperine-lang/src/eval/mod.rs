//! # Evaluator
//!
//! One tree-walking interpreter over the `fn`-body grammar (SPEC Part I §9),
//! shared by two hosts:
//!
//! - [`const_host::ConstHost`] — pure, backs [`crate::elab::const_eval::ConstEnv`]
//!   (array dims, structural `for`/`if`, param defaults, enum discriminants).
//! - a `SimHost` in `piperine-bench` — effectful, backs the `bench` block
//!   (SPEC_BENCH.md): runs analyses, stages overrides, does I/O.
//!
//! [`interp::Interpreter`] and [`interp::Host`] hold everything context-
//! independent; a host supplies name resolution, system-task dispatch, and
//! assignment targets (see the trait docs).

pub mod const_host;
pub mod error;
pub mod interp;
pub mod tasks;
pub use crate::value;

pub use error::EvalError;
pub use interp::{Callable, Flow, Host, Interpreter};
pub use tasks::{Task, TaskRegistry};
pub use value::{Closure, Object, Value};
