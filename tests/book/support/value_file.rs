//! `ValueFile`: one file of the value language, the scaffold chapters 2 to 4 of the Book share (a module
//! with no ports and a reaction at 1 us that writes one record), so their targets run a statement and
//! read the record it wrote the same way.

use piperine::{Field, Overrides, Record};

use super::faulty::Faulty;
use super::model::{Model, Ran};
use super::printed::Printed;
use super::records::Fields;

/// One file of the value language: the single record its reaction writes at 1 us.
pub struct ValueFile;

impl ValueFile {
    /// The overrides of `piperine run <file> --analysis transient --stop 2e-6`.
    pub fn transient() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(2e-6), ..Overrides::default() }
    }

    /// The one record of the stream `stream` of the run `result`, written at 1 us. Panics with the failure
    /// when the run failed.
    pub fn only(result: Result<Ran, piperine::Failure>, stream: &str) -> Record {
        let ran = result.unwrap_or_else(|failure| panic!("the file fails: {failure}"));
        let records = ran.stream(stream);
        assert_eq!(records.len(), 1, "the reaction on `at(1u)` fires once: {:?}", ran.records);
        Printed::holds("1.000000e-6", records[0].time, "the instant of the record");
        records[0].clone()
    }

    /// The one record of the stream `r` written by `statements` in a reaction at 1 us, after `items`.
    pub fn written(items: &str, statements: &str) -> Record {
        let source = format!("{items}\nmodule Top() {{}}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n");
        let ran = Model::written("rule.msl", &source).run(Self::transient()).unwrap_or_else(|failure| panic!("{source}\nfails: {failure}"));
        let records = ran.stream("r");
        assert_eq!(records.len(), 1, "{:?}", ran.records);
        records[0].clone()
    }

    /// The diagnostics of `statements` in a reaction at 1 us, after `items`, held under the row `row`.
    pub fn refused(row: &str, items: &str, statements: &str) -> Faulty {
        Faulty::elaborating(row, &format!("{items}\nmodule Top() {{}}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n"), "Top")
    }

    /// Asserts that the field `name` of `record` is the integer `value`.
    pub fn int(record: &Record, name: &str, value: i128) {
        assert_eq!(Fields::of(record, name), &Field::Int(value), "the field `{name}`");
    }

    /// Asserts that the field `name` of `record` is the bool `value`.
    pub fn flag(record: &Record, name: &str, value: bool) {
        assert_eq!(Fields::of(record, name), &Field::Bool(value), "the field `{name}`");
    }
}
