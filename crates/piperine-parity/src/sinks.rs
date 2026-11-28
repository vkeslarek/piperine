//! The record sinks of the host-library tests: one that keeps nothing and one that keeps every record, so a
//! test that reads a table does not declare a sink of its own.

use piperine_api::{Record, RecordSink};

/// A sink that keeps nothing: a run read through its table.
pub struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// A sink that keeps every record, in the order the run delivered them.
#[derive(Default)]
pub struct Kept(pub Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}
