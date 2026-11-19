//! `KeptRecords`: the sink a Book test hands to a run, so the records the run emits can be read
//! afterwards in the order the solver delivered them.

use piperine::{Field, Record, RecordSink};

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

/// The fields of a record by name.
pub struct Fields;

impl Fields {
    /// The value of the field `name` of `record`. Panics naming the field when there is none.
    pub fn of<'a>(record: &'a Record, name: &str) -> &'a Field {
        &record.fields.iter().find(|(field, _)| field == name).unwrap_or_else(|| panic!("no field `{name}` in {:?}", record.fields)).1
    }

    /// The real number of the field `name` of `record`. Panics when the field is not a real number.
    pub fn real(record: &Record, name: &str) -> f64 {
        match Self::of(record, name) {
            Field::Real(value) => *value,
            other => panic!("field `{name}`: a real number, found {other:?}"),
        }
    }
}
