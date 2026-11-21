//! The printed columns of a run as `(time, value)` rows, for the criteria that judge every row.

use piperine::{Cell, Table};
use piperine_parity::scenario::NamedColumn;

/// One column of a run's printed table with the time of each row.
pub struct PrintedColumn;

impl PrintedColumn {
    /// The rows of the real column `name` of `table`: row time and value; panics on a cell that is not real.
    pub fn reals(table: &Table, name: &str) -> Vec<(f64, f64)> {
        let column = NamedColumn::find(table, name);
        table
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| match (&cells[0], &cells[column.index()]) {
                (Cell::Real(time), Cell::Real(value)) => (*time, *value),
                other => panic!("row {row} of `{}` is not a real number at a real time: {other:?}", column.label()),
            })
            .collect()
    }
}
