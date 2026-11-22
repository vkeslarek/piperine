//! `Plot`: the geometry of a plot, as numbers: what is drawn where, before any SVG text exists
//! (CLI-22). A value maps linearly (or by its logarithm on a log axis) from the axis range onto
//! the plot area (AC 3); each real signal is a [`Series`] of runs of points already in drawing
//! coordinates, broken at a value that is not a finite real (AC 9); each discrete signal is a [`Lane`] under the
//! plot area, never a series (AC 5); an `ac` run has two panels, dB over degrees, with a log
//! frequency axis for `per_decade` (AC 6); an operating point has nothing to plot (AC 8).
//!
//! The fixed numbers: 800 by 480 for one panel and 800 by 640 for two; margins of 70 (left),
//! 20 (right), 20 (top) and 50 (bottom, where the x axis is labelled); a lane is 28 high; the
//! y range is the data's range widened to the enclosing ticks; the x range is the data's.

use crate::failure::Failure;
use crate::request::{Analysis, Grid};
use crate::table::{Cell, Table};

const WIDTH: f64 = 800.0;
const HEIGHT_ONE: f64 = 480.0;
const HEIGHT_TWO: f64 = 640.0;
const LEFT: f64 = 70.0;
const RIGHT: f64 = 20.0;
const TOP: f64 = 20.0;
const BOTTOM: f64 = 50.0;
/// The gap between the two panels of an `ac` plot.
const GAP: f64 = 40.0;
const LANE: f64 = 28.0;
/// The smallest plot area a panel keeps; more lanes than fit under it is a failure.
const MIN_AREA: f64 = 100.0;

/// A rectangle in drawing coordinates, `y` growing downward.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// A tick mark: the value it stands for and its label.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub value: f64,
    pub label: String,
}

/// An axis: its title, its range, whether it is logarithmic, and its ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct Axis {
    pub title: String,
    pub min: f64,
    pub max: f64,
    pub log: bool,
    pub ticks: Vec<Tick>,
}

/// A real signal: one run of points per stretch between values that are no finite real, in drawing coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub label: String,
    pub runs: Vec<Vec<(f64, f64)>>,
}

/// A discrete signal: its value at each abscissa of the table, in the lane under the plot.
#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub label: String,
    pub area: Rect,
    pub steps: Vec<(f64, Cell)>,
}

/// One plot area with its axes and its series.
#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    pub area: Rect,
    pub x: Axis,
    pub y: Axis,
    pub series: Vec<Series>,
}

/// A whole plot.
#[derive(Debug, Clone, PartialEq)]
pub struct Plot {
    pub width: f64,
    pub height: f64,
    pub panels: Vec<Panel>,
    pub lanes: Vec<Lane>,
}

impl Axis {
    /// Where `value` falls between `from` (the coordinate of `min`) and `to` (that of `max`):
    /// linear in the value, or in its logarithm on a log axis.
    pub fn place(&self, value: f64, from: f64, to: f64) -> f64 {
        let fraction = if self.log { (value.log10() - self.min.log10()) / (self.max.log10() - self.min.log10()) } else { (value - self.min) / (self.max - self.min) };
        from + (to - from) * fraction
    }

    /// The ticks inside `[min, max]`: multiples of 1, 2 or 5 times a power of ten, the smallest
    /// step that leaves at most 10 of them (so at least 4 whenever the range spans four steps).
    pub fn ticks(min: f64, max: f64) -> Vec<Tick> {
        let (mantissa, exponent) = Axis::step(min, max);
        Axis::multiples(min, max, mantissa, exponent)
    }

    /// The decades inside `[min, max]` of a log axis; when fewer than two fit, the 1, 2 and 5
    /// times a power of ten that do.
    pub fn decades(min: f64, max: f64) -> Vec<Tick> {
        let first = (min.log10() - 1e-9).ceil() as i32;
        let last = (max.log10() + 1e-9).floor() as i32;
        let decades: Vec<Tick> = (first..=last).map(|k| Tick { value: Axis::power(1, k), label: Axis::label(1, k) }).collect();
        if decades.len() >= 2 { decades } else { Axis::ticks(min, max) }
    }

