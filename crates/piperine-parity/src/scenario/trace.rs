//! [`Trace`]: one real column of a run measured over a window.

use piperine_api::{Cell, Table};

use super::column::NamedColumn;

/// A measured row: its time and its value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// The time of the row, in seconds.
    pub time: f64,
    /// The value the measurement gives at that row.
    pub value: f64,
}

/// The direction in which a trace reaches a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossing {
    /// The value is at or above the level.
    Rising,
    /// The value is at or below the level.
    Falling,
}

/// The rows of one real column with the time of each, optionally cut to a window. Every
/// measurement of a scenario (SPEC `examples/buck/SPEC.md` §10.1) is one method of this type, so
/// a pass means the SPEC's criterion and not a quirk of a helper written beside a test.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    /// The full label of the column, for the messages of a failure.
    label: String,
    /// The window the rows were cut to; none for the whole run.
    window: Option<(f64, f64)>,
    /// The rows, in time order.
    rows: Vec<Sample>,
}

impl Trace {
    /// Every row of the column `name` of `table` with its time (column 0). A cell that is not a
    /// real number panics naming the row: a scenario column is always analog.
    pub fn of(table: &Table, name: &str) -> Trace {
        let column = NamedColumn::find(table, name);
        let rows = table
            .rows
            .iter()
            .enumerate()
            .map(|(row, cells)| match (&cells[0], &cells[column.index()]) {
                (Cell::Real(time), Cell::Real(value)) => Sample { time: *time, value: *value },
                other => panic!("row {row} of `{}` is not a real number at a real time: {other:?}", column.label()),
            })
            .collect();
        Trace { label: column.label().to_string(), window: None, rows }
    }

    /// The same rows with every value multiplied by `factor` (the sense chain's 0.1 V/A turns
    /// volts into amperes).
    pub fn scaled(mut self, factor: f64) -> Trace {
        for row in &mut self.rows {
            row.value *= factor;
        }
        self
    }

    /// The rows with `from <= time < to`; the window is kept for the messages of a failure.
    pub fn between(&self, from: f64, to: f64) -> Trace {
        let rows = self.rows.iter().copied().filter(|row| row.time >= from && row.time < to).collect();
        Trace { label: self.label.clone(), window: Some((from, to)), rows }
    }

    /// The row with the largest value.
    pub fn maximum(&self) -> Sample {
        self.rows().iter().copied().reduce(|best, row| if row.value > best.value { row } else { best }).expect("rows() is never empty")
    }

    /// The row with the smallest value.
    pub fn minimum(&self) -> Sample {
        self.rows().iter().copied().reduce(|best, row| if row.value < best.value { row } else { best }).expect("rows() is never empty")
    }

    /// The row whose value is farthest from `nominal`; its `value` is that absolute difference
    /// (SPEC §10.1's deviation).
    pub fn deviation_from(&self, nominal: f64) -> Sample {
        self.rows()
            .iter()
            .map(|row| Sample { time: row.time, value: (row.value - nominal).abs() })
            .reduce(|best, row| if row.value > best.value { row } else { best })
            .expect("rows() is never empty")
    }

    /// The arithmetic mean of the values.
    pub fn mean(&self) -> f64 {
        self.rows().iter().map(|row| row.value).sum::<f64>() / self.rows.len() as f64
    }

    /// The largest value minus the smallest.
    pub fn peak_to_peak(&self) -> f64 {
        self.maximum().value - self.minimum().value
    }

    /// The time of the first row from which every later row is inside `[low, high]` (SPEC
    /// §10.1's settling time), or none when the last row is outside it. The scan runs backwards
    /// from the end so one pass finds the start of the final inside run.
    pub fn settled_into(&self, low: f64, high: f64) -> Option<f64> {
        let outside = |row: &Sample| row.value < low || row.value > high;
        let rows = self.rows();
        if outside(rows.last().expect("rows() is never empty")) {
            return None;
        }
        let first_inside = rows.iter().rposition(outside).map_or(0, |last_outside| last_outside + 1);
        Some(rows[first_inside].time)
    }

    /// The time of the first row at or after `after` whose value is at or past `level` in the
    /// direction `crossing`; none when no row is.
    pub fn first_crossing(&self, level: f64, crossing: Crossing, after: f64) -> Option<f64> {
        let reached = |value: f64| match crossing {
            Crossing::Rising => value >= level,
            Crossing::Falling => value <= level,
        };
        self.rows.iter().find(|row| row.time >= after && reached(row.value)).map(|row| row.time)
    }

    /// The row that rises most above its predecessor; its `value` is that rise (negative when every
    /// row falls). A decay is judged against the solver's own voltage resolution with this, since
    /// two rows of an accepted-step interpolation can differ by less than the solver tolerance in
    /// either direction.
    pub fn largest_rise(&self) -> Sample {
        self.rows()
            .windows(2)
            .map(|pair| Sample { time: pair[1].time, value: pair[1].value - pair[0].value })
            .reduce(|best, row| if row.value > best.value { row } else { best })
            .unwrap_or_else(|| panic!("`{}` has fewer than two rows to compare", self.label))
    }

    /// The time constant of an exponential decay through the first and the last row:
    /// `(t_last - t_first) / ln(v_first / v_last)`.
    pub fn decay_time_constant(&self) -> f64 {
        let (first, last) = (self.rows()[0], *self.rows().last().expect("rows() is never empty"));
        (last.time - first.time) / (first.value / last.value).ln()
    }

