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

    /// The rows of the discrete column `name` of `table`: row time and value, an unknown cell as `None`; a
    /// `Bool` reads as 0 or 1.
    pub fn integers(table: &Table, name: &str) -> Vec<(f64, Option<i128>)> {
        let column = NamedColumn::find(table, name);
        table
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                let Cell::Real(time) = cells[0] else { panic!("row {row} of `{}` has no real time: {:?}", column.label(), cells[0]) };
                let value = match &cells[column.index()] {
                    Cell::Int(value) => Some(*value),
                    Cell::Bool(flag) => Some(i128::from(*flag)),
                    Cell::Unknown => None,
                    other => panic!("row {row} of `{}` is not a discrete value: {other:?}", column.label()),
                };
                (time, value)
            })
            .collect()
    }
}
