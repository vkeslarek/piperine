//! `TextTable`: a result table as aligned text (CLI-07 AC 2).

use std::io::{self, Write};

use piperine_api::{Cell, Table};

use crate::render::number::Number;

/// A table laid out for a terminal.
pub struct TextTable;

impl TextTable {
    /// One header row and one row per data row; cells left-aligned, each column padded to its
    /// widest cell, columns separated by two spaces, no trailing space on a line.
    pub fn write(table: &Table, out: &mut dyn Write) -> io::Result<()> {
        let mut lines: Vec<Vec<String>> = vec![table.columns.clone()];
        lines.extend(table.rows.iter().map(|row| row.iter().map(Number::text).collect()));
        let widths: Vec<usize> = (0..table.columns.len()).map(|c| lines.iter().map(|l| l.get(c).map_or(0, |cell| cell.chars().count())).max().unwrap_or(0)).collect();
        for line in &lines {
            let padded: Vec<String> = line.iter().zip(&widths).map(|(cell, width)| format!("{cell:<width$}")).collect();
            writeln!(out, "{}", padded.join("  ").trim_end())?;
        }
        Ok(())
    }

    /// The summary lines of a run after its rows, one per line as `name  value`, the value
    /// written as a table cell is.
    pub fn write_summary(lines: &[(String, f64)], out: &mut dyn Write) -> io::Result<()> {
        for (name, value) in lines {
            writeln!(out, "{name}  {}", Number::text(&Cell::Real(*value)))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_api::{Cell, CompositeCell};

    /// A table as the lines a terminal shows.
    struct LaidOutTable;

    impl LaidOutTable {
        fn laid_out(table: &Table) -> String {
            let mut out = Vec::new();
            TextTable::write(table, &mut out).expect("a vector takes writes");
            String::from_utf8(out).expect("text")
        }
    }

    #[test]
    fn a_table_is_left_aligned_two_spaces_no_trailing_space() {
        // CLI-07 AC 2: the operating point of the Book's RC (chapter 13), column by column
        let op = Table {
            columns: vec!["signal".into(), "value".into()],
            rows: vec![
                vec![Cell::Text("Step.vin".into()), Cell::Real(1.0)],
                vec![Cell::Text("Step.vout".into()), Cell::Real(1.0)],
            ],
        };
        assert_eq!(LaidOutTable::laid_out(&op), "signal     value\nStep.vin   1.000000e0\nStep.vout  1.000000e0\n");
        // a column wider in its header than in its cells, an absent `Option`, and a last column whose
        // cells are shorter than its header: no line ends in a space
        let tran = Table {
            columns: vec!["time".into(), "Step.vout".into(), "ok".into()],
            rows: vec![
                vec![Cell::Real(0.0), Cell::Composite(Box::new(CompositeCell::Option(None))), Cell::Bool(true)],
                vec![Cell::Real(1e-6), Cell::Real(0.5), Cell::Int(7)],
            ],
        };
        let text = LaidOutTable::laid_out(&tran);
        let want = [
            "time         Step.vout    ok",
            "0.000000e0   None         true",
            "1.000000e-6  5.000000e-1  7",
            "",
        ];
        assert_eq!(text, want.join("\n"));
        assert!(text.lines().all(|l| l == l.trim_end()), "{text:?}");
        // a table with no row is its header
        let empty = Table { columns: vec!["signal".into(), "value".into()], rows: vec![] };
        assert_eq!(LaidOutTable::laid_out(&empty), "signal  value\n");
    }
}
