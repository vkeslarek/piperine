//! The event-driven digital engine: events and logic values (`events`), the
//! LRM §7.9 driver table (`drivers`), the evaluation interface a discrete
//! participant implements (`interface`), the caps and the error (`limits`,
//! `error`), the run state (`state`), the topology (`topology`) and the
//! two-phase delta-cycle scheduler (`scheduler`). It names no analog concept
//! and depends on no other crate of the workspace.

pub mod drivers;
pub mod error;
pub mod events;
pub mod interface;
pub mod limits;
pub mod scheduler;
pub mod state;
pub mod topology;
