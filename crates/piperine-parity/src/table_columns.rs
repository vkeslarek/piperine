//! A column of a result table found by its name.

use piperine_api::{Cell, Table};

/// Reads of a [`Table`] by column name.
pub struct TableColumns;

impl TableColumns {
    /// The position of the column `column` of `table`. Panics naming the column when the table has none.
    pub fn position(table: &Table, column: &str) -> usize {
        table.columns.iter().position(|name| name == column).unwrap_or_else(|| panic!("no column `{column}` in {:?}", table.columns))
    }

    /// The cell of `table` at `row` under `column`.
    pub fn cell<'t>(table: &'t Table, row: usize, column: &str) -> &'t Cell {
        &table.rows[row][Self::position(table, column)]
    }
}
