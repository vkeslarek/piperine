//! `Recorder`: the rows of a transient, made as the run goes (feature `long-run`, LR-03). The
//! session hands it each accepted step; it keeps the rows the run prints and nothing of the
//! steps the solver took. A step is read through a closure over the step's own values, so the
//! recorder knows nothing of the solver.

use crate::failure::Failure;
use crate::probe::Column;
use crate::table::{Cell, Table};

/// What a closure reads for one column of one step: its cell, `None` when the step holds no
/// value for it, a `Failure` when the value cannot be decoded.
pub(crate) type Read<'a> = dyn Fn(&Column) -> Result<Option<Cell>, Failure> + 'a;

/// The rows of one transient, made step by step.
pub(crate) struct Recorder {
    columns: Vec<Column>,
    labels: Vec<String>,
    from: f64,
    rows: Vec<Vec<Cell>>,
    /// The first failure of the run: the recorder makes no row after it and `finish` reports it.
    failure: Option<Failure>,
}

impl Recorder {
    /// A recorder of `columns`, named `labels` (one each, in order), keeping the steps at or after
    /// `from`.
    pub(crate) fn new(columns: Vec<Column>, labels: Vec<String>, from: f64) -> Recorder {
        Recorder { columns, labels, from, rows: Vec::new(), failure: None }
    }

    /// One accepted step at `time`, its values read by `read`.
    pub(crate) fn step(&mut self, time: f64, read: &Read<'_>) {
        if self.failure.is_some() {
            return;
        }
        match self.cells(time, read) {
            Ok(cells) => {
                if time >= self.from {
                    let mut row = vec![Cell::Real(time)];
                    row.extend(cells);
                    self.rows.push(row);
                }
            }
            Err(failure) => self.failure = Some(failure),
        }
    }

    /// Every column's cell at the step at `time`.
    fn cells(&self, time: f64, read: &Read<'_>) -> Result<Vec<Cell>, Failure> {
        self.columns
            .iter()
            .zip(&self.labels)
            .map(|(column, label)| read(column)?.ok_or_else(|| Failure::Backend(format!("the step at {time} holds no value for `{label}`"))))
            .collect()
    }

    /// The table of the run: the column `time`, then one per label.
    pub(crate) fn finish(self) -> Result<Table, Failure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let mut columns = vec!["time".to_string()];
        columns.extend(self.labels);
        Ok(Table { columns, rows: self.rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unknown(index: usize) -> Column {
        Column::Unknown { index }
    }

    /// The step whose solution is `solution`, read by index.
    fn solution(solution: &[f64]) -> impl Fn(&Column) -> Result<Option<Cell>, Failure> + '_ {
        move |column| match column {
            Column::Unknown { index } => Ok(solution.get(*index).map(|v| Cell::Real(*v))),
            Column::Net { .. } => Ok(None),
        }
    }

    fn times(table: &Table) -> Vec<f64> {
        table.rows.iter().map(|r| if let Cell::Real(t) = r[0] { t } else { panic!("a time") }).collect()
    }

    #[test]
    fn without_a_grid_each_step_at_or_after_from_is_a_row() {
        // LR-03 AC 1: every step from `from` on is a row: the time and each column's cell
        let mut recorder = Recorder::new(vec![unknown(1), unknown(0)], vec!["b".into(), "a".into()], 0.0);
        for (t, a, b) in [(0.0, 1.0, 10.0), (0.4, 2.0, 20.0), (1.0, 3.0, 30.0)] {
            recorder.step(t, &solution(&[a, b]));
        }
        let table = recorder.finish().expect("table");
        assert_eq!(table.columns, ["time", "b", "a"]);
        assert_eq!(table.rows, [
            vec![Cell::Real(0.0), Cell::Real(10.0), Cell::Real(1.0)],
            vec![Cell::Real(0.4), Cell::Real(20.0), Cell::Real(2.0)],
            vec![Cell::Real(1.0), Cell::Real(30.0), Cell::Real(3.0)],
        ]);
    }

    #[test]
    fn a_from_past_the_start_drops_the_start_row() {
        // LR-03 AC 2: a step before `from` is no row; a step at `from` is
        let mut recorder = Recorder::new(vec![unknown(0)], vec!["a".into()], 0.4);
        for (t, a) in [(0.0, 1.0), (0.3, 2.0), (0.4, 3.0), (0.9, 4.0)] {
            recorder.step(t, &solution(&[a]));
        }
        let table = recorder.finish().expect("table");
        assert_eq!(times(&table), [0.4, 0.9]);
        assert_eq!(table.rows[0][1], Cell::Real(3.0));
    }

    #[test]
    fn a_signal_named_twice_gives_two_equal_columns() {
        // spec edge 3: a signal printed twice is two columns holding the same values
        let mut recorder = Recorder::new(vec![unknown(0), unknown(0)], vec!["a".into(), "a".into()], 0.0);
        recorder.step(0.0, &solution(&[5.0]));
        recorder.step(1.0, &solution(&[6.0]));
        let table = recorder.finish().expect("table");
        assert_eq!(table.columns, ["time", "a", "a"]);
        for row in &table.rows {
            assert_eq!(row[1], row[2]);
        }
    }

    #[test]
    fn a_column_that_reads_nothing_fails_naming_it() {
        // LR-03 AC 3: a column a step holds no value for is a failure naming its label, never a
        // zero or a skipped cell
        let mut recorder = Recorder::new(vec![unknown(0), unknown(7)], vec!["a".into(), "ghost".into()], 0.0);
        recorder.step(0.0, &solution(&[1.0]));
        match recorder.finish() {
            Err(Failure::Backend(message)) => assert!(message.contains("`ghost`"), "{message}"),
            other => panic!("a Failure naming ghost, found {other:?}"),
        }
    }
}
