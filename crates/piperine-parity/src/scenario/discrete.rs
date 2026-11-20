//! [`DiscreteTrace`]: one discrete column of a run (a mode, a fault code, a bit) over a window.

use piperine_api::{Cell, Table};

use super::column::NamedColumn;

/// One row of a discrete trace: its time and what it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscreteSample {
    /// The time of the row, in seconds.
    pub time: f64,
    /// The value the row holds; none when the column was unknown there.
    pub value: Option<i128>,
}

/// The first row that does not hold what a trace was asked to hold: the error of
/// [`DiscreteTrace::holds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Departure {
    /// The time of the departing row, in seconds.
    pub time: f64,
    /// The value the row holds; none when the column was unknown there.
    pub value: Option<i128>,
}

/// The rows of one discrete column with the time of each, optionally cut to a window. A `Bool`
/// cell reads as 0 or 1 and an `Unknown` cell as none, so a mode, a fault code and a flag are all
/// read the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteTrace {
    label: String,
    window: Option<(f64, f64)>,
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
                    Cell::Int(value) => Some(*value),
                    Cell::Bool(flag) => Some(i128::from(*flag)),
                    Cell::Unknown => None,
                    other => panic!("row {row} of `{}` is not a discrete value: {other:?}", column.label()),
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
        self.rows.iter().find(|row| row.time >= after && row.value == Some(value)).map(|row| row.time)
    }

    /// Passes when every row holds `value`; otherwise the first row that does not, with its time
    /// and what it holds. An unknown row does not hold any value.
    pub fn holds(&self, value: i128) -> Result<(), Departure> {
        self.rows().iter().find(|row| row.value != Some(value)).map_or(Ok(()), |row| Err(Departure { time: row.time, value: row.value }))
    }

    /// What the row at or before `time` holds (the last row not after `time`); none when it was
    /// unknown. A time before the first row panics: a scenario reads an instant the run covers.
    pub fn value_at(&self, time: f64) -> Option<i128> {
        match self.rows.iter().rev().find(|row| row.time <= time) {
            Some(row) => row.value,
            None => panic!("`{}` has no row at or before {time:e} s", self.label),
        }
    }

    /// The largest known value minus the smallest. A row that is unknown panics naming it: a
    /// peak-to-peak over an unknown is no measurement.
    pub fn peak_to_peak(&self) -> i128 {
        let known: Vec<i128> = self
            .rows()
            .iter()
            .map(|row| row.value.unwrap_or_else(|| panic!("`{}` is unknown at {:e} s", self.label, row.time)))
            .collect();
        known.iter().max().expect("rows() is never empty") - known.iter().min().expect("rows() is never empty")
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
    use std::panic::{AssertUnwindSafe, catch_unwind};

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
        let trace = Tables::trace(&[(0.0, Cell::Bool(true)), (1.0, Cell::Bool(true)), (2.0, Cell::Bool(false)), (3.0, Cell::Unknown)]);
        // booleans read as 0 and 1; the first departure from 1 is the row at t = 2 holding 0
        assert_eq!(trace.holds(1), Err(Departure { time: 2.0, value: Some(0) }));
        // inside the window [0, 2) every row holds 1
        assert_eq!(trace.between(0.0, 2.0).holds(1), Ok(()));
        // an unknown row holds nothing: it departs with no value
        assert_eq!(trace.between(3.0, 4.0).holds(1), Err(Departure { time: 3.0, value: None }));
    }

    #[test]
    fn value_at_is_the_row_at_or_before_the_time() {
        let trace = Tables::integers(&[(0.0, 0), (1.0, 2), (2.0, 3)]);
        assert_eq!(trace.value_at(1.0), Some(2));
        assert_eq!(trace.value_at(1.7), Some(2));
        assert_eq!(trace.value_at(9.0), Some(3));
        assert_eq!(Tables::trace(&[(0.0, Cell::Unknown)]).value_at(0.5), None);
    }

    #[test]
    fn peak_to_peak_is_the_largest_known_value_minus_the_smallest_and_panics_on_an_unknown_row() {
        let trace = Tables::integers(&[(0.0, 130), (1.0, 128), (2.0, 131)]);
        assert_eq!(trace.peak_to_peak(), 3);
        let unknown = Tables::trace(&[(0.0, Cell::Int(1)), (1.0, Cell::Unknown)]);
        let payload = catch_unwind(AssertUnwindSafe(|| unknown.peak_to_peak())).expect_err("an unknown row must panic");
        let text = payload.downcast_ref::<String>().expect("a formatted panic message");
        assert!(text.contains("`mode`") && text.contains("unknown") && text.contains("1e0"), "{text}");
    }
}
