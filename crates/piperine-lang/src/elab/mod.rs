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

mod aggregate;
mod body; mod carrier_arrays;
mod carriers;
mod children;
mod closures;
mod compound;
mod consts;
mod control;
mod contribute;
mod delays; mod derivatives; mod noise; mod partial; mod session;
mod derived;
mod diagnose;
mod drive_loops; mod drive_overlaps; mod drives;
mod event_functions; mod event_reads;
mod events; mod expansion; mod symbolic;
mod exits;
mod files; mod override_text; mod table_model; mod tables;
mod elaborator; mod analog; mod element_commits; mod element_coverage; mod elements;
mod functions; mod generation; mod generic_bodies;
mod handles; mod hoist;
mod host_sources; mod levels;
mod inflow; mod instances;
mod intrinsics;
mod iterators;
mod lower;
mod methods;
mod nature;
mod origin;
mod patterns;
mod place;
mod reach; mod reach_through_templates; mod reach_values; mod node_choice;
mod repr;
mod reactions;
mod returns;
mod records;
mod selectors;
mod signal_flow;
mod structural;
mod templates;
mod types;
mod unlowered;
mod unroll;
mod variants;

pub use consts::{Bindings, CompositeKind, ConstEval, ConstFault, ConstTree, Exit};
pub use {files::{FileFault, IncludedFiles}, tables::{HexText, TableFault, TableText}};
pub use elaborator::Elaborator;
pub use host_sources::HostSourceParam;
pub use {override_text::LiteralText, structural::Overrides};
