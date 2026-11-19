//! `KeptRecords`: the sink a Book test hands to a run, so the records the run emits can be read
//! afterwards in the order the solver delivered them.

use piperine::{Record, RecordSink};

/// A `RecordSink` that keeps every record it is handed.
#[derive(Default)]
pub struct KeptRecords {
    records: Vec<Record>,
}

impl KeptRecords {
    /// The records kept so far, in delivery order.
    pub fn records(&self) -> &[Record] {
        &self.records
    }
}

impl RecordSink for KeptRecords {
    /// Keeps a copy of `record`.
    fn record(&mut self, record: &Record) {
        self.records.push(record.clone());
    }
}
