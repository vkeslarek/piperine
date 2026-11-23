//! [`DiscreteTrace`]: one discrete column of a run (a mode, a fault code, a bit) over a window.

use piperine_api::{Cell, Table};

use super::column::NamedColumn;

/// One row of a discrete trace: its time and what it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscreteSample {
    /// The time of the row, in seconds.
    pub time: f64,
    /// The value the row holds.
    pub value: i128,
}

/// The first row that does not hold what a trace was asked to hold: the error of
/// [`DiscreteTrace::holds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Departure {
    /// The time of the departing row, in seconds.
    pub time: f64,
    /// The value the row holds.
    pub value: i128,
}

/// The rows of one discrete column with the time of each, optionally cut to a window. A `Bool`
/// cell reads as 0 or 1, so a mode, a fault code and a flag are all read the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteTrace {
    /// The full label of the column, for the messages of a failure.
    label: String,
    /// The window the rows were cut to; none for the whole run.
    window: Option<(f64, f64)>,
    /// The rows, in time order.
    rows: Vec<DiscreteSample>,
}

impl DiscreteTrace {
    /// Every row of the column `name` of `table` with its time (column 0). A real or text cell
    /// panics naming the row: a discrete column never holds one.
    pub fn of(table: &Table, name: &str) -> DiscreteTrace {
        let column = NamedColumn::find(table, name);
        let rows = table
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                let Cell::Real(time) = cells[0] else { panic!("row {row} of `{}` has no real time: {:?}", column.label(), cells[0]) };
                let value = match &cells[column.index()] {
                    Cell::Int(value) => *value,
                    Cell::Bool(flag) => i128::from(*flag),
                    other => panic!("row {row} of `{}` is not a discrete value: {other:?}", column.label()),
                };
                DiscreteSample { time, value }
            })
            .collect();
        DiscreteTrace { label: column.label().to_string(), window: None, rows }
    }

    /// [`of`](Self::of) for a column of an enum with no payload, which a table prints by the variant's
    /// name (composite-values CV-22): each row reads as the declaration index of its variant among
    /// `variants`, in declaration order. A name `variants` lacks panics naming the row.
    pub fn of_enum(table: &Table, name: &str, variants: &[&str]) -> DiscreteTrace {
        let column = NamedColumn::find(table, name);
        let rows = table
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                let Cell::Real(time) = cells[0] else { panic!("row {row} of `{}` has no real time: {:?}", column.label(), cells[0]) };
                let value = match &cells[column.index()] {
                    Cell::Text(variant) => variants.iter().position(|candidate| candidate == variant).unwrap_or_else(|| panic!("row {row} of `{}` names `{variant}`, no variant of {variants:?}", column.label())) as i128,
                    other => panic!("row {row} of `{}` is not a variant's name: {other:?}", column.label()),
                };
                DiscreteSample { time, value }
            })
            .collect();
        DiscreteTrace { label: column.label().to_string(), window: None, rows }
    }

    /// The rows with `from <= time < to`; the window is kept for the messages of a failure.
    pub fn between(&self, from: f64, to: f64) -> DiscreteTrace {
        let rows = self.rows.iter().copied().filter(|row| row.time >= from && row.time < to).collect();
        DiscreteTrace { label: self.label.clone(), window: Some((from, to)), rows }
    }

    /// The time of the first row at or after `after` that holds `value`; none when no row does.
    pub fn first_time(&self, value: i128, after: f64) -> Option<f64> {
        self.rows.iter().find(|row| row.time >= after && row.value == value).map(|row| row.time)
    }

    /// Passes when every row holds `value`; otherwise the first row that does not, with its time
    /// and what it holds.
    pub fn holds(&self, value: i128) -> Result<(), Departure> {
        self.rows().iter().find(|row| row.value != value).map_or(Ok(()), |row| Err(Departure { time: row.time, value: row.value }))
    }

    /// What the row at or before `time` holds (the last row not after `time`). A time before the
    /// first row panics: a scenario reads an instant the run covers.
    pub fn value_at(&self, time: f64) -> i128 {
        match self.rows.iter().rev().find(|row| row.time <= time) {
            Some(row) => row.value,
            None => panic!("`{}` has no row at or before {time:e} s", self.label),
        }
    }

    /// The largest value minus the smallest.
    pub fn peak_to_peak(&self) -> i128 {
        let values: Vec<i128> = self.rows().iter().map(|row| row.value).collect();
        values.iter().max().expect("rows() is never empty") - values.iter().min().expect("rows() is never empty")
    }

    /// The rows; an empty window panics naming the label and the window, so a scenario never
    /// judges nothing.
    fn rows(&self) -> &[DiscreteSample] {
        if self.rows.is_empty() {
            let window = self.window.map_or("the whole run".to_string(), |(from, to)| format!("[{from:e} s, {to:e} s)"));
            panic!("`{}` has no row in {window}", self.label);
        }
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tables the tests read, built by hand.
    struct Tables;

    impl Tables {
        /// A trace of `mode` over `(time, cell)` rows.
        fn trace(rows: &[(f64, Cell)]) -> DiscreteTrace {
            let table = Table {
                columns: vec!["time".into(), "mode".into()],
                rows: rows.iter().map(|(time, cell)| vec![Cell::Real(*time), cell.clone()]).collect(),
            };
            DiscreteTrace::of(&table, "mode")
        }

        /// A trace of integer rows.
        fn integers(rows: &[(f64, i128)]) -> DiscreteTrace {
            let cells: Vec<(f64, Cell)> = rows.iter().map(|&(time, value)| (time, Cell::Int(value))).collect();
            Tables::trace(&cells)
        }
    }

    #[test]
    fn first_time_is_the_first_row_holding_the_value_after_a_time() {
        let trace = Tables::integers(&[(0.0, 0), (1.0, 2), (2.0, 3), (3.0, 2), (4.0, 3)]);
        // 2 is first held at t = 1; after t = 1.5 at t = 3
        assert_eq!(trace.first_time(2, 0.0), Some(1.0));
        assert_eq!(trace.first_time(2, 1.5), Some(3.0));
        // 1 is never held
        assert_eq!(trace.first_time(1, 0.0), None);
    }

    #[test]
    fn holds_names_the_first_row_that_departs() {
        let trace = Tables::trace(&[(0.0, Cell::Bool(true)), (1.0, Cell::Bool(true)), (2.0, Cell::Bool(false)), (3.0, Cell::Bool(true))]);
        // booleans read as 0 and 1; the first departure from 1 is the row at t = 2 holding 0
        assert_eq!(trace.holds(1), Err(Departure { time: 2.0, value: 0 }));
        // inside the window [0, 2) every row holds 1
        assert_eq!(trace.between(0.0, 2.0).holds(1), Ok(()));
        assert_eq!(trace.between(3.0, 4.0).holds(1), Ok(()));
    }

    #[test]
    fn value_at_is_the_row_at_or_before_the_time() {
        let trace = Tables::integers(&[(0.0, 0), (1.0, 2), (2.0, 3)]);
        assert_eq!(trace.value_at(1.0), 2);
        assert_eq!(trace.value_at(1.7), 2);
        assert_eq!(trace.value_at(9.0), 3);
    }

    #[test]
    fn peak_to_peak_is_the_largest_value_minus_the_smallest() {
        let trace = Tables::integers(&[(0.0, 130), (1.0, 128), (2.0, 131)]);
        assert_eq!(trace.peak_to_peak(), 3);
    }
}
