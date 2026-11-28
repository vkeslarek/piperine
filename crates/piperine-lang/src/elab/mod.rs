//! # Elaboration of the supported subset
//!
//! A type-checked [`Design`](crate::pom::Design) and the name of a top module in, an
//! [`Ir`](piperine_ir::Ir) out (or the reasons there is none). The design is only borrowed: every
//! result is a side table or the IR, never a write into the object model (the POM navigability
//! rule).
//!
//! Three passes, each writing through [`IrBuilder`](piperine_ir::IrBuilder): natures, bodies
//! (first-reach order, each function variant lowered when a call first reaches it), then the
//! instance tree. Any diagnostic means no `Ir`.

mod value;
mod analog_state; mod digital_state; mod function_state; mod structure_state;
mod body; mod bundles; mod carrier_arrays;
mod carriers;
mod children;
mod consts;
mod contribute;
mod delays; mod derivatives; mod noise; mod partial; mod session;
mod diagnose;
mod drive_loops; mod drive_overlaps; mod drives;
mod event_functions; mod event_reads;
mod events; mod expansion; mod symbolic;
mod override_text;
mod elaborator; mod analog; mod element_commits; mod element_coverage; mod elements;
mod functions; mod generation; mod generic_bodies;
mod handles; mod hoist;
mod host_sources;
mod inflow; mod instances;
mod loops;
mod nature;
mod origin;
mod place;
mod reach; mod node_choice; mod resolvers;
mod reactions;
mod records;
mod selectors;
mod signal_flow;
mod structural;
mod templates;
mod unroll;
mod variants;
pub use consts::{Bindings, CompositeKind, ConstEval, ConstFault, ConstTree, Exit};
pub use {value::files::{FileFault, IncludedFiles}, value::tables::{HexText, TableFault, TableText}};
pub use elaborator::{Elaborator, RunStatistics};
pub use host_sources::HostSourceParam;
pub use {override_text::LiteralText, structural::Overrides};