    /// The step `mantissa * 10^exponent` of the ticks of `[min, max]`: the smallest of 1, 2, 5
    /// times a power of ten that leaves at most 10 multiples inside.
    fn step(min: f64, max: f64) -> (i64, i32) {
        let start = (max - min).log10().floor() as i32 - 2;
        for exponent in start..start + 6 {
            for mantissa in [1, 2, 5] {
                if Axis::count(min, max, mantissa, exponent) <= 10 {
                    return (mantissa, exponent);
                }
            }
        }
        (1, start + 6)
    }

    /// The multiples `k * mantissa * 10^exponent` inside `[min, max]`, with a little slack for
    /// the rounding of the ends.
    fn multiples(min: f64, max: f64, mantissa: i64, exponent: i32) -> Vec<Tick> {
        let (first, last) = Axis::range(min, max, mantissa, exponent);
        (first..=last).map(|k| Tick { value: Axis::power(k * mantissa, exponent), label: Axis::label(k * mantissa, exponent) }).collect()
    }

    fn count(min: f64, max: f64, mantissa: i64, exponent: i32) -> i64 {
        let (first, last) = Axis::range(min, max, mantissa, exponent);
        last - first + 1
    }

    /// The first and last multiples of the step inside `[min, max]`.
    fn range(min: f64, max: f64, mantissa: i64, exponent: i32) -> (i64, i64) {
        let step = Axis::power(mantissa, exponent);
        ((min / step - 1e-9).ceil() as i64, (max / step + 1e-9).floor() as i64)
    }

    /// `n * 10^exponent`, with the one rounding a division gives for a negative exponent.
    fn power(n: i64, exponent: i32) -> f64 {
        if exponent >= 0 { n as f64 * 10f64.powi(exponent) } else { n as f64 / 10f64.powi(-exponent) }
    }

    /// `n * 10^exponent` as a short decimal, or `<n>e<exponent>` when that is far from 1.
    fn label(n: i64, exponent: i32) -> String {
        if (0..=4).contains(&exponent) {
            return (n * 10i64.pow(exponent as u32)).to_string();
        }
        if (-6..0).contains(&exponent) {
            let digits = n.abs().to_string();
            let places = (-exponent) as usize;
            let padded = format!("{digits:0>width$}", width = places + 1);
            let (whole, fraction) = padded.split_at(padded.len() - places);
            let sign = if n < 0 { "-" } else { "" };
            let fraction = fraction.trim_end_matches('0');
            return if fraction.is_empty() { format!("{sign}{whole}") } else { format!("{sign}{whole}.{fraction}") };
        }
        format!("{n}e{exponent}")
    }

    /// A y axis over `[min, max]` widened to the enclosing ticks.
    fn widened(title: &str, min: f64, max: f64) -> Axis {
        let (min, max) = if min == max { (min - 0.5 * min.abs().max(1.0), max + 0.5 * max.abs().max(1.0)) } else { (min, max) };
        let (mantissa, exponent) = Axis::step(min, max);
        let step = Axis::power(mantissa, exponent);
        let (low, high) = ((min / step + 1e-9).floor() as i64, (max / step - 1e-9).ceil() as i64);
        let (min, max) = (Axis::power(low * mantissa, exponent), Axis::power(high * mantissa, exponent));
        Axis { title: title.into(), min, max, log: false, ticks: Axis::multiples(min, max, mantissa, exponent) }
    }

    /// An x axis over `[min, max]` as it is.
    fn spanning(title: &str, min: f64, max: f64, log: bool) -> Axis {
        let (min, max) = if min == max && !log { (min - 0.5, max + 0.5) } else { (min, max) };
        let ticks = if log { Axis::decades(min, max) } else { Axis::ticks(min, max) };
        Axis { title: title.into(), min, max, log, ticks }
    }
}

