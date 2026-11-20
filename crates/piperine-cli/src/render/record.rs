//! `RecordLine`: an `emit!` record as one line (CLI-06 AC 2, 4 and 5).

use std::io::{self, Write};

use piperine_api::{Cell, Field, Record};

use crate::render::number::Number;

/// A record laid out for a terminal.
pub struct RecordLine;

impl RecordLine {
    /// `@<time> <instance> <stream> <field>=<value> ...`, one space between fields; a text in
    /// double quotes, an unknown value as `x`. A record reached under unknown control ends with
    /// `maybe=true` (§19 rule 1); one that surely ran has no `maybe` at all, so no listing of a
    /// design without unknown control moves.
    pub fn write(record: &Record, out: &mut dyn Write) -> io::Result<()> {
        write!(out, "@{} {} {}", Number::text(&Cell::Real(record.time)), record.instance, record.stream)?;
        for (name, field) in &record.fields {
            let value = match field {
                Field::Real(v) => Number::text(&Cell::Real(*v)),
                Field::Bool(v) => Number::text(&Cell::Bool(*v)),
                Field::Int(v) => Number::text(&Cell::Int(*v)),
                Field::Text(v) => format!("\"{v}\""),
                Field::Unknown => Number::text(&Cell::Unknown),
            };
            write!(out, " {name}={value}")?;
        }
        if record.maybe {
            write!(out, " maybe=true")?;
        }
        writeln!(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(record: &Record) -> String {
        let mut out = Vec::new();
        RecordLine::write(record, &mut out).expect("a vector takes writes");
        String::from_utf8(out).expect("text")
    }

    fn record(time: f64, instance: &str, stream: &str, fields: Vec<(&str, Field)>) -> Record {
        Record { time, instance: instance.into(), stream: stream.into(), fields: fields.into_iter().map(|(n, f)| (n.to_string(), f)).collect(), maybe: false }
    }

    #[test]
    fn a_record_is_one_line() {
        // CLI-06 AC 2: `@<time> <instance path> <stream> <field>=<value> ...`; the lines are the
        // Book's (chapter 13): a real, a text in double quotes (AC 4), a bool, an integer, an
        // unknown as `x` (AC 5), and a record with no field
        let trip = record(6.931472e-7, "Step.mon", "trip", vec![("v", Field::Real(0.5))]);
        assert_eq!(line(&trip), "@6.931472e-7 Step.mon trip v=5.000000e-1\n");
        let check = record(
            6.931472e-7,
            "Step.mon",
            "checks",
            vec![("kind", Field::Text("require".into())), ("name", Field::Text("crossed_upward".into())), ("pass", Field::Bool(true))],
        );
        assert_eq!(line(&check), "@6.931472e-7 Step.mon checks kind=\"require\" name=\"crossed_upward\" pass=true\n");
        let lost = record(5e-7, "AdderTop.add", "lost", vec![("t", Field::Real(5e-7)), ("a", Field::Int(3)), ("b", Field::Unknown)]);
        assert_eq!(line(&lost), "@5.000000e-7 AdderTop.add lost t=5.000000e-7 a=3 b=x\n");
        assert_eq!(line(&record(0.0, "Top", "tick", vec![])), "@0.000000e0 Top tick\n");
    }

    #[test]
    fn a_maybe_record_ends_with_maybe_true_and_a_sure_one_prints_no_maybe() {
        // US-05 AC 1, 2: `maybe=true` is the last field of the line when the record was reached
        // under unknown control, and nothing is printed for a record that surely ran
        let reached = Record { maybe: true, ..record(5e-7, "Top.m", "lost", vec![("a", Field::Int(3)), ("b", Field::Unknown)]) };
        assert_eq!(line(&reached), "@5.000000e-7 Top.m lost a=3 b=x maybe=true\n");
        let sure = record(5e-7, "Top.m", "lost", vec![("a", Field::Int(3)), ("b", Field::Unknown)]);
        assert_eq!(line(&sure), "@5.000000e-7 Top.m lost a=3 b=x\n");
    }
}
