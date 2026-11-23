//! `Recorder`: the rows of a transient, made as the run goes (feature `long-run`, LR-03). The
//! session hands it each accepted step; it keeps the rows the run prints and nothing of the
//! steps the solver took. A step is read through a closure over the step's own values, so the
//! recorder knows nothing of the solver.
//!
//! A discrete net is held, not sampled: the solver reports each change of a net as it commits
//! it (`Recorder::change`, before the step's call), so a row on the grid at `k*h` shows the net
//! as it was after the last instant at or before `k*h`, however many instants lay between two
//! analog steps. At every step the value the changes add up to must be the one the solver
//! reads; a difference is a change the solver did not report, and the run fails naming the net.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use piperine_solver::prelude::{StepObserver, StepView};

use crate::failure::Failure;
use crate::probe::{Column, Probe};
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
    /// The columns of each printed net, by its path (a net printed twice is two columns).
    by_path: HashMap<String, Vec<usize>>,
    /// Each net column's value as the changes so far make it; a placeholder for a column that is no
    /// net.
    held: Vec<Cell>,
    /// The changes of printed nets since the previous step, in the order they were made:
    /// the column, the time and the value after it.
    changes: Vec<(usize, f64, Cell)>,
    /// How many of `changes` a grid row has applied to `held` already.
    applied: usize,
    /// Whether a step has been seen: the first seeds `held` from the net reader.
    started: bool,
    /// The rows already made whose time plus the slack is still past the last step: the index in
    /// `rows` and the time. A change that arrives with a time inside one's slack reaches it, so
    /// at most the rows of one slack window are held here.
    pending: Vec<(usize, f64)>,
    /// The first failure of the run: the recorder makes no row after it and `finish` reports it.
    failure: Option<Failure>,
    /// The time of the last step the observer saw: how far the run got, with or without an
    /// analog unknown to step (HII-17).
    reached: Option<f64>,
}

impl Recorder {
    /// A recorder of `columns`, named `labels` (one each, in order), keeping the steps at or after
    /// `from`.
    pub(crate) fn new(columns: Vec<Column>, labels: Vec<String>, from: f64) -> Recorder {
        let mut by_path: HashMap<String, Vec<usize>> = HashMap::new();
        for (at, (column, label)) in columns.iter().zip(&labels).enumerate() {
            match column {
                Column::Net { .. } => by_path.entry(label.clone()).or_default().push(at),
                // a column showing one leaf of a net is still changed by the net's path
                Column::Shaped { path, .. } => by_path.entry(path.clone()).or_default().push(at),
                Column::Unknown { .. } | Column::Param { .. } | Column::Held { .. } | Column::Array { .. } => {}
            }
        }
        // every net column is seeded by the first step, and no other column reads `held`
        let held = vec![Cell::Real(0.0); columns.len()];
        Recorder { columns, labels, from, grid: None, rows: Vec::new(), by_path, held, changes: Vec::new(), applied: 0, started: false, pending: Vec::new(), failure: None, reached: None }
    }

    /// A change of the net at `path` to `bytes` at `time`, as the solver commits it. A path no
    /// column prints is not the recorder's.
    pub(crate) fn change(&mut self, path: &str, time: f64, bytes: &[u8]) {
        let Some(at) = self.by_path.get(path) else { return };
        let slack = self.grid.as_ref().map_or(0.0, |grid| grid.slack);
        if self.failure.is_some() {
            return;
        }
        for &column in at {
            let decoded = match &self.columns[column] {
                Column::Net { ty, .. } => Cell::decode(ty, bytes),
                Column::Shaped { shape, leaf, .. } => Probe::shown(shape, leaf.as_deref(), bytes),
                _ => continue,
            };
            match decoded {
                Ok(cell) => {
                    Recorder::reach_pending(&mut self.rows, &self.pending, slack, column, time, &cell);
                    self.changes.push((column, time, cell));
                }
                Err(failure) => {
                    self.failure = Some(failure);
                    return;
                }
            }
        }
    }