/// How a column of a table is drawn.
enum Kind {
    Real,
    Discrete,
}

impl Plot {
    /// The plot of `table`, the result of `analysis`: a `transient` or `dc` table is one panel
    /// with the first column as the x axis; an `ac` table is the magnitude in dB over the phase
    /// in degrees. `P0010` for an operating point, which has nothing to plot.
    pub fn of(table: &Table, analysis: &Analysis) -> Result<Plot, Failure> {
        // a composite column is one series or lane per numeric leaf
        let table = &table.expanded(false);
        match analysis {
            Analysis::Op => Err(Failure::Host { code: "P0010", message: "an operating point has nothing to plot".into() }),
            Analysis::Ac { grid, .. } => Plot::frequency(table, matches!(grid, Grid::PerDecade(_))),
            Analysis::Dc { .. } | Analysis::Transient { .. } | Analysis::Pss { .. } => Plot::against_first_column(table),
            Analysis::Tf { .. } | Analysis::Pz { .. } | Analysis::Sens { .. } => {
                Err(Failure::Host { code: "P0010", message: format!("a `{}` run has nothing to plot: its result is a table of values, not a curve", analysis.name()) })
            }
        }
    }

    fn kind(table: &Table, column: usize) -> Kind {
        let discrete = table.rows.iter().any(|r| matches!(r[column], Cell::Bool(_) | Cell::Int(_)));
        if discrete { Kind::Discrete } else { Kind::Real }
    }

    /// The first column as the abscissa of every other column.
    fn abscissae(table: &Table) -> Result<Vec<f64>, Failure> {
        table
            .rows
            .iter()
            .map(|r| match r.first() {
                Some(Cell::Real(x)) => Ok(*x),
                other => Err(Failure::Backend(format!("a plot needs a real first column, found {other:?}"))),
            })
            .collect()
    }

    fn range(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
        values.fold(None, |range, v| match range {
            None => Some((v, v)),
            Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
        })
    }

    fn against_first_column(table: &Table) -> Result<Plot, Failure> {
        let xs = Plot::abscissae(table)?;
        let reals: Vec<usize> = (1..table.columns.len()).filter(|c| matches!(Plot::kind(table, *c), Kind::Real)).collect();
        let discretes: Vec<usize> = (1..table.columns.len()).filter(|c| matches!(Plot::kind(table, *c), Kind::Discrete)).collect();
        let (x_min, x_max) = Plot::range(xs.iter().copied()).ok_or_else(|| Failure::Backend("the table has no row to plot".into()))?;
        let y_range = Plot::range(reals.iter().flat_map(|c| table.rows.iter().filter_map(move |r| if let Cell::Real(v) = r[*c] { Some(v) } else { None })).filter(|v| v.is_finite()))
            .unwrap_or((0.0, 1.0));
        let lanes_height = LANE * discretes.len() as f64;
        let area = Rect { x: LEFT, y: TOP, w: WIDTH - LEFT - RIGHT, h: HEIGHT_ONE - TOP - BOTTOM - lanes_height };
        if area.h < MIN_AREA {
            return Err(Failure::Host { code: "P0010", message: format!("a plot draws at most {} discrete signals, found {}", ((HEIGHT_ONE - TOP - BOTTOM - MIN_AREA) / LANE) as usize, discretes.len()) });
        }
        let x = Axis::spanning(&table.columns[0], x_min, x_max, false);
        let y = Axis::widened("value", y_range.0, y_range.1);
        let series = reals.iter().map(|c| Plot::series(&table.columns[*c], &xs, table, *c, &x, &y, area, |v| Some(v))).collect();
        let lanes = discretes
            .iter()
            .enumerate()
            .map(|(i, c)| Lane {
                label: table.columns[*c].clone(),
                area: Rect { x: area.x, y: area.y + area.h + LANE * i as f64, w: area.w, h: LANE },
                steps: xs.iter().zip(&table.rows).map(|(x, r)| (*x, r[*c].clone())).collect(),
            })
            .collect();
        Ok(Plot { width: WIDTH, height: HEIGHT_ONE, panels: vec![Panel { area, x, y, series }], lanes })
    }

