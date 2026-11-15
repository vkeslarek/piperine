//! Pending commits (abi.md §6): ordered by due instant, ordered within an
//! instant (C2) and transported (C3).

use std::collections::BTreeMap;

use super::net::NetId;
use super::tick::Tick;

/// An instant: a tick and a microstep inside it.
pub type Instant = (Tick, u32);

/// Who made a commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
    Host,
    Reaction { instance: u32, reaction: u32 },
}

/// One commit waiting for its instant.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub net: NetId,
    pub offset: u32,
    pub bytes: Vec<u8>,
    pub writer: Writer,
    /// The statement's index in the calling reaction's `writes`.
    pub statement: u32,
    pub made: Instant,
    /// The order of the call within the microstep that made it.
    pub call: u32,
    pub due: Instant,
}

impl Pending {
    fn same_statement(&self, other: &Pending) -> bool {
        self.writer == other.writer && self.statement == other.statement && self.net == other.net
    }

    /// Whether the two commits touch a common byte. Two commits of no byte (an
    /// event pin's statement, `len = 0`) touch the same thing.
    fn overlaps(&self, other: &Pending) -> bool {
        let (a, b) = (self.offset as usize, other.offset as usize);
        if self.bytes.is_empty() && other.bytes.is_empty() {
            return true;
        }
        a < b + other.bytes.len() && b < a + self.bytes.len()
    }
}

/// The commits not yet applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queue {
    by_due: BTreeMap<Instant, Vec<Pending>>,
}

impl Queue {
    pub fn new() -> Queue {
        Queue::default()
    }

    /// C3: removes the pending commits of the same statement and byte range
    /// due at or after this one (every one of the statement when `replace`),
    /// then inserts it. A host commit is never transported.
    pub fn push(&mut self, pending: Pending, replace: bool) {
        if pending.writer != Writer::Host {
            let at = pending.due;
            for (due, list) in self.by_due.iter_mut() {
                list.retain(|old| {
                    let removed = old.same_statement(&pending)
                        && if replace { true } else { *due >= at && old.overlaps(&pending) };
                    !removed
                });
            }
            self.by_due.retain(|_, list| !list.is_empty());
        }
        self.by_due.entry(pending.due).or_default().push(pending);
    }

    /// The earliest instant something is due at.
    pub fn next_due(&self) -> Option<Instant> {
        self.by_due.keys().next().copied()
    }

    /// Removes and returns the commits due at exactly `at`, in C2 order:
    /// (instant made, statement index, call order).
    pub fn take_due(&mut self, at: Instant) -> Vec<Pending> {
        let mut due = self.by_due.remove(&at).unwrap_or_default();
        due.sort_by_key(|p| (p.made, p.statement, p.call));
        due
    }

    /// The number of pending commits.
    pub fn len(&self) -> usize {
        self.by_due.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_due.is_empty()
    }
}
