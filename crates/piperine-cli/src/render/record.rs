//! `RecordLine`: an `emit!` record as one line (CLI-06 AC 2, 4 and 5).

use std::io::{self, Write};

use piperine_api::{Cell, Field, Record};

use crate::render::number::Number;

/// A record laid out for a terminal.
pub struct RecordLine;

impl RecordLine {
    /// `@<time> <instance> <stream> <field>=<value> ...`, one space between fields; a text in
    /// double quotes, an absent `Option` value as `None`.
    pub fn write(record: &Record, out: &mut dyn Write) -> io::Result<()> {
        write!(out, "@{} {} {}", Number::text(&Cell::Real(record.time)), record.instance, record.stream)?;
        for (name, field) in &record.fields {
            let value = match field {
                Field::Real(v) => Number::text(&Cell::Real(*v)),
                Field::Bool(v) => Number::text(&Cell::Bool(*v)),
                Field::Int(v) => Number::text(&Cell::Int(*v)),
                Field::Float(v) => Number::text(&Cell::Float(*v)),
                Field::Wide(v) => Number::text(&Cell::Wide(v.clone())),
                Field::Text(v) => format!("\"{v}\""),
                Field::Variant(v) => v.clone(),
                Field::Composite(v) => Number::text(&Cell::Composite(v.clone())),
            };
            write!(out, " {name}={value}")?;
        }
        writeln!(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_api::CompositeCell;

    /// Associated test helpers of `record`, grouped on one fixture type.
    struct RecordFixture;

    impl RecordFixture {
        fn line(record: &Record) -> String {
            let mut out = Vec::new();
            RecordLine::write(record, &mut out).expect("a vector takes writes");
            String::from_utf8(out).expect("text")
        }

        fn record(time: f64, instance: &str, stream: &str, fields: Vec<(&str, Field)>) -> Record {
            Record { time, instance: instance.into(), stream: stream.into(), fields: fields.into_iter().map(|(n, f)| (n.to_string(), f)).collect(), iteration: None, analysis: None }
        }
    }

    #[test]
    fn a_record_is_one_line() {
        // CLI-06 AC 2: `@<time> <instance path> <stream> <field>=<value> ...`; the lines are the
        // Book's (chapter 13): a real, a text in double quotes (AC 4), a bool, an integer, an
        // an absent `Option` as `None` (AC 5), and a record with no field
        let trip = RecordFixture::record(6.931472e-7, "Step.mon", "trip", vec![("v", Field::Real(0.5))]);
        assert_eq!(RecordFixture::line(&trip), "@6.931472e-7 Step.mon trip v=5.000000e-1\n");
        let check = RecordFixture::record(
            6.931472e-7,
            "Step.mon",
            "checks",
            vec![("kind", Field::Text("require".into())), ("name", Field::Text("crossed_upward".into())), ("pass", Field::Bool(true))],
        );
        assert_eq!(RecordFixture::line(&check), "@6.931472e-7 Step.mon checks kind=\"require\" name=\"crossed_upward\" pass=true\n");
        let absent = Field::Composite(Box::new(CompositeCell::Option(None)));
        let lost = RecordFixture::record(5e-7, "AdderTop.add", "lost", vec![("t", Field::Real(5e-7)), ("a", Field::Int(3)), ("b", absent)]);
        assert_eq!(RecordFixture::line(&lost), "@5.000000e-7 AdderTop.add lost t=5.000000e-7 a=3 b=None\n");
        assert_eq!(RecordFixture::line(&RecordFixture::record(0.0, "Top", "tick", vec![])), "@0.000000e0 Top tick\n");
    }
}
