//! The owner's synchronization protocol (abi.md §5, §10), written once over
//! two traits: `DiscreteSide` and `AnalogSide` (`sides`). `transient` holds
//! `Protocol` and the transient loop, `operating_point` the operating point,
//! `sink` the delivery of observations, `host` the host's waiters and writes. It
//! names no item of either sub-solver crate; `solver-abi-integration`
//! implements the traits for them and calls the protocol from the drivers.

mod host;
mod operating_point;
mod sides;
mod sink;
mod step;
mod transient;

pub use sides::{
    Accepted, Advance, AnalogSide, AnalysisKind, Attempt, Boundary, DiscreteSide, Horizon, InstantMode, Observation,
    ObservationKind, Outcome, Restart, Shadow, Tick,
};
pub use host::{HostCx, SetOutcome, Waiter};
pub use sink::Sink;
pub use step::{StepObserver, StepView};
pub use transient::{Ended, Protocol, ProtocolOptions};
