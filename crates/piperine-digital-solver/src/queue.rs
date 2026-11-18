//! Pending commits (abi.md §6): ordered by due instant, ordered within an
//! instant (C2) and transported (C3).

use std::collections::BTreeMap;

use crate::journal::Journal;
use crate::net::NetId;
use crate::tick::Tick;

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

/// One change of the queue, kept to undo it (HYG-14).
#[derive(Debug)]
enum Undo {
    /// A commit was appended to the list due at this instant.
    Inserted(Instant),
    /// Commits left the list due at this instant, each from its position at the time.
    Removed(Instant, Vec<(usize, Pending)>),
    /// The whole list due at this instant was taken, in its stored order.
    Taken(Instant, Vec<Pending>),
}

/// Where a queue stood when it was marked: the length of its log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueMark {
    len: usize,
}

/// The commits not yet applied.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queue {
    by_due: BTreeMap<Instant, Vec<Pending>>,
    journal: Journal<Undo>,
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
            let journaling = self.journal.is_open();
            for (due, list) in self.by_due.iter_mut() {
                let (mut index, mut gone) = (0usize, Vec::new());
                list.retain(|old| {
                    let removed = old.same_statement(&pending)
                        && if replace { true } else { *due >= at && old.overlaps(&pending) };
                    if removed && journaling {
                        gone.push((index, old.clone()));
                    }
                    index += 1;
                    !removed
                });
                if !gone.is_empty() {
                    self.journal.push(Undo::Removed(*due, gone));
                }
            }
            self.by_due.retain(|_, list| !list.is_empty());
        }
        self.journal.push(Undo::Inserted(pending.due));
        self.by_due.entry(pending.due).or_default().push(pending);
    }

    /// Starts journaling: from here to the matching `release`, every push, removal and take is
    /// kept so `rewind` can undo it without a copy of the queue. Marks nest.
    pub fn mark(&mut self) -> QueueMark {
        QueueMark { len: self.journal.open() }
    }

    /// Undoes, newest first, what the queue did since `mark`. The mark stays open.
    pub fn rewind(&mut self, mark: &QueueMark) {
        while let Some(undo) = self.journal.pop_above(mark.len) {
            match undo {
                Undo::Inserted(due) => {
                    if let Some(list) = self.by_due.get_mut(&due) {
                        list.pop();
                        if list.is_empty() {
                            self.by_due.remove(&due);
                        }
                    }
                }
                Undo::Removed(due, items) => {
                    let list = self.by_due.entry(due).or_default();
                    for (index, pending) in items {
                        list.insert(index, pending);
                    }
                }
                Undo::Taken(due, list) => {
                    self.by_due.insert(due, list);
                }
            }
        }
    }

    /// Ends the newest open mark; the log empties with the outermost.
    pub fn release(&mut self, _mark: QueueMark) {
        self.journal.close();
    }

    /// The number of operations in the log (a diagnostic).
    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    /// A `REPLACE` commit that never comes due: removes every pending commit of its statement and
    /// inserts nothing (abi.md §6 C3). A host commit removes nothing.
    pub fn cancel(&mut self, pending: &Pending) {
        if pending.writer == Writer::Host {
            return;
        }
        let journaling = self.journal.is_open();
        for (due, list) in self.by_due.iter_mut() {
            let (mut index, mut gone) = (0usize, Vec::new());
            list.retain(|old| {
                let removed = old.same_statement(pending);
                if removed && journaling {
                    gone.push((index, old.clone()));
                }
                index += 1;
                !removed
            });
            if !gone.is_empty() {
                self.journal.push(Undo::Removed(*due, gone));
            }
        }
        self.by_due.retain(|_, list| !list.is_empty());
    }

    /// The earliest instant something is due at.
    pub fn next_due(&self) -> Option<Instant> {
        self.by_due.keys().next().copied()
    }

    /// Removes and returns the commits due at exactly `at`, in C2 order:
    /// (instant made, statement index, call order).
    pub fn take_due(&mut self, at: Instant) -> Vec<Pending> {
        let mut due = self.by_due.remove(&at).unwrap_or_default();
        if self.journal.is_open() && !due.is_empty() {
            self.journal.push(Undo::Taken(at, due.clone()));
        }
        due.sort_by_key(|p| (p.made, p.statement, p.call));
        due
    }

    /// The commits due at exactly `at`, in no particular order.
    pub fn due_at(&self, at: Instant) -> impl Iterator<Item = &Pending> {
        self.by_due.get(&at).into_iter().flatten()
    }

    /// The number of pending commits.
    pub fn len(&self) -> usize {
        self.by_due.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_due.is_empty()
    }
}
