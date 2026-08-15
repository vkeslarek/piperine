//! Digital run state: the net values, the event queue, and the
//! checkpoint/rollback snapshots the analog accept gate relies on.
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use crate::digital::{LogicValue, DigitalNet, DigitalEvent, DriverTable};
use crate::digital::delays::{DelayTable, TimingCheckTable};

/// Frozen scheduler snapshot for checkpoint/rollback.
#[derive(Clone)]
struct Checkpoint {
    nets: Vec<LogicValue>,
    queue: BinaryHeap<Reverse<DigitalEvent>>,
    labels: Vec<String>,
    driver_table: DriverTable,
}

// ---------------------------------------------------------------------------
// DigitalState
// ---------------------------------------------------------------------------

pub struct DigitalState {
    pub nets: Vec<LogicValue>,
    pub event_queue: BinaryHeap<Reverse<DigitalEvent>>,
    /// Stable source-level labels for each digital net, parallel to `nets`.
    /// Empty when the circuit builder does not attach labels — the public
    /// lookup then falls back to the anonymous `d{idx}` form.
    labels: Vec<String>,
    checkpoint: Option<Checkpoint>,
    /// LRM §7.9 per-net driver records (T34/LRMC-C7-05) — rolled back
    /// alongside `nets`/`event_queue` so a rejected transient step can't
    /// leave a stale driver contribution behind.
    pub driver_table: DriverTable,
    /// LRM §10 per-net delay/timing-check records (T60, LRMC-C10-12 model
    /// half — `vpi_get_delays`/`vpi_put_delays`'s native equivalent reads/
    /// writes through these, T61). Static per-net configuration, not
    /// rolled-back-per-timestep simulation state, so unlike `nets`/
    /// `event_queue`/`driver_table` these are not part of [`Checkpoint`].
    pub delay_table: DelayTable,
    pub timing_checks: TimingCheckTable,
}

impl DigitalState {
    pub fn new(num_nets: usize) -> Self {
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            labels: Vec::new(),
            checkpoint: None,
            driver_table: DriverTable::new(),
            delay_table: DelayTable::new(),
            timing_checks: TimingCheckTable::new(),
        }
    }

    /// Build with explicit labels (one per net, in dense order).
    pub fn with_labels(num_nets: usize, labels: Vec<String>) -> Self {
        assert_eq!(
            labels.len(),
            num_nets,
            "with_labels: label count must match net count"
        );
        Self {
            nets: vec![LogicValue::X; num_nets],
            event_queue: BinaryHeap::new(),
            labels,
            checkpoint: None,
            driver_table: DriverTable::new(),
            delay_table: DelayTable::new(),
            timing_checks: TimingCheckTable::new(),
        }
    }

    /// Attach a label to a single digital net. Existing nets beyond the
    /// supplied index keep their labels (or `d{i}` if none was set).
    pub fn set_label(&mut self, net: DigitalNet, label: impl Into<String>) {
        let idx = net.0;
        if self.labels.len() <= idx {
            self.labels
                .resize(idx + 1, format!("d{}", self.labels.len()));
        }
        self.labels[idx] = label.into();
    }

    /// Look up the stable label for a digital net. Returns the anonymous
    /// `d{idx}` form when no source-level label was attached.
    pub fn label_or_default(&self, net: DigitalNet) -> String {
        match self.labels.get(net.0) {
            Some(s) if !s.is_empty() => s.clone(),
            _ => format!("d{}", net.0),
        }
    }

    pub fn schedule(&mut self, event: DigitalEvent) {
        self.event_queue.push(Reverse(event));
    }

    pub fn peek_next_event_time(&self) -> f64 {
        self.event_queue.peek().map(|Reverse(e)| e.time).unwrap_or(f64::INFINITY)
    }

    pub fn checkpoint(&mut self) {
        self.checkpoint = Some(Checkpoint {
            nets: self.nets.clone(),
            queue: self.event_queue.clone(),
            labels: self.labels.clone(),
            driver_table: self.driver_table.clone(),
        });
    }

    pub fn rollback(&mut self) {
        if let Some(chk) = self.checkpoint.take() {
            self.nets = chk.nets;
            self.event_queue = chk.queue;
            self.labels = chk.labels;
            self.driver_table = chk.driver_table;
        }
    }

    pub fn commit(&mut self) {
        self.checkpoint = None;
    }
}
