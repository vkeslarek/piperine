//! Look-ahead (abi.md §5, protocol step 2): run the discrete instants before
//! a target time, each under a sub-checkpoint, and stop before the first one
//! that touches the analog side.

use crate::engine::{Digital, Horizon, Mode, Outcome};
use crate::instant::Stepped;
use crate::tick::Tick;
use crate::error::DigitalError;

/// How far `Digital::advance` got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    /// Every due instant strictly before the target ran and none was a landing.
    Reached,
    /// The instant at `tick` is a landing and is untouched: a time point (R4), or, when
    /// `time_point` is false, only an `IRREVERSIBLE` reaction (R6) fires there.
    Landing { tick: Tick, time_point: bool },
    /// A reaction at the instant returned `FINISH`: the instant is untouched, and the step must land there.
    Final(Tick),
}

impl Digital {
    /// Runs the due instants strictly before `to`, in order. The first instant that R4 makes a time
    /// point, that fires an `IRREVERSIBLE` reaction or whose reactions return `FINISH` is restored
    /// to its sub-checkpoint and reported; every instant before it stays run. Reactions inside
    /// carry no analog sample (one that read one would be a time point), and `HORIZON` is capped at `to`.
    pub fn advance(&mut self, to: Tick) -> Result<Advance, DigitalError> {
        loop {
            let Some(s) = self.next_due() else { return Ok(Advance::Reached) };
            if s >= to {
                return Ok(Advance::Reached);
            }
            let sub = self.mark();
            let stepped = self.microsteps(s, &[], Horizon::uniform(to), Mode::default(), true);
            if matches!(&stepped, Ok(Stepped::Landing { .. }) | Ok(Stepped::Done(Outcome { finish: true, .. }))) {
                self.rewind(&sub);
            }
            self.release(sub);
            match stepped? {
                Stepped::Landing { time_point } => return Ok(Advance::Landing { tick: s, time_point }),
                Stepped::Done(outcome) if outcome.finish => return Ok(Advance::Final(s)),
                Stepped::Done(_) => {}
            }
        }
    }
}
