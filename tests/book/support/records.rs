//! `KeptRecords`: the sink a Book test hands to a run, so the records the run emits can be read
//! afterwards in the order the solver delivered them.

use piperine::{Cell, CompositeCell, Field, Record, RecordSink};

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

/// The records of a stream as one text each, for a table of outcomes a test compares whole.
pub struct Rows;

impl Rows {
    /// The text of a field: a variant by its name, a flag as `true` or `false`, a number as itself.
    fn text(field: &Field) -> String {
        match field {
            Field::Variant(name) | Field::Text(name) => name.clone(),
            Field::Bool(flag) => flag.to_string(),
            Field::Int(number) => number.to_string(),
            Field::Real(number) => format!("{number:e}"),
            other => format!("{other:?}"),
        }
    }

    /// One row per record of `records`: its instant in nanoseconds, then the fields `fields` in that order, separated by
    /// spaces (`1000 0 Strong`).
    pub fn of(records: &[&Record], fields: &[&str]) -> Vec<String> {
        records
            .iter()
            .map(|record| {
                let values: Vec<String> = fields.iter().map(|name| Self::text(Fields::of(record, name))).collect();
                format!("{} {}", (record.time * 1e9).round(), values.join(" "))
            })
            .collect()
    }
}

/// A printed column of a `Driven` net (a table cell that is the struct `Driven { v, s }`) as one text each.
pub struct DrivenCells;

impl DrivenCells {
    /// The level and the strength of a `Driven` cell, `1 Pull`: the text of `v`, a space, the name of `s`'s variant. Panics
    /// naming the cell when it is no `Driven`.
    pub fn text(cell: &Cell) -> String {
        let Cell::Composite(composite) = cell else { panic!("a `Driven` cell, found {cell:?}") };
        let CompositeCell::Struct { name, fields, .. } = composite.as_ref() else { panic!("a `Driven` cell, found {cell:?}") };
        match (name.as_str(), fields.as_slice()) {
            ("Driven", [(_, CompositeCell::Scalar(Cell::Text(level))), (_, CompositeCell::Variant { variant, .. })]) => format!("{level} {variant}"),
            _ => panic!("a `Driven` cell, found {cell:?}"),
        }
    }

    /// The cells of the column `column` of `table`, one text each, in row order.
    pub fn column(table: &piperine::Table, column: &str) -> Vec<String> {
        let at = table.columns.iter().position(|name| name == column).unwrap_or_else(|| panic!("no column `{column}` in {:?}", table.columns));
        table.rows.iter().map(|row| Self::text(&row[at])).collect()
    }
}
