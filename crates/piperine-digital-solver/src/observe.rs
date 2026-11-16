//! What the engine reports (abi.md §6 C5): every applied change, every event
//! fired, every commit made and every record, in (instant, instance path,
//! statement) order. Observations name nets and instances by their text, never
//! by an index, so two builds of one design in different insertion orders
//! report the same sequence.

use crate::queue::Instant;

/// One thing the sink sees.
#[derive(Clone, Debug, PartialEq)]
pub enum Observation {
    /// A net's bytes changed (an applied commit or a function's write).
    Change { net: String, at: Instant, bytes: Vec<u8> },
    /// An event net fired.
    Fire { net: String, at: Instant },
    /// A reaction made a commit; `due` is `None` for one that never comes due.
    Commit { instance: String, reaction: u32, statement: u32, made: Instant, due: Option<Instant> },
    /// A commit to a `RECORD` net, stamped with the instant it was made, with the reaction and
    /// the statement that made it (abi.md C5): a record names its own site, whatever order the
    /// records and commits of one microstep are delivered in.
    Record { stream: String, reaction: u32, statement: u32, at: Instant, bytes: Vec<u8> },
    /// A relaxed field with a known byte still moved and was zeroed (abi.md §9 D1).
    Unsettled { net: String, at: Instant },
}

impl Observation {
    /// The order inside one microstep: by kind, then by name, then by statement.
    fn key(&self) -> (u8, &str, u32) {
        match self {
            Observation::Change { net, .. } => (0, net, 0),
            Observation::Fire { net, .. } => (1, net, 0),
            Observation::Unsettled { net, .. } => (2, net, 0),
            Observation::Commit { instance, statement, .. } => (3, instance, *statement),
            Observation::Record { stream, .. } => (4, stream, 0),
        }
    }
}

/// The buffer: what the microstep in progress has seen, and what earlier ones sent on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obs {
    pending: Vec<Observation>,
    done: Vec<Observation>,
    /// How many observations `drain` has handed out.
    drained: usize,
}

impl Obs {
    pub fn new() -> Obs {
        Obs::default()
    }

    /// Records one observation of the current microstep.
    pub fn push(&mut self, o: Observation) {
        self.pending.push(o);
    }

    /// Ends the microstep: its observations are sorted and join the buffer.
    pub fn flush(&mut self) {
        self.pending.sort_by(|a, b| a.key().cmp(&b.key()));
        self.done.append(&mut self.pending);
    }

    /// The observations of finished microsteps not yet drained.
    pub fn list(&self) -> &[Observation] {
        &self.done
    }

    /// A position to come back to with `truncate`.
    pub fn mark(&self) -> usize {
        self.drained + self.done.len()
    }

    /// Forgets what was observed after `mark` (and the unfinished microstep). What the
    /// host already drained stays drained.
    pub fn truncate(&mut self, mark: usize) {
        self.pending.clear();
        self.done.truncate(mark.saturating_sub(self.drained));
    }

    /// Hands out every finished observation.
    pub fn drain(&mut self) -> Vec<Observation> {
        self.drained += self.done.len();
        std::mem::take(&mut self.done)
    }
}
