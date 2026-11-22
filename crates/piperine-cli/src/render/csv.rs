//! `CsvTable`: a result table as CSV (CLI-11 AC 1).

use std::io::{self, Write};

use piperine_api::Table;

use crate::render::number::Number;

/// A table laid out for a file a spreadsheet or a script reads.
pub struct CsvTable;

impl CsvTable {
    /// One header row with the column names, then one row per data row; a column of composite values
    /// is one column per dotted leaf (`Top.pair.a`, `Top.v[1]`; an enum a variant column and its payload
    /// leaves `Top.tx.Data.n`); cells separated by `,`,
    /// numbers as the text table writes them. A cell holding a comma, a quote or a line break is
    /// quoted, its quotes doubled (RFC 4180).
    pub fn write(table: &Table, out: &mut dyn Write) -> io::Result<()> {
        // a column of composite values is one column per dotted leaf (D13)
        let table = &table.expanded(true);
        let header: Vec<String> = table.columns.iter().map(|c| CsvTable::cell(c)).collect();
        writeln!(out, "{}", header.join(","))?;
        for row in &table.rows {
            let cells: Vec<String> = row.iter().map(|cell| CsvTable::cell(&Number::text(cell))).collect();
            writeln!(out, "{}", cells.join(","))?;
        }
        Ok(())
    }

    fn cell(text: &str) -> String {
        if text.contains([',', '"', '\n', '\r']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_api::Cell;

    #[test]
    fn a_cell_with_a_comma_or_a_quote_is_quoted() {
        // RFC 4180: the cell is wrapped in quotes and its quotes are doubled
        let table = Table { columns: vec!["a,b".into(), "say \"x\"".into()], rows: vec![vec![Cell::Real(1.0), Cell::Bool(true)]] };
        let mut out = Vec::new();
        CsvTable::write(&table, &mut out).expect("a vector takes writes");
        assert_eq!(String::from_utf8(out).expect("text"), "\"a,b\",\"say \"\"x\"\"\"\n1.000000e0,true\n");
    }
}
