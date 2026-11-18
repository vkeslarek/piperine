//! The undo log behind `mark`/`rewind` (HYG-14): what a run changed since a mark, kept in the
//! order it changed, so a rewind undoes it in reverse without copying the state at the mark.
//!
//! A log records only while a mark is open; marks nest, and the log empties when the outermost
//! one is released. A clone of a log is an empty, closed one: a checkpoint is a copy of the
//! state, never of the work in progress, and two states compare equal whatever their logs hold.

/// The entries a run made since its open marks.
#[derive(Debug)]
pub(crate) struct Journal<T> {
    depth: u32,
    entries: Vec<T>,
}

impl<T> Default for Journal<T> {
    fn default() -> Journal<T> {
        Journal { depth: 0, entries: Vec::new() }
    }
}

impl<T> Clone for Journal<T> {
    fn clone(&self) -> Journal<T> {
        Journal::default()
    }
}

impl<T> PartialEq for Journal<T> {
    fn eq(&self, _: &Journal<T>) -> bool {
        true
    }
}

impl<T> Journal<T> {
    /// Opens a mark: the position of the next entry.
    pub(crate) fn open(&mut self) -> usize {
        self.depth += 1;
        self.entries.len()
    }

    /// Releases the newest open mark; the log empties with the last one.
    pub(crate) fn close(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth == 0 {
            self.entries.clear();
        }
    }

    /// Whether a mark is open, so a change is recorded.
    pub(crate) fn is_open(&self) -> bool {
        self.depth > 0
    }

    pub(crate) fn push(&mut self, entry: T) {
        if self.depth > 0 {
            self.entries.push(entry);
        }
    }

    /// The newest entry made after position `len`, removed: undo them in this order.
    pub(crate) fn pop_above(&mut self, len: usize) -> Option<T> {
        if self.entries.len() > len { self.entries.pop() } else { None }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
