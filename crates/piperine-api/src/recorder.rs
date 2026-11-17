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

/// The print grid of a run: the multiples `k*h` of `h` from the first not before `from` to the
/// last not past `stop`, and what it holds while the run goes. The bounds and the slack are the
/// ones `Table::on_grid` computes, so the rows are the same bits.
struct Grid {
    h: f64,
    kmax: u64,
    slack: f64,
    /// The next multiple to make a row for.
    k: u64,
    /// The previous step: its time and one cell per column. The only step held.
    prev: Option<(f64, Vec<Cell>)>,
}

/// The rows of one transient, made step by step.
pub(crate) struct Recorder {
    columns: Vec<Column>,
    labels: Vec<String>,
    from: f64,
    grid: Option<Grid>,
    rows: Vec<Vec<Cell>>,
    /// The first failure of the run: the recorder makes no row after it and `finish` reports it.
    failure: Option<Failure>,
}

impl Recorder {
    /// A recorder of `columns`, named `labels` (one each, in order), keeping the steps at or after
    /// `from`.
    pub(crate) fn new(columns: Vec<Column>, labels: Vec<String>, from: f64) -> Recorder {
        Recorder { columns, labels, from, grid: None, rows: Vec::new(), failure: None }
    }

    /// The recorder of a run with a print grid of step `h` up to `stop`: one row per multiple
    /// of `h` from the first not before `from` (CLI-11 AC 4).
    pub(crate) fn with_grid(mut self, h: f64, stop: f64) -> Recorder {
        // the tolerance on k absorbs the rounding of `stop / h` and `from / h`
        let kmin = (self.from / h - 1e-9).ceil().max(0.0) as u64;
        let kmax = (stop / h + 1e-9).floor() as u64;
        self.grid = Some(Grid { h, kmax, slack: 1e-9 * h, k: kmin, prev: None });
        self
    }

    /// One accepted step at `time`, its values read by `read`.
    pub(crate) fn step(&mut self, time: f64, read: &Read<'_>) {
        if self.failure.is_some() {
            return;
        }
        match self.cells(time, read) {
            Ok(cells) if self.grid.is_some() => self.on_grid(time, cells),
            Ok(cells) => {
                if time >= self.from {
                    self.rows.push(Recorder::row(time, cells));
                }
            }
            Err(failure) => self.failure = Some(failure),
        }
    }

    fn row(time: f64, cells: Vec<Cell>) -> Vec<Cell> {
        let mut row = vec![Cell::Real(time)];
        row.extend(cells);
        row
    }