    /// The rows; an empty window panics naming the label and the window, so a scenario never
    /// judges nothing.
    fn rows(&self) -> &[Sample] {
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
        /// A trace of `buck.vout` over `(time, value)` rows.
        fn trace(rows: &[(f64, f64)]) -> Trace {
            let table = Table {
                columns: vec!["time".into(), "buck.vout".into()],
                rows: rows.iter().map(|&(time, value)| vec![Cell::Real(time), Cell::Real(value)]).collect(),
            };
            Trace::of(&table, "vout")
        }
    }

    #[test]
    fn settled_into_is_the_first_row_from_which_every_row_stays_inside() {
        // band [0.99, 1.01]: row 1 (1.005) is inside, row 2 (1.02) leaves it again, so the
        // settling starts at row 3 (t = 3), where every later row is inside
        let trace = Tables::trace(&[(0.0, 0.5), (1.0, 1.005), (2.0, 1.02), (3.0, 1.0), (4.0, 0.995)]);
        assert_eq!(trace.settled_into(0.99, 1.01), Some(3.0));
        // the last row (1.2) is outside: never settled
        let escaped = Tables::trace(&[(0.0, 1.0), (1.0, 1.0), (2.0, 1.2)]);
        assert_eq!(escaped.settled_into(0.99, 1.01), None);
        // all rows inside: the first row
        let inside = Tables::trace(&[(5.0, 1.0), (6.0, 1.0)]);
        assert_eq!(inside.settled_into(0.99, 1.01), Some(5.0));
    }

    #[test]
    fn deviation_is_the_largest_absolute_difference_with_its_time() {
        // from 3.3: |3.25 - 3.3| = 0.05, |3.5 - 3.3| = 0.2 (t = 2), |3.1 - 3.3| = 0.2 (t = 3):
        // the first row of the largest difference is t = 2
        let trace = Tables::trace(&[(1.0, 3.25), (2.0, 3.5), (3.0, 3.1)]);
        let worst = trace.deviation_from(3.3);
        assert_eq!(worst.time, 2.0);
        assert!((worst.value - 0.2).abs() < 1e-12, "{worst:?}");
    }

    #[test]
    fn peak_to_peak_is_the_largest_minus_the_smallest() {
        // largest 4.0, smallest -1.5: 5.5
        let trace = Tables::trace(&[(0.0, 1.0), (1.0, 4.0), (2.0, -1.5), (3.0, 0.0)]);
        assert_eq!(trace.peak_to_peak(), 5.5);
        assert_eq!((trace.maximum().time, trace.minimum().time), (1.0, 2.0));
    }

    #[test]
    fn mean_is_the_arithmetic_mean_of_the_rows() {
        // (1 + 2 + 6) / 3 = 3, whatever the spacing of the times
        let trace = Tables::trace(&[(0.0, 1.0), (0.1, 2.0), (5.0, 6.0)]);
        assert_eq!(trace.mean(), 3.0);
    }

    #[test]
    fn first_crossing_is_the_first_row_at_or_past_the_level_after_a_time() {
        let trace = Tables::trace(&[(0.0, 9.0), (1.0, 8.0), (2.0, 3.0), (3.0, 8.5), (4.0, 7.0)]);
        // falling to 8.0: the row at t = 1 is exactly 8.0 and "at the level" counts
        assert_eq!(trace.first_crossing(8.0, Crossing::Falling, 0.0), Some(1.0));
        // rising to 8.5 after t = 1.5: first reached at t = 3
        assert_eq!(trace.first_crossing(8.5, Crossing::Rising, 1.5), Some(3.0));
        // the 9.0 of t = 0 does not count after t = 0.5, and nothing rises to it again
        assert_eq!(trace.first_crossing(9.0, Crossing::Rising, 0.5), None);
    }

    #[test]
    fn decay_time_constant_recovers_an_exponential() {
        // v = 3.3 exp(-t / 1.452e-3) sampled every 1e-4: strictly decreasing, tau recovered
        let tau = 1.452e-3;
        let rows: Vec<(f64, f64)> = (0..=20).map(|k| (k as f64 * 1e-4, 3.3 * (-(k as f64) * 1e-4 / tau).exp())).collect();
        let decay = Tables::trace(&rows);
        assert!(decay.largest_rise().value < 0.0);
        assert!((decay.decay_time_constant() - tau).abs() / tau < 1e-9, "{}", decay.decay_time_constant());
    }

    #[test]
    fn largest_rise_is_the_biggest_step_up_and_negative_on_a_decay() {
        // steps: -1, +0.5, -2: the rise is 0.5 at time 2; a strict decay gives its smallest fall
        let wobble = Tables::trace(&[(0.0, 3.0), (1.0, 2.0), (2.0, 2.5), (3.0, 0.5)]);
        assert_eq!(wobble.largest_rise(), Sample { time: 2.0, value: 0.5 });
        let decay = Tables::trace(&[(0.0, 3.0), (1.0, 2.0), (2.0, 0.5)]);
        assert_eq!(decay.largest_rise(), Sample { time: 1.0, value: -1.0 });
    }

    #[test]
    fn between_keeps_half_open_windows_and_an_empty_window_panics_naming_it() {
        let trace = Tables::trace(&[(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)]);
        assert_eq!(trace.between(1.0, 2.0).mean(), 2.0);
        let payload = catch_unwind(AssertUnwindSafe(|| trace.between(5.0, 6.0).mean())).expect_err("an empty window must panic");
        let text = payload.downcast_ref::<String>().expect("a formatted panic message");
        assert!(text.contains("buck.vout") && text.contains("5e0") && text.contains("6e0"), "{text}");
    }

    #[test]
    fn scaled_multiplies_every_value() {
        let trace = Tables::trace(&[(0.0, 0.5), (1.0, -0.3)]).scaled(10.0);
        assert_eq!((trace.maximum().value, trace.minimum().value), (5.0, -3.0));
    }
}
