use std::cmp::Ordering;
use crate::digital::logic::LogicValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DigitalNet(pub usize);

#[derive(Debug, Clone)]
pub struct DigitalEvent {
    pub time: f64,
    pub net: DigitalNet,
    pub value: LogicValue,
    pub source: usize,
    pub seq: u64, // Used to preserve FIFO ordering for events at the same time
}

impl PartialEq for DigitalEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && 
        self.seq == other.seq && 
        self.net == other.net && 
        self.value == other.value && 
        self.source == other.source
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
        // Compare by time first.
        let time_cmp = self.time.total_cmp(&other.time);
        if time_cmp != Ordering::Equal {
            return time_cmp;
        }
        // For same time, compare sequence number to preserve FIFO insertion order
        self.seq.cmp(&other.seq)
    }
}