    /// A step on the grid: a row for every pending multiple not past it, then the step is the
    /// previous one. A multiple before the first step is no row; a node or a flow on an accepted
    /// step is that step's value and between two steps the linear interpolation of
    /// `Table::on_grid`, expression for expression.
    fn on_grid(&mut self, time: f64, cells: Vec<Cell>) {
        let Some(grid) = self.grid.as_mut() else { return };
        if grid.prev.is_none() {
            while grid.k <= grid.kmax && (grid.k as f64 * grid.h) < time {
                grid.k += 1;
            }
        }
        while grid.k <= grid.kmax {
            let at = grid.k as f64 * grid.h;
            if at > time {
                break;
            }
            let row = match &grid.prev {
                Some((t0, before)) if at != time => {
                    let held = before.iter().zip(&cells).zip(&self.columns).map(|((a, b), column)| match (column, a, b) {
                        (Column::Unknown { .. }, Cell::Real(a), Cell::Real(b)) => Cell::Real(a + (b - a) * (at - t0) / (time - t0)),
                        (Column::Unknown { .. }, _, _) => Cell::Unknown,
                        (Column::Net { .. }, held, _) => held.clone(),
                    });
                    Recorder::row(at, held.collect())
                }
                _ => Recorder::row(at, cells.clone()),
            };
            self.rows.push(row);
            grid.k += 1;
        }
        grid.prev = Some((time, cells));
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
    pub(crate) fn finish(mut self) -> Result<Table, Failure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if let Some(Grid { h, kmax, slack, k, prev: Some((last, cells)) }) = self.grid {
            // a multiple within rounding of the last step takes that step's values; one further
            // past is a run that stopped short
            for k in k..=kmax {
                let at = k as f64 * h;
                if at > last + slack {
                    return Err(Failure::Backend(format!("the print grid reaches {at}, past the last step of the run at {last}")));
                }
                self.rows.push(Recorder::row(at, cells.clone()));
            }
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

    /// A run of one node column over `steps`, on a grid of `h` to `stop` from `from`, fed step by
    /// step.
    fn gridded(steps: &[(f64, f64)], h: f64, from: f64, stop: f64) -> Recorder {
        let mut recorder = Recorder::new(vec![unknown(0)], vec!["v".into()], from).with_grid(h, stop);
        for (t, v) in steps {
            recorder.step(*t, &solution(&[*v]));
        }
        recorder
    }

    #[test]
    fn grid_rows_run_from_the_first_multiple_not_before_from_to_stop() {
        // LR-05 AC 1: 0.5 on a grid from 0.6 to 2.0: the multiples 1.0, 1.5 and 2.0
        let steps = [(0.0, 0.0), (0.7, 7.0), (1.3, 13.0), (2.0, 20.0)];
        let table = gridded(&steps, 0.5, 0.6, 2.0).finish().expect("table");
        assert_eq!(times(&table), [1.0, 1.5, 2.0]);
        // a `from` that is a multiple keeps it
        assert_eq!(times(&gridded(&steps, 0.5, 1.0, 2.0).finish().expect("table")), [1.0, 1.5, 2.0]);
    }

    /// Irregular accepted steps from 0 to 1, values that no line fits.
    fn irregular() -> Vec<(f64, f64)> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut time = 0.0;
        let mut steps = vec![(0.0, 0.0)];
        while time < 1.0 {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            time = (time + 0.003 + (state >> 40) as f64 / (1u64 << 24) as f64 * 0.04).min(1.0);
            steps.push((time, (time * 7.3).sin() + time * time));
        }
        steps
    }

    #[test]
    fn a_real_column_is_bit_equal_to_on_grid() {
        // LR-06 AC 1: the recorder's rows are the rows `on_grid` makes of the table of every step,
        // compared by bits, with and without `from`, for grids that miss and hit the steps
        let steps = irregular();
        let all = Table {
            columns: vec!["time".into(), "v".into()],
            rows: steps.iter().map(|(t, v)| vec![Cell::Real(*t), Cell::Real(*v)]).collect(),
        };
        for (h, from) in [(0.01, 0.0), (0.0137, 0.0), (0.01, 0.25), (0.0137, 0.3331), (0.1, 0.0), (0.25, 0.2)] {
            let expected = all.on_grid(h, from, 1.0).unwrap_or_else(|f| panic!("{f}"));
            let table = gridded(&steps, h, from, 1.0).finish().unwrap_or_else(|f| panic!("{f}"));
            assert_eq!(table.rows.len(), expected.rows.len(), "h {h} from {from}");
            for (row, want) in table.rows.iter().zip(&expected.rows) {
                for (cell, want) in row.iter().zip(want) {
                    let (Cell::Real(a), Cell::Real(b)) = (cell, want) else { panic!("reals") };
                    assert_eq!(a.to_bits(), b.to_bits(), "h {h} from {from}: {a} against {b}");
                }
            }
        }
    }

    #[test]
    fn a_row_on_an_accepted_step_takes_its_value() {
        // LR-06 AC 2: a multiple that is a step's time is that step's value, not an
        // interpolation (1/3 + (0.9 - 1/3) is 0.8999999999999999, not 0.9)
        let a = 1.0 / 3.0;
        assert_ne!(a + (0.9 - a), 0.9, "the interpolation would differ from the value");
        let table = gridded(&[(0.0, 0.0), (0.25, a), (0.5, 0.9)], 0.5, 0.0, 0.5).finish().expect("table");
        assert_eq!(table.rows[1], vec![Cell::Real(0.5), Cell::Real(0.9)]);
    }

    #[test]
    fn each_grid_row_is_made_when_its_step_arrives() {
        // LR-04 AC 1, LR-05 AC 2: after each step the rows are the multiples up to it, and the
        // recorder holds one step, the previous
        let mut recorder = Recorder::new(vec![unknown(0)], vec!["v".into()], 0.0).with_grid(0.5, 2.0);
        let mut counts = Vec::new();
        for (t, v) in [(0.0, 0.0), (0.3, 3.0), (1.2, 12.0), (1.4, 14.0), (2.0, 20.0)] {
            recorder.step(t, &solution(&[v]));
            let held = recorder.grid.as_ref().and_then(|g| g.prev.as_ref()).map(|(t, cells)| (*t, cells.len()));
            assert_eq!(held, Some((t, 1)), "the step just taken is the one held");
            counts.push(recorder.rows.len());
        }
        // 0.0 at once; none at 0.3; 0.5 and 1.0 at 1.2; none at 1.4; 1.5 and 2.0 at 2.0
        assert_eq!(counts, [1, 1, 3, 3, 5]);
    }

    #[test]
    fn a_row_within_rounding_of_the_last_step_takes_its_values() {
        // LR-05 AC 5: a multiple a hair past the last step (within 1e-9 of the step) is a row of
        // the last step's values; the run's last step is 0.3 less a rounding
        let last = 0.3 - 1e-12;
        let table = gridded(&[(0.0, 0.0), (0.1, 1.0), (last, 3.0)], 0.1, 0.0, 0.3).finish().expect("table");
        assert_eq!(times(&table).len(), 4, "0, 0.1, 0.2 and 3 * 0.1");
        assert_eq!(table.rows[3][1], Cell::Real(3.0));
    }

    #[test]
    fn a_row_past_the_last_step_fails() {
        // LR-05 AC 6: a multiple further past the last step than rounding is a failure with
        // `on_grid`'s message, never a row held past the run's end
        match gridded(&[(0.0, 0.0), (1.0, 1.0)], 0.5, 0.0, 2.0).finish() {
            Err(Failure::Backend(message)) => assert!(message.contains("the print grid reaches 1.5, past the last step of the run at 1"), "{message}"),
            other => panic!("a failure, found {other:?}"),
        }
    }

    #[test]
    fn a_print_step_past_stop_gives_the_row_at_zero() {
        // edge case: a print step greater than stop gives the one row at 0
        let table = gridded(&[(0.0, 4.0), (1.0, 8.0), (2.5, 9.0)], 10.0, 0.0, 2.5).finish().expect("table");
        assert_eq!(table.rows, [vec![Cell::Real(0.0), Cell::Real(4.0)]]);
    }
}
