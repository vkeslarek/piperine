//! The event-driven digital engine: events and logic values (`events.rs`),
//! the two-phase delta-cycle scheduler (`scheduler.rs`), the evaluation
//! interface elements implement (`interface.rs`), and the LRM §7.9 per-net
//! driver-receiver table (`drivers.rs`, T34/LRMC-C7-05).

pub mod drivers;
pub mod events;
pub mod interface;
pub mod scheduler;
pub mod state;
pub mod topology;

pub use drivers::{DriverId, DriverKind, DriverRecord, DriverTable, Strength};
pub use events::{DigitalEvent, DigitalNet, LogicValue};
pub use state::DigitalState;
pub use topology::DigitalTopology;
