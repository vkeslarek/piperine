//! # Elaboration of the M1 subset
//!
//! A type-checked [`Design`](crate::pom::Design) and the name of a top module in, an
//! [`Ir`](piperine_ir::Ir) out (or the reasons there is none). The design is only borrowed: every
//! result is a side table or the IR, never a write into the object model (the POM navigability
//! rule).
//!
//! Four passes, each writing through [`IrBuilder`](piperine_ir::IrBuilder): natures, functions,
//! bodies (first-reach order), then the instance tree. Any diagnostic means no `Ir`.

mod body;
mod children;
mod consts;
mod contribute;
mod diagnose;
mod elaborator;
mod analog;
mod functions;
mod lower;
mod nature;
mod origin;
mod place;

pub use consts::{Bindings, ConstEval, ConstFault};
pub use elaborator::Elaborator;
