//! The reference interpreter (feature `ir-interp`): an elaborated [`Ir`] run through the solver
//! by walking each body's `analog` block over dual numbers. It spawns no process and reads no
//! file; the solver meets it through shapes and `eval` only (abi.md). Module `interpreter` holds the
//! [`Interpreter`] and module `rejected` the [`Rejected`] error of an `Ir` that fails `check()`.

mod composite;
mod device;
mod dual;
mod generics;
mod interpreter;
mod operators;
mod param_types;
mod rejected;
mod widths;
mod walk;

pub use device::{BodyFactory, Interpreted};
pub use dual::Dual;
pub use piperine_plan::BodyPlan;
pub use walk::{Value, Walk};
pub use interpreter::Interpreter;
pub use rejected::Rejected;
