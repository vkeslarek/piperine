//! Digital events and logic values: `LogicValue`, the `DigitalNet`
//! handle, and the time-ordered `DigitalEvent` the scheduler queues.

use std::cmp::Ordering;

use crate::drivers::Strength;

// ---------------------------------------------------------------------------
// LogicValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

impl LogicValue {
    /// Resolves two driving logic values onto the same net.
    pub fn resolve(a: LogicValue, b: LogicValue) -> LogicValue {
        match (a, b) {
            (LogicValue::Z, other) | (other, LogicValue::Z) => other,
            (LogicValue::Zero, LogicValue::Zero) => LogicValue::Zero,
            (LogicValue::One, LogicValue::One) => LogicValue::One,
            _ => LogicValue::X,
        }
    }
}

// ---------------------------------------------------------------------------
// DigitalNet / DigitalEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNet(pub usize);

#[derive(Debug, Clone)]
pub struct DigitalEvent {
    pub time: f64,
    pub net: DigitalNet,
    pub value: LogicValue,
    pub source: usize,
    pub seq: u64,
    /// LRM §7.9 Table 7-2 drive strength (LRMW3-09, A.2.2.2) explicitly
    /// authored by the emitting model (e.g. a continuous assign carrying
    /// `assign (strong1, weak0) w = r;`) — `None` for every event source
    /// that predates this field (the overwhelming common case: an
    /// ordinary write with no authored strength), which
    /// [`super::scheduler::apply_driven_event`] falls back to
    /// [`super::drivers::default_strength_for`] for, exactly as before
    /// this field existed. Additive per this struct's own ABI-stability
    /// contract (`interface.rs`'s module doc) — a model compiled against
    /// an older version never needs to know this field exists.
    pub strength: Option<Strength>,
}

impl PartialEq for DigitalEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time &&
        self.seq == other.seq &&
        self.net == other.net &&
        self.value == other.value &&
        self.source == other.source &&
        self.strength == other.strength
    }
}

impl Eq for DigitalEvent {}

impl PartialOrd for DigitalEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DigitalEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_cmp = self.time.total_cmp(&other.time);
        if time_cmp != Ordering::Equal {
            return time_cmp;
        }
        self.seq.cmp(&other.seq)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logic_resolution() {
        assert_eq!(LogicValue::resolve(LogicValue::Zero, LogicValue::One), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::Z, LogicValue::One), LogicValue::One);
        assert_eq!(LogicValue::resolve(LogicValue::X, LogicValue::Zero), LogicValue::X);
        assert_eq!(LogicValue::resolve(LogicValue::One, LogicValue::One), LogicValue::One);
    }
}