    /// A change at `time` reaches every row already made whose time plus the slack is not before
    /// it: the row shows the net as the last instant within rounding of it left it.
    fn reach_pending(rows: &mut [Vec<Cell>], pending: &[(usize, f64)], slack: f64, column: usize, time: f64, cell: &Cell) {
        for &(row, at) in pending {
            if time <= at + slack {
                rows[row][column + 1] = cell.clone();
            }
        }
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

    /// The run ended at `end` (a `finish` before its `stop`): the grid has no multiple past it.
    pub(crate) fn end_at(&mut self, end: f64) {
        if let Some(grid) = self.grid.as_mut() {
            grid.kmax = grid.kmax.min((end / grid.h + 1e-9).floor() as u64);
        }
    }

    /// The time of the last step the observer saw; `None` before the first.
    pub(crate) fn reached(&self) -> Option<f64> {
        self.reached
    }

    /// One accepted step at `time`, its values read by `read`.
    pub(crate) fn step(&mut self, time: f64, read: &Read<'_>) {
        self.reached = Some(time);
        if self.failure.is_some() {
            return;
        }
        let cells = match self.cells(time, read) {
            Ok(cells) => cells,
            Err(failure) => {
                self.failure = Some(failure);
                return;
            }
        };
        if !self.started {
            // the start of a run: a net holds what the reader says (a restored run has no change
            // for it), and the changes of instants up to the start are in it
            self.started = true;
            self.changes.clear();
            for (held, (cell, column)) in self.held.iter_mut().zip(cells.iter().zip(&self.columns)) {
                if matches!(column, Column::Net { .. } | Column::Shaped { .. }) {
                    *held = cell.clone();
                }
            }
        }
        if self.grid.is_some() {
            self.on_grid(time, &cells);
        } else if time >= self.from {
            self.rows.push(Recorder::row(time, cells.clone()));
        }
        self.settle(time, cells);
    }

    /// Applies the changes up to `limit` (every one when `None`) not yet applied, in order.
    fn apply(&mut self, limit: Option<f64>) {
        while let Some((column, time, cell)) = self.changes.get(self.applied) {
            if limit.is_some_and(|limit| *time > limit) {
                break;
            }
            self.held[*column] = cell.clone();
            self.applied += 1;
        }
    }

    /// The end of a step: every change of the step is applied, and each net the changes make
    /// must be the net the solver reads. Then the step is the previous one.
    fn settle(&mut self, time: f64, mut cells: Vec<Cell>) {
        self.apply(None);
        self.changes.clear();
        self.applied = 0;
        for (at, column) in self.columns.iter().enumerate() {
            if matches!(column, Column::Net { .. } | Column::Shaped { .. }) {
                if !Recorder::same(&self.held[at], &cells[at]) {
                    let (held, read) = (&self.held[at], &cells[at]);
                    self.failure = Some(Failure::Backend(format!(
                        "the net `{}` changed without a change reaching the recorder: its changes make {held:?} at {time} and the solver reads {read:?}",
                        self.labels[at]
                    )));
                    return;
                }
                cells[at] = self.held[at].clone();
            }
        }
        if let Some(grid) = self.grid.as_mut() {
            grid.prev = Some((time, cells));
        }
    }

    /// Whether two cells are the same value, a real by its bits.
    fn same(a: &Cell, b: &Cell) -> bool {
        match (a, b) {
            (Cell::Real(a), Cell::Real(b)) => a.to_bits() == b.to_bits(),
            _ => a == b,
        }
    }

    fn row(time: f64, cells: Vec<Cell>) -> Vec<Cell> {
        let mut row = vec![Cell::Real(time)];
        row.extend(cells);
        row
    }

    /// A step on the grid: a row for every pending multiple not past it. A multiple before the
    /// first step is no row; a node or a flow on an accepted step is that step's value and
    /// between two steps the linear interpolation of `Table::on_grid`, expression for
    /// expression; a net is its value after the last change at or before the multiple.
    fn on_grid(&mut self, time: f64, cells: &[Cell]) {
        let Some(grid) = self.grid.as_ref() else { return };
        let (h, kmax, slack) = (grid.h, grid.kmax, grid.slack);
        let mut k = grid.k;
        self.pending.retain(|(_, at)| at + slack > time);
        if grid.prev.is_none() {
            while k <= kmax && (k as f64 * h) < time {
                k += 1;
            }
        }
        while k <= kmax {
            let at = k as f64 * h;
            if at > time {
                break;
            }
            self.apply(Some(at + slack));
            let prev = self.grid.as_ref().and_then(|g| g.prev.as_ref());
            let values = cells.iter().enumerate().map(|(i, b)| match (&self.columns[i], prev) {
                (Column::Net { .. } | Column::Shaped { .. }, _) => self.held[i].clone(),
                (Column::Unknown { .. }, Some((t0, before))) if at != time => match (&before[i], b) {
                    (Cell::Real(a), Cell::Real(b)) => Cell::Real(a + (b - a) * (at - t0) / (time - t0)),
                    (before, _) => before.clone(),
                },
                (Column::Held { held }, _) => held.cell_at(at),
                (Column::Unknown { .. } | Column::Param { .. } | Column::Array { .. }, _) => b.clone(),
            });
            let row = Recorder::row(at, values.collect());
            if at + slack > time {
                self.pending.push((self.rows.len(), at));
            }
            self.rows.push(row);
            k += 1;
        }
        if let Some(grid) = self.grid.as_mut() {
            grid.k = k;
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

/// The solver's step observer of a run: each accepted step goes to the recorder, read through the
/// step's solution and net reader.
pub(crate) struct Tap(pub(crate) Rc<RefCell<Recorder>>);

impl StepObserver for Tap {
    fn step(&mut self, step: &StepView<'_>) {
        self.0.borrow_mut().step(step.time(), &|column| match column {
            Column::Unknown { index } => Ok(step.unknown(*index).map(Cell::Real)),
            Column::Net { id, ty } => step.net(*id).map(|bytes| Cell::decode(ty, bytes)).transpose(),
            Column::Shaped { id, shape, leaf, .. } => step.net_bytes(*id).map(|bytes| Probe::shown(shape, leaf.as_deref(), &bytes)).transpose(),
            Column::Array { id, shape, leaf } => step.net_bytes(*id).map(|bytes| Probe::shown(shape, leaf.as_deref(), &bytes)).transpose(),
            Column::Param { id, ty } => step.net(*id).map(|bytes| Cell::decode_param(ty, bytes)).transpose(),
            Column::Held { held } => Ok(Some(held.cell_at(step.time()))),
        });
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::ValueType;
    use piperine_plan::CarrierLayout;

    use super::*;

    fn unknown(index: usize) -> Column {
        Column::Unknown { index }
    }

    /// The step whose solution is `solution`, read by index.
    fn solution(solution: &[f64]) -> impl Fn(&Column) -> Result<Option<Cell>, Failure> + '_ {
        move |column| match column {
            Column::Unknown { index } => Ok(solution.get(*index).map(|v| Cell::Real(*v))),
            Column::Net { .. } | Column::Shaped { .. } | Column::Param { .. } | Column::Held { .. } | Column::Array { .. } => Ok(None),
        }
    }

    fn times(table: &Table) -> Vec<f64> {
        table.rows.iter().map(|r| if let Cell::Real(t) = r[0] { t } else { panic!("a time") }).collect()
    }

    #[test]
    fn the_recorder_reports_the_time_of_the_last_step_it_saw() {
        // HII-17 AC 3 (CLI-05 AC 11): how far a run got is its last observed step, `None` before one
        let mut recorder = Recorder::new(vec![unknown(0)], vec!["a".into()], 0.0);
        assert_eq!(recorder.reached(), None);
        recorder.step(0.0, &solution(&[1.0]));
        recorder.step(2e-6, &solution(&[1.0]));
        assert_eq!(recorder.reached(), Some(2e-6));
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

    // -- discrete nets -----------------------------------------------------------------------

    fn net(id: u32, ty: ValueType) -> Column {
        Column::Net { id, ty }
    }

    /// The bytes of `cell` as a carrier of `ty` holds them: the value at its offset.
    fn bytes(ty: &ValueType, cell: &Cell) -> Vec<u8> {
        let layout = CarrierLayout::of(ty).expect("a carrier");
        let field = layout.fields.get(0).expect("a field");
        let mut out = vec![0u8; layout.size as usize];
        let at = field.offset as usize;
        match cell {
            Cell::Bool(v) => out[at] = u8::from(*v),
            Cell::Real(v) => out[at..at + 8].copy_from_slice(&v.to_le_bytes()),
            Cell::Int(v) => out[at..at + 4].copy_from_slice(&(*v as i32).to_le_bytes()),
            Cell::Text(_) | Cell::Composite(_) | Cell::Absent => panic!("no scalar net holds text, a composite or an absent cell"),
        }
        out
    }

    /// One analog unknown and the nets, read as the cells the solver would hold at the step.
    fn reading<'a>(v: f64, nets: &'a [Cell]) -> impl Fn(&Column) -> Result<Option<Cell>, Failure> + 'a {
        move |column| match column {
            Column::Unknown { .. } => Ok(Some(Cell::Real(v))),
            Column::Net { id, .. } | Column::Shaped { id, .. } | Column::Param { id, .. } | Column::Array { id, .. } => Ok(nets.get(*id as usize).cloned()),
            Column::Held { held } => Ok(Some(held.cell_at(0.0))),
        }
    }

    /// A recorder of `v` and the net `q` of type `ty`, on a grid of `h` to `stop`.
    fn with_net(ty: &ValueType, h: f64, stop: f64) -> Recorder {
        Recorder::new(vec![unknown(0), net(0, ty.clone())], vec!["v".into(), "T.q".into()], 0.0).with_grid(h, stop)
    }

    fn q(table: &Table) -> Vec<Cell> {
        table.rows.iter().map(|r| r[2].clone()).collect()
    }

    #[test]
    fn a_discrete_column_holds_the_value_after_the_last_instant() {
        // LR-05 AC 4: at 0.5 the net is what the last instant at or before 0.5 left, though two
        // instants between the analog steps changed it; at 1.0 it is the change at 0.9
        let ty = ValueType::Bool;
        let mut recorder = with_net(&ty, 0.5, 1.5);
        recorder.step(0.0, &reading(0.0, &[Cell::Bool(false)]));
        for (time, value) in [(0.4, true), (0.4, false), (0.9, true)] {
            recorder.change("T.q", time, &bytes(&ty, &Cell::Bool(value)));
        }
        recorder.step(1.5, &reading(0.0, &[Cell::Bool(true)]));
        let table = recorder.finish().expect("table");
        assert_eq!(q(&table), [Cell::Bool(false), Cell::Bool(false), Cell::Bool(true), Cell::Bool(true)]);
    }

    #[test]
    fn a_change_between_two_steps_reaches_the_rows_after_it() {
        // LR-05 AC 4: a change at 0.5 between the steps at 0 and 1 is in the rows from 0.5 on
        // (a row at the instant of the change shows it) and not in 0.25
        let ty = ValueType::Bool;
        let mut recorder = with_net(&ty, 0.25, 1.0);
        recorder.step(0.0, &reading(0.0, &[Cell::Bool(false)]));
        recorder.change("T.q", 0.5, &bytes(&ty, &Cell::Bool(true)));
        recorder.step(1.0, &reading(0.0, &[Cell::Bool(true)]));
        let table = recorder.finish().expect("table");
        let want = [false, false, true, true, true].map(Cell::Bool);
        assert_eq!(q(&table), want);
    }

    #[test]
    fn a_discrete_column_that_changes_only_at_steps_equals_on_grid() {
        // LR-06 AC 3: when a net changes only at accepted steps, the held column is the column
        // `on_grid` makes of the table of every step, for a bool and an integer
        let steps = irregular();
        for ty in [ValueType::Bool, ValueType::Int(32)] {
            let cell = |n: usize| if ty == ValueType::Bool { Cell::Bool(n / 7 % 2 == 1) } else { Cell::Int((n / 7) as i128) };
            let all = Table {
                columns: vec!["time".into(), "v".into(), "T.q".into()],
                rows: steps.iter().enumerate().map(|(n, (t, v))| vec![Cell::Real(*t), Cell::Real(*v), cell(n)]).collect(),
            };
            for (h, from) in [(0.01, 0.0), (0.0137, 0.3331), (0.25, 0.2)] {
                let mut recorder = Recorder::new(vec![unknown(0), net(0, ty.clone())], vec!["v".into(), "T.q".into()], from).with_grid(h, 1.0);
                for (n, (t, v)) in steps.iter().enumerate() {
                    // the solver reports a change when it commits it, before the step's call
                    if n == 0 || cell(n) != cell(n - 1) {
                        recorder.change("T.q", *t, &bytes(&ty, &cell(n)));
                    }
                    recorder.step(*t, &reading(*v, &[cell(n)]));
                }
                let want = all.on_grid(h, from, 1.0).unwrap_or_else(|f| panic!("{f}"));
                let got = recorder.finish().unwrap_or_else(|f| panic!("{f}"));
                assert_eq!(got.rows.len(), want.rows.len(), "{ty} h {h}");
                for (row, want) in got.rows.iter().zip(&want.rows) {
                    assert_eq!(row[0], want[0], "{ty} h {h}");
                    assert_eq!(row[2], want[2], "{ty} h {h} at {:?}", row[0]);
                }
            }
        }
    }

    #[test]
    fn a_change_within_rounding_of_a_row_counts_at_it() {
        // LR-05 AC 5: a change 1e-10 past the multiple 0.5 (slack 5e-10) is at it; one 1e-6 past
        // is after it
        let ty = ValueType::Bool;
        for (late, at_row) in [(1e-10, true), (1e-6, false)] {
            let mut recorder = with_net(&ty, 0.5, 1.0);
            recorder.step(0.0, &reading(0.0, &[Cell::Bool(false)]));
            recorder.change("T.q", 0.5 + late, &bytes(&ty, &Cell::Bool(true)));
            recorder.step(1.0, &reading(0.0, &[Cell::Bool(true)]));
            let table = recorder.finish().expect("table");
            assert_eq!(q(&table)[1], Cell::Bool(at_row), "a change {late} past the row");
            assert_eq!(q(&table)[2], Cell::Bool(true));
        }
    }

    #[test]
    fn a_change_within_rounding_after_a_row_already_made_reaches_it() {
        // LR-05 AC 5 (verification F1): steps at 0, 0.5, 1.0 on the grid 0.5; the row at 0.5 is
        // made at the step at 0.5, and the change 1e-10 past it (slack 5e-10) arrives after; the
        // row shows the changed value and its analog cell stays the step's own
        let ty = ValueType::Bool;
        let mut recorder = with_net(&ty, 0.5, 1.0);
        recorder.step(0.0, &reading(1.0, &[Cell::Bool(false)]));
        recorder.step(0.5, &reading(2.0, &[Cell::Bool(false)]));
        recorder.change("T.q", 0.5 + 1e-10, &bytes(&ty, &Cell::Bool(true)));
        recorder.step(1.0, &reading(4.0, &[Cell::Bool(true)]));
        let table = recorder.finish().expect("table");
        assert_eq!(q(&table), [Cell::Bool(false), Cell::Bool(true), Cell::Bool(true)]);
        let v: Vec<Cell> = table.rows.iter().map(|r| r[1].clone()).collect();
        assert_eq!(v, [Cell::Real(1.0), Cell::Real(2.0), Cell::Real(4.0)]);
    }

    #[test]
    fn several_changes_and_steps_within_the_slack_of_a_made_row_all_reach_it() {
        // F1: the row at 0.5 stays pending through a second step 2e-10 past it, then two changes
        // arrive (the last wins); a change 1e-6 past the row, beyond its slack, is not the row's.
        // A second net column is updated beside the first
        let ty = ValueType::Bool;
        let columns = vec![unknown(0), net(0, ty.clone()), net(1, ty.clone())];
        let labels = vec!["v".into(), "T.q".into(), "T.r".into()];
        let mut recorder = Recorder::new(columns, labels, 0.0).with_grid(0.5, 1.0);
        let off = [Cell::Bool(false), Cell::Bool(false)];
        recorder.step(0.0, &reading(1.0, &off));
        recorder.step(0.5, &reading(2.0, &off));
        recorder.step(0.5 + 2e-10, &reading(2.0, &off));
        recorder.change("T.q", 0.5 + 3e-10, &bytes(&ty, &Cell::Bool(true)));
        recorder.change("T.r", 0.5 + 3e-10, &bytes(&ty, &Cell::Bool(true)));
        recorder.change("T.q", 0.5 + 4e-10, &bytes(&ty, &Cell::Bool(false)));
        recorder.change("T.r", 0.5 + 1e-6, &bytes(&ty, &Cell::Bool(false)));
        recorder.step(1.0, &reading(4.0, &[Cell::Bool(false), Cell::Bool(false)]));
        let table = recorder.finish().expect("table");
        let col = |at: usize| table.rows.iter().map(|r| r[at].clone()).collect::<Vec<_>>();
        assert_eq!(col(2), [Cell::Bool(false), Cell::Bool(false), Cell::Bool(false)]);
        assert_eq!(col(3), [Cell::Bool(false), Cell::Bool(true), Cell::Bool(false)]);
        assert_eq!(col(1), [Cell::Real(1.0), Cell::Real(2.0), Cell::Real(4.0)]);
    }

    #[test]
    fn a_real_typed_net_is_held_not_interpolated() {
        // LR-05 AC 4: a net of type f64 holds its last value; only a node or a flow is
        // interpolated
        let ty = ValueType::F64;
        let mut recorder = with_net(&ty, 0.5, 1.0);
        recorder.step(0.0, &reading(0.0, &[Cell::Real(1.0)]));
        recorder.change("T.q", 1.0, &bytes(&ty, &Cell::Real(5.0)));
        recorder.step(1.0, &reading(0.0, &[Cell::Real(5.0)]));
        let table = recorder.finish().expect("table");
        assert_eq!(q(&table), [Cell::Real(1.0), Cell::Real(1.0), Cell::Real(5.0)]);
    }

    #[test]
    fn a_zero_net_value_is_a_value() {
        // LR-05 with the explicit model: a net's bytes are its value, so all-zero bytes are `false`,
        // at the start and after a change
        let ty = ValueType::Bool;
        let mut recorder = with_net(&ty, 0.5, 1.0);
        recorder.step(0.0, &reading(0.0, &[Cell::Bool(false)]));
        recorder.change("T.q", 0.5, &bytes(&ty, &Cell::Bool(true)));
        recorder.change("T.q", 0.75, &bytes(&ty, &Cell::Bool(false)));
        recorder.step(1.0, &reading(0.0, &[Cell::Bool(false)]));
        let table = recorder.finish().expect("table");
        assert_eq!(q(&table), [Cell::Bool(false), Cell::Bool(true), Cell::Bool(false)]);
    }

    #[test]
    fn a_net_that_changed_unobserved_fails_naming_it() {
        // LR-06 AC 4: the reader says true at the step and no change reached the recorder: the
        // run fails naming the net and the time, never taking the reader's value
        let ty = ValueType::Bool;
        for grid in [true, false] {
            let mut recorder = Recorder::new(vec![unknown(0), net(0, ty.clone())], vec!["v".into(), "T.q".into()], 0.0);
            if grid {
                recorder = recorder.with_grid(0.5, 1.0);
            }
            recorder.step(0.0, &reading(0.0, &[Cell::Bool(false)]));
            recorder.step(1.0, &reading(0.0, &[Cell::Bool(true)]));
            match recorder.finish() {
                Err(Failure::Backend(message)) => assert!(message.contains("`T.q`") && message.contains("at 1"), "{message}"),
                other => panic!("a failure naming the net, found {other:?}"),
            }
        }
    }

    #[test]
    fn a_resumed_run_seeds_its_nets_from_the_reader() {
        // contract.md point 2: a run restored from a state has no change for the nets it holds;
        // the first step is what the reader says, and the changes after it add up from there
        let ty = ValueType::Bool;
        let mut recorder = with_net(&ty, 0.5, 1.0);
        recorder.step(0.0, &reading(0.0, &[Cell::Bool(true)]));
        recorder.change("T.q", 0.75, &bytes(&ty, &Cell::Bool(false)));
        recorder.step(1.0, &reading(0.0, &[Cell::Bool(false)]));
        let table = recorder.finish().expect("table");
        assert_eq!(q(&table), [Cell::Bool(true), Cell::Bool(true), Cell::Bool(false)]);
    }
}
