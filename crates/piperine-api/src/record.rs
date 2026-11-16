//! `Record`: an `emit!` record as the host receives it, and `RecordSink`, where a session
//! hands each one over as the solver delivers it (CLI-06).

/// One `emit!` record: when, which instance's site, which stream, and its fields in source
/// order.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub time: f64,
    pub instance: String,
    pub stream: String,
    pub fields: Vec<(String, Field)>,
}

/// The value of one field of a record.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Real(f64),
    Bool(bool),
    Int(i128),
    /// A text literal of the site.
    Text(String),
    /// A value whose known byte is clear.
    Unknown,
}

/// Where a session hands the records of a run, as the solver delivers them.
pub trait RecordSink {
    fn record(&mut self, record: &Record);
}
