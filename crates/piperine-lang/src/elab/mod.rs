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
mod closures;
mod compound;
mod consts;
mod control;
mod contribute;
mod derivatives;
mod diagnose;
mod drives;
mod event_reads;
mod events;
mod exits;
mod files;
mod elaborator;
mod analog;
mod functions;
mod hoist;
mod host_sources;
mod inflow;
mod intrinsics;
mod known;
mod lower;
mod nature;
mod origin;
mod patterns;
mod place;
mod reach;
mod reactions;
mod returns;
mod records;
mod selectors;
mod structural;
mod templates;
mod unlowered;
mod unroll;
mod variants;

pub use consts::{Bindings, ConstEval, ConstFault, Exit, Folded};
pub use files::{FileFault, IncludedFiles};
pub use elaborator::Elaborator;
pub use host_sources::HostSourceParam;
pub use structural::Overrides;
