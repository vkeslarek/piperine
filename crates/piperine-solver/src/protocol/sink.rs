//! The owner's one sink (abi.md C5, A5, §12 "Watch it"). Observations of the
//! discrete side wait in a buffer the checkpoint covers and reach the host's
//! `Sink` only after the acceptance that makes them final, in instant order;
//! an undone instant leaves nothing. Trace records of the analog side are kept
//! after every attempt, rejected or not (abi.md A5), and delivered once an
//! acceptance reaches their time, so the host reads one stream in time order;
//! at one time the solve's traces come before its instant. With no sink
//! nothing is kept.

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
    /// Trace records not yet reached by an acceptance; a restore keeps them.
    traces: Vec<Observation>,
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

    /// Keeps the trace records `drain` yields, whatever happens to the step
    /// (abi.md A5: traces of a rejected attempt are delivered too).
    pub(super) fn keep_traces(&mut self, analysis: AnalysisKind, drain: impl FnOnce(&mut Vec<Observation>)) {
        drain(&mut self.scratch);
        if self.sink.is_none() {
            self.scratch.clear();
            return;
        }
        for mut o in self.scratch.drain(..) {
            o.analysis = analysis;
            self.traces.push(o);
        }
    }

    /// Delivers everything buffered, final at the accepted time `upto`, with
    /// the traces up to that time, in time order (traces first at one time).
    pub(super) fn flush(&mut self, upto: f64) {
        let (due, later): (Vec<Observation>, Vec<Observation>) =
            std::mem::take(&mut self.traces).into_iter().partition(|o| o.time <= upto);
        self.traces = later;
        let mut stream: Vec<(u8, Observation)> = due.into_iter().map(|o| (0, o)).collect();
        stream.extend(self.pending.drain(..).map(|o| (1, o)));
        stream.sort_by(|a, b| a.1.time.total_cmp(&b.1.time).then(a.0.cmp(&b.0)));
        if let Some(sink) = self.sink.as_mut() {
            for (_, o) in &stream {
                sink.observe(o);
            }
        }
    }

    /// Delivers everything left at the end of an analysis.
    pub(super) fn flush_all(&mut self) {
        self.flush(f64::INFINITY);
    }

    /// The number of observations waiting for an acceptance.
    pub(super) fn buffered(&self) -> usize {
        self.pending.len() + self.traces.len()
    }
}
