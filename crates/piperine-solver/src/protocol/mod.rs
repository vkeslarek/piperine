//! The owner's synchronization protocol (abi.md §5, §10), written once over
//! two traits: `DiscreteSide` and `AnalogSide` (`sides`). It names no item of
//! either sub-solver crate; `solver-abi-integration` implements the traits for
//! them and calls the protocol from the drivers.

mod sides;

pub use sides::{
    Accepted, Advance, AnalogSide, AnalysisKind, Attempt, Boundary, DiscreteSide, InstantMode, Observation,
    ObservationKind, Outcome, Restart, Shadow, Tick,
};
