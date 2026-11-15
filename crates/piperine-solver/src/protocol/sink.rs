//! The owner's one sink (abi.md C5, A5, §12 "Watch it"). Observations of the
//! discrete side wait in a buffer the checkpoint covers and reach the host's
//! `Sink` only after the acceptance that makes them final, in instant order;
//! an undone instant leaves nothing. With no sink nothing is kept.

use super::sides::{AnalysisKind, Observation};

/// Where the host receives observations.
pub trait Sink {
    fn observe(&mut self, o: &Observation);
}

/// The pending observations and the host's sink.
#[derive(Default)]
pub(super) struct Delivery {
    sink: Option<Box<dyn Sink>>,
    pending: Vec<Observation>,
    scratch: Vec<Observation>,
}

impl Delivery {
    pub(super) fn set_sink(&mut self, sink: Box<dyn Sink>) {
        self.sink = Some(sink);
    }

    /// The checkpoint of the buffer.
    pub(super) fn mark(&self) -> usize {
        self.pending.len()
    }

    /// Drops what was buffered after `mark` (an undone instant).
    pub(super) fn restore(&mut self, mark: usize) {
        self.pending.truncate(mark);
    }

    /// Takes what `drain` yields; buffers it only when a sink exists.
    pub(super) fn collect(&mut self, analysis: AnalysisKind, drain: impl FnOnce(&mut Vec<Observation>)) {
        drain(&mut self.scratch);
        if self.sink.is_none() {
            self.scratch.clear();
            return;
        }
        for mut o in self.scratch.drain(..) {
            o.analysis = analysis;
            self.pending.push(o);
        }
    }

    /// Delivers everything buffered: it is final.
    pub(super) fn flush(&mut self) {
        if let Some(sink) = self.sink.as_mut() {
            for o in &self.pending {
                sink.observe(o);
            }
        }
        self.pending.clear();
    }

    /// The number of observations waiting for an acceptance.
    pub(super) fn buffered(&self) -> usize {
        self.pending.len()
    }
}
