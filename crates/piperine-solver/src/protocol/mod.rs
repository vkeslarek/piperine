//! The owner's synchronization protocol (abi.md §5, §10), written once over
//! two traits: `DiscreteSide` and `AnalogSide` (`sides`). `transient` holds
//! `Protocol` and the transient loop, `operating_point` the operating point,
//! `sink` the delivery of observations, `host` the host's waiters and writes, `retake` the
//! retake of a step whose crossings change what analog reads (§15.2 rule 4). It
//! names no item of either sub-solver crate; `solver-abi-integration`
//! implements the traits for them and calls the protocol from the drivers.

mod host;
mod operating_point;
mod retake;
mod sides;
mod sink;
mod statistics;
mod step;
mod transient;

pub use sides::{
    Accepted, Advance, AnalogSide, AnalysisKind, Attempt, Boundary, DiscreteSide, Horizon, InstantMode, LandingCause,
    Observation, ObservationKind, Outcome, Restart, Shadow, Tick,
};
pub use host::{HostCx, SetOutcome, Waiter};
pub use sink::Sink;
pub use statistics::{StepEnd, StepStatistics};
pub use step::{StepObserver, StepView};
pub use transient::{Ended, Protocol, ProtocolOptions};
