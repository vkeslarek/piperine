//! Integer time (abi.md §6 C1). Every scheduled time is a signed 64-bit count
//! of ticks; the tick length is a session option.

use crate::error::DigitalError;

/// A point in time: a count of ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub i64);

/// The session's tick length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    pub seconds_per_tick: f64,
}

impl Default for Resolution {
    /// One femtosecond.
    fn default() -> Self {
        Resolution { seconds_per_tick: 1e-15 }
    }
}

/// When a commit with a given delay comes due.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// At the next microstep of the same instant.
    NextMicrostep,
    /// At microstep 0 of this tick.
    At(Tick),
    /// Never.
    Never,
}

impl Resolution {
    /// The tick nearest `seconds` (halves round away from zero); E0502 when it
    /// is not a representable count.
    pub fn tick_of(&self, seconds: f64) -> Result<Tick, DigitalError> {
        let count = (seconds / self.seconds_per_tick).round();
        // `i64::MAX as f64` is 2^63, which is itself out of range.
        if !count.is_finite() || count >= i64::MAX as f64 || count < i64::MIN as f64 {
            return Err(DigitalError::TickRange { code: "E0502", seconds });
        }
        Ok(Tick(count as i64))
    }

    /// When a commit made at `now` with delay `after` (seconds) comes due.
    /// A NaN delay is E0435 with no site: the caller names the statement.
    pub fn due(&self, now: Tick, after: f64) -> Result<Due, DigitalError> {
        if after.is_nan() {
            return Err(DigitalError::NanDelay { code: "E0435", instance: String::new(), statement: 0 });
        }
        if after == f64::INFINITY {
            return Ok(Due::Never);
        }
        if after <= 0.0 {
            return Ok(Due::NextMicrostep);
        }
        let Tick(advance) = self.tick_of(after)?;
        if advance == 0 {
            return Ok(Due::NextMicrostep);
        }
        match now.0.checked_add(advance) {
            Some(at) => Ok(Due::At(Tick(at))),
            None => Err(DigitalError::TickRange { code: "E0502", seconds: after }),
        }
    }
}
