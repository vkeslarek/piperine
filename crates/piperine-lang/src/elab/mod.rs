//! # Elaboration of the M1 subset
//!
//! A type-checked [`Design`](crate::pom::Design) and the name of a top module in, an
//! [`Ir`](piperine_ir::Ir) out (or the reasons there is none). The design is only borrowed: every
//! result is a side table or the IR, never a write into the object model (the POM navigability
//! rule).
//!
//! Three passes, each writing through [`IrBuilder`](piperine_ir::IrBuilder): natures, bodies
//! (first-reach order, each function variant lowered when a call first reaches it), then the
//! instance tree. Any diagnostic means no `Ir`.

mod body;
mod carriers;
mod children;
mod compound;
mod consts;
mod contribute;
mod diagnose;
mod drives;
mod events;
mod elaborator;
mod analog;
mod functions;
mod hoist;
mod intrinsics;
mod known;
mod lower;
mod nature;
mod origin;
mod place;
mod reactions;
mod records;
mod unlowered;
mod variants;

pub use consts::{Bindings, ConstEval, ConstFault, Folded};
pub use elaborator::Elaborator;
