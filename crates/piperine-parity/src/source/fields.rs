//! The fields of a record, read by name.

use piperine_api::{Field, Record};

/// The typed fields of one record a model emitted.
pub struct RecordFields;

impl RecordFields {
    /// The boolean field `name` of `record`.
    pub fn boolean(record: &Record, name: &str) -> bool {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Bool(value))) => *value,
            other => panic!("{name} is a bool: {other:?}"),
        }
    }
}
