//! The event-driven digital engine: events and logic values (`events.rs`),
//! the two-phase delta-cycle scheduler (`scheduler.rs`), the evaluation
//! interface elements implement (`interface.rs`), and the LRM chapter 8
//! stratified event-region ordering primitive (`regions.rs`, T51 —
//! re-exported through `crate::abi`, not here; see that module's own doc
//! comment for why).

pub mod events;
pub mod interface;
pub mod regions;
pub mod scheduler;
pub mod state;
pub mod topology;

pub use events::{DigitalEvent, DigitalNet, LogicValue};
pub use state::DigitalState;
pub use topology::DigitalTopology;
