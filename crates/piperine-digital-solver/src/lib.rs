//! The event-driven digital engine: events and logic values (`events`), the
//! LRM §7.9 driver table (`drivers`), the evaluation interface a discrete
//! participant implements (`interface`), and the digital caps and error
//! (`limits`, `error`). It names no analog concept and depends on no other
//! crate of the workspace.

pub mod drivers;
pub mod error;
pub mod events;
pub mod interface;
pub mod limits;