    /// The runs of the real column `column`, mapped by `value` (which may refuse a point, as a
    /// zero magnitude has no decibels) and placed by the axes.
    fn series(label: &str, xs: &[f64], table: &Table, column: usize, x: &Axis, y: &Axis, area: Rect, value: impl Fn(f64) -> Option<f64>) -> Series {
        let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut open = false;
        for (at, row) in xs.iter().zip(&table.rows) {
            match row[column] {
                Cell::Real(v) if value(v).is_some_and(f64::is_finite) => {
                    let point = (x.place(*at, area.x, area.x + area.w), y.place(value(v).unwrap_or(v), area.y + area.h, area.y));
                    if open {
                        if let Some(run) = runs.last_mut() {
                            run.push(point);
                        }
                    } else {
                        runs.push(vec![point]);
                        open = true;
                    }
                }
                _ => open = false,
            }
        }
        Series { label: label.into(), runs }
    }

    /// The magnitude in dB over the phase in degrees: the columns after `freq` come in pairs.
    fn frequency(table: &Table, log: bool) -> Result<Plot, Failure> {
        let xs = Plot::abscissae(table)?;
        let (x_min, x_max) = Plot::range(xs.iter().copied()).ok_or_else(|| Failure::Backend("the table has no row to plot".into()))?;
        let signals: Vec<(String, usize)> = (1..table.columns.len())
            .step_by(2)
            .map(|c| (table.columns[c].strip_prefix("mag(").and_then(|n| n.strip_suffix(')')).unwrap_or(&table.columns[c]).to_string(), c))
            .collect();
        let decibels = |m: f64| (m > 0.0).then(|| 20.0 * m.log10());
        let range = |column: &dyn Fn(usize) -> usize, map: &dyn Fn(f64) -> Option<f64>| {
            Plot::range(signals.iter().flat_map(|(_, c)| table.rows.iter().filter_map(move |r| if let Cell::Real(v) = r[column(*c)] { map(v) } else { None })).filter(|v| v.is_finite())).unwrap_or((0.0, 1.0))
        };
        let (db_range, phase_range) = (range(&|c| c, &decibels), range(&|c| c + 1, &|p| Some(p)));
        let height = (HEIGHT_TWO - TOP - BOTTOM - GAP) / 2.0;
        let areas = [Rect { x: LEFT, y: TOP, w: WIDTH - LEFT - RIGHT, h: height }, Rect { x: LEFT, y: TOP + height + GAP, w: WIDTH - LEFT - RIGHT, h: height }];
        let x = Axis::spanning(&table.columns[0], x_min, x_max, log);
        let panels = [("magnitude (dB)", db_range, 0usize), ("phase (degrees)", phase_range, 1)]
            .into_iter()
            .map(|(title, (lo, hi), offset)| {
                let y = Axis::widened(title, lo, hi);
                let area = areas[offset];
                let series = signals
                    .iter()
                    .map(|(label, c)| {
                        let map = |v: f64| if offset == 0 { decibels(v) } else { Some(v) };
                        Plot::series(label, &xs, table, c + offset, &x, &y, area, map)
                    })
                    .collect();
                Panel { area, x: x.clone(), y, series }
            })
            .collect();
        Ok(Plot { width: WIDTH, height: HEIGHT_TWO, panels, lanes: Vec::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn real_table(columns: &[&str], rows: &[&[Cell]]) -> Table {
        Table { columns: columns.iter().map(|c| c.to_string()).collect(), rows: rows.iter().map(|r| r.to_vec()).collect() }
    }

    fn transient() -> Analysis {
        Analysis::Transient { stop: 4.0, max_step: None, print_step: None, from: None }
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9
    }

    #[test]
    fn a_value_maps_linearly_onto_the_plot_area() {
        // CLI-22 AC 3: one panel, 800 x 480, margins 70 / 20 / 20 / 50, so the plot area is
        // x = 70, y = 20, w = 800 - 70 - 20 = 710, h = 480 - 20 - 50 = 410 (right edge 780,
        // bottom edge 430). Time 0..4 s maps onto x 70..780, v 0..8 onto y 430..20.
        let r = |t: f64, v: f64| vec![Cell::Real(t), Cell::Real(v)];
        let table = Table { columns: vec!["time".into(), "v".into()], rows: vec![r(0.0, 0.0), r(1.0, 2.0), r(2.0, 4.0), r(3.0, 6.0), r(4.0, 8.0)] };
        let plot = Plot::of(&table, &transient()).expect("a plot");
        assert_eq!((plot.width, plot.height), (800.0, 480.0));
        let panel = &plot.panels[0];
        assert_eq!(panel.area, Rect { x: 70.0, y: 20.0, w: 710.0, h: 410.0 });
        // the y range 0..8 has ticks of step 1 (9 of them), so it is already the axis range
        assert_eq!((panel.y.min, panel.y.max), (0.0, 8.0));
        assert_eq!((panel.x.min, panel.x.max), (0.0, 4.0));
        // x = 70 + 710 * t / 4 and y = 430 - 410 * v / 8, point by point:
        //   (0, 0) -> (70, 430)        (1, 2) -> (247.5, 327.5)     (2, 4) -> (425, 225)
        //   (3, 6) -> (602.5, 122.5)   (4, 8) -> (780, 20)
        let want = [(70.0, 430.0), (247.5, 327.5), (425.0, 225.0), (602.5, 122.5), (780.0, 20.0)];
        let run = &panel.series[0].runs[0];
        assert_eq!(run.len(), 5);
        for (got, want) in run.iter().zip(want) {
            assert!(near(got.0, want.0) && near(got.1, want.1), "{got:?} against {want:?}");
        }
        assert_eq!(panel.series[0].label, "v");
        assert_eq!(panel.x.title, "time");
        // the mapping itself: the middle of a linear axis, and of a log axis (1e5 is half way,
        // in decades, between 1e3 and 1e7: 70 + 710 * 2 / 4 = 425)
        assert!(near(panel.y.place(4.0, 430.0, 20.0), 225.0));
        let log = Axis { title: "freq".into(), min: 1e3, max: 1e7, log: true, ticks: vec![] };
        assert!(near(log.place(1e5, 70.0, 780.0), 425.0));
    }

    #[test]
    fn ticks_are_1_2_5_times_a_power_of_ten() {
        // by hand: 0..1 with step 0.1 would be 11 ticks, so the step is 0.2: 0, 0.2 .. 1 (6);
        // 0..8 with step 1 is 9 ticks; 0..100 with step 10 would be 11, so the step is 20:
        // 0, 20 .. 100 (6); 0.003..0.0071: step 0.0002 is 21 ticks, 0.0005 is 9 (0.003 .. 0.007)
        let labels = |min, max| Axis::ticks(min, max).into_iter().map(|t| t.label).collect::<Vec<_>>();
        assert_eq!(labels(0.0, 1.0), ["0", "0.2", "0.4", "0.6", "0.8", "1"]);
        assert_eq!(labels(0.0, 8.0), ["0", "1", "2", "3", "4", "5", "6", "7", "8"]);
        assert_eq!(labels(0.0, 100.0), ["0", "20", "40", "60", "80", "100"]);
        assert_eq!(labels(0.003, 0.0071), ["0.003", "0.0035", "0.004", "0.0045", "0.005", "0.0055", "0.006", "0.0065", "0.007"]);
        let values: Vec<f64> = Axis::ticks(0.0, 1.0).into_iter().map(|t| t.value).collect();
        assert_eq!(values, [0.0, 0.2, 0.4, 0.6, 0.8, 1.0], "a tick value is the nearest f64 of its label");
        // every step of every range is 1, 2 or 5 times a power of ten, and there are 4 to 10 ticks
        for (min, max) in [(0.0, 1.0), (0.0, 5e-6), (-1.0, 0.8), (1e3, 9e3), (0.0, 12.0), (-40.0, 0.0), (0.0, 7e9)] {
            let ticks = Axis::ticks(min, max);
            assert!((4..=10).contains(&ticks.len()), "{min}..{max}: {} ticks", ticks.len());
            let step = ticks[1].value - ticks[0].value;
            let mantissa = step / 10f64.powf(step.log10().floor());
            assert!([1.0, 2.0, 5.0].iter().any(|m| (mantissa - m).abs() < 1e-6), "{min}..{max}: step {step}");
        }
        // a log axis has its decades
        let decades: Vec<String> = Axis::decades(1e3, 1e7).into_iter().map(|t| t.label).collect();
        assert_eq!(decades, ["1000", "10000", "1e5", "1e6", "1e7"]);
    }

    #[test]
    fn a_discrete_signal_gets_a_lane_and_no_series() {
        // CLI-22 AC 5: the real column `v` is a series, the bool column `b` a lane of 28 under
        // the plot area, which gives up that height: 410 - 28 = 382, so the lane is at
        // y = 20 + 382 = 402, from x = 70, 710 wide
        let table = real_table(
            &["time", "v", "b"],
            &[&[Cell::Real(0.0), Cell::Real(0.0), Cell::Bool(false)], &[Cell::Real(1.0), Cell::Real(1.0), Cell::Bool(true)], &[Cell::Real(2.0), Cell::Real(2.0), Cell::Bool(false)]],
        );
        let plot = Plot::of(&table, &transient()).expect("a plot");
        assert_eq!(plot.panels[0].series.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(), ["v"]);
        assert_eq!(plot.panels[0].area.h, 382.0);
        assert_eq!(plot.lanes.len(), 1);
        assert_eq!(plot.lanes[0].label, "b");
        assert_eq!(plot.lanes[0].area, Rect { x: 70.0, y: 402.0, w: 710.0, h: 28.0 });
        assert_eq!(plot.lanes[0].steps, [(0.0, Cell::Bool(false)), (1.0, Cell::Bool(true)), (2.0, Cell::Bool(false))]);
    }

    #[test]
    fn an_ac_plot_has_a_db_panel_over_a_degree_panel_and_a_log_axis() {
        // CLI-22 AC 6: 800 x 640; the panels are (640 - 20 - 50 - 40) / 2 = 265 high, the first
        // at y = 20 (bottom 285), the second at y = 20 + 265 + 40 = 325 (bottom 590)
        let row = |f: f64, m: f64, p: f64| vec![Cell::Real(f), Cell::Real(m), Cell::Real(p)];
        let corner = std::f64::consts::FRAC_1_SQRT_2;
        let table = Table { columns: vec!["freq".into(), "mag(AcRc.out)".into(), "phase(AcRc.out)".into()], rows: vec![row(1e3, 1.0, 0.0), row(1e5, corner, -45.0), row(1e7, 0.01, -90.0)] };
        let ac = Analysis::Ac { from: 1e3, to: 1e7, grid: Grid::PerDecade(1) };
        let plot = Plot::of(&table, &ac).expect("a plot");
        assert_eq!((plot.width, plot.height), (800.0, 640.0));
        assert_eq!(plot.panels.len(), 2);
        assert_eq!(plot.panels[0].area, Rect { x: 70.0, y: 20.0, w: 710.0, h: 265.0 });
        assert_eq!(plot.panels[1].area, Rect { x: 70.0, y: 325.0, w: 710.0, h: 265.0 });
        assert!(plot.panels[0].x.log && plot.panels[1].x.log);
        assert_eq!(plot.panels[0].x.title, "freq");
        // dB: 20 log10(1) = 0, 20 log10(0.7071..) = -3.0103, 20 log10(0.01) = -40; the range
        // -40..0 has ticks of step 5 (9; step 2 would be 21), so the axis is -40..0
        assert_eq!((plot.panels[0].y.min, plot.panels[0].y.max), (-40.0, 0.0));
        assert_eq!(plot.panels[0].y.title, "magnitude (dB)");
        let db = &plot.panels[0].series[0];
        assert_eq!(db.label, "AcRc.out");
        // x = 70 + 710 * log10(f / 1e3) / 4, y = 285 - 265 * (dB + 40) / 40:
        //   1e3 -> (70, 20)   1e5 -> (425, 285 - 265 * 36.9897 / 40 = 39.9...)   1e7 -> (780, 285)
        let want = [(70.0, 20.0), (425.0, 285.0 - 265.0 * (40.0 + 20.0 * corner.log10()) / 40.0), (780.0, 285.0)];
        for (got, want) in db.runs[0].iter().zip(want) {
            assert!(near(got.0, want.0) && near(got.1, want.1), "{got:?} against {want:?}");
        }
        assert!((want[1].1 - 40.0).abs() < 0.1, "{:?}", want[1]);
        // degrees: -90..0 has ticks of step 10 (10; step 5 would be 19); y = 590 - 265 * (p + 90) / 90,
        // so -45 is at 590 - 132.5 = 457.5
        assert_eq!((plot.panels[1].y.min, plot.panels[1].y.max), (-90.0, 0.0));
        assert_eq!(plot.panels[1].y.title, "phase (degrees)");
        assert!(near(plot.panels[1].series[0].runs[0][1].1, 457.5));
        // a zero magnitude has no decibels: the path breaks there
        let broken = Table { columns: table.columns.clone(), rows: vec![row(1e3, 1.0, 0.0), row(1e5, 0.0, -45.0), row(1e7, 1.0, -90.0)] };
        let plot = Plot::of(&broken, &ac).expect("a plot");
        assert_eq!(plot.panels[0].series[0].runs.len(), 2);
        // a linear grid has a linear axis
        let linear = Analysis::Ac { from: 1e3, to: 1e7, grid: Grid::Points(3) };
        assert!(!Plot::of(&table, &linear).expect("a plot").panels[0].x.log);
    }

    #[test]
    fn an_operating_point_has_nothing_to_plot() {
        // CLI-22 AC 8: P0010 saying so
        let table = Table { columns: vec!["signal".into(), "value".into()], rows: vec![vec![Cell::Text("a".into()), Cell::Real(1.0)]] };
        match Plot::of(&table, &Analysis::Op) {
            Err(Failure::Host { code: "P0010", message }) => assert!(message.contains("nothing to plot"), "{message}"),
            other => panic!("expected P0010, found {other:?}"),
        }
    }

    #[test]
    fn a_non_finite_value_breaks_the_run() {
        // CLI-22 AC 9: v is 0, NaN, 2, 3: two runs, the first of one point, the second of two
        let r = |t: f64, v: Cell| vec![Cell::Real(t), v];
        let table = Table { columns: vec!["time".into(), "v".into()], rows: vec![r(0.0, Cell::Real(0.0)), r(1.0, Cell::Real(f64::NAN)), r(2.0, Cell::Real(2.0)), r(3.0, Cell::Real(3.0))] };
        let plot = Plot::of(&table, &transient()).expect("a plot");
        let lengths: Vec<usize> = plot.panels[0].series[0].runs.iter().map(Vec::len).collect();
        assert_eq!(lengths, [1, 2]);
    }
}
