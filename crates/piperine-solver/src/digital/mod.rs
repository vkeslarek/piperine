//! The event-driven digital engine: events and logic values (`events.rs`),
//! the two-phase delta-cycle scheduler (`scheduler.rs`), and the evaluation
//! interface elements implement (`interface.rs`).

pub mod events;
pub mod interface;
pub mod scheduler;
pub mod state;
pub mod topology;

pub use events::{DigitalEvent, DigitalNet, LogicValue};
pub use interface::{DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use state::DigitalState;
pub use topology::DigitalTopology;
