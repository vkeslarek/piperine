//! `JsonRecord`: an `emit!` record as one JSON object, one line of a JSON Lines file (CLI-12
//! AC 3 and 5). Written by hand: five value kinds and one string escape need no JSON crate.

use std::io::{self, Write};

use piperine_api::{Field, Record};

/// A record laid out for a file a script reads.
pub struct JsonRecord;

impl JsonRecord {
    /// `{"time":..,"instance":"..","stream":"..","fields":{..}}` and a line break. A real is a
    /// JSON number in Rust's shortest form that reads back as the same `f64` (a real JSON cannot
    /// hold, NaN or an infinity, is the string `"NaN"`, `"inf"` or `"-inf"`), a `bool` a JSON
    /// boolean, an integer a JSON number, a text a JSON string and an unknown value `null`.
    pub fn write(record: &Record, out: &mut dyn Write) -> io::Result<()> {
        write!(out, "{{\"time\":{},\"instance\":{},\"stream\":{},\"fields\":{{", JsonRecord::real(record.time), JsonRecord::string(&record.instance), JsonRecord::string(&record.stream))?;
        for (i, (name, field)) in record.fields.iter().enumerate() {
            if i > 0 {
                write!(out, ",")?;
            }
            let value = match field {
                Field::Real(v) => JsonRecord::real(*v),
                Field::Bool(v) => v.to_string(),
                Field::Int(v) => v.to_string(),
                Field::Text(v) => JsonRecord::string(v),
                Field::Unknown => "null".to_string(),
            };
            write!(out, "{}:{value}", JsonRecord::string(name))?;
        }
        writeln!(out, "}}}}")
    }

    fn real(value: f64) -> String {
        if value.is_nan() {
            "\"NaN\"".into()
        } else if value.is_infinite() {
            if value > 0.0 { "\"inf\"".into() } else { "\"-inf\"".into() }
        } else {
            format!("{value:?}")
        }
    }

    /// A JSON string: `"`, `\` and the control characters escaped.
    fn string(text: &str) -> String {
        let mut quoted = String::with_capacity(text.len() + 2);
        quoted.push('"');
        for c in text.chars() {
            match c {
                '"' => quoted.push_str("\\\""),
                '\\' => quoted.push_str("\\\\"),
                '\n' => quoted.push_str("\\n"),
                '\r' => quoted.push_str("\\r"),
                '\t' => quoted.push_str("\\t"),
                c if (c as u32) < 0x20 => quoted.push_str(&format!("\\u{:04x}", c as u32)),
                c => quoted.push(c),
            }
        }
        quoted.push('"');
        quoted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(record: &Record) -> String {
        let mut out = Vec::new();
        JsonRecord::write(record, &mut out).expect("a vector takes writes");
        String::from_utf8(out).expect("text")
    }

    #[test]
    fn each_value_kind_has_its_json() {
        // CLI-12 AC 5: a real is a JSON number, a bool a boolean, an integer a number, a text a
        // string and an unknown `null`; AC 3: the keys are time, instance, stream and fields
        let record = Record {
            time: 1e-6,
            instance: "Step.mon".into(),
            stream: "trip".into(),
            fields: vec![
                ("v".into(), Field::Real(0.5)),
                ("ok".into(), Field::Bool(true)),
                ("n".into(), Field::Int(-7)),
                ("big".into(), Field::Int(u64::MAX as i128)),
                ("kind".into(), Field::Text("require".into())),
                ("lost".into(), Field::Unknown),
            ],
        };
        assert_eq!(
            line(&record),
            "{\"time\":1e-6,\"instance\":\"Step.mon\",\"stream\":\"trip\",\"fields\":{\"v\":0.5,\"ok\":true,\"n\":-7,\"big\":18446744073709551615,\"kind\":\"require\",\"lost\":null}}\n"
        );
        // a record with no field has an empty object
        let bare = Record { time: 0.0, instance: "T".into(), stream: "s".into(), fields: vec![] };
        assert_eq!(line(&bare), "{\"time\":0.0,\"instance\":\"T\",\"stream\":\"s\",\"fields\":{}}\n");
        // string escaping: a quote, a backslash, a line break and a control character
        let text = Record { time: 0.0, instance: "T".into(), stream: "s".into(), fields: vec![("m".into(), Field::Text("a\"b\\c\nd\u{1}".into()))] };
        assert_eq!(line(&text), "{\"time\":0.0,\"instance\":\"T\",\"stream\":\"s\",\"fields\":{\"m\":\"a\\\"b\\\\c\\nd\\u0001\"}}\n");
        // a real reads back as the same f64: the shortest form is exact
        for v in [0.1, 6.931075e-7, 1.0 / 3.0, 1e300, -2.5e-300, f64::MIN_POSITIVE] {
            let record = Record { time: v, instance: "T".into(), stream: "s".into(), fields: vec![] };
            let text = line(&record);
            let number = text.strip_prefix("{\"time\":").and_then(|t| t.split(',').next()).expect("a time");
            assert_eq!(number.parse::<f64>().expect("a number"), v, "{text}");
        }
        // a value JSON has no number for is a string, never a bare `NaN`
        let nan = Record { time: 0.0, instance: "T".into(), stream: "s".into(), fields: vec![("x".into(), Field::Real(f64::NAN)), ("y".into(), Field::Real(f64::NEG_INFINITY))] };
        assert!(line(&nan).contains("\"x\":\"NaN\",\"y\":\"-inf\""), "{}", line(&nan));
    }
}
