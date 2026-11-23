//! `pwm_filter` (IT-06): a 100 MHz counter PWM with 2 counts of dead time, its high side through `D2A` and
//! 900 ohm into 10 nF (tau = 10 us). Rows are on a 10 ns grid and the clock's rising edges at
//! 5 ns offsets, so row `n` holds what edge `n` committed. The digital side is compared bit for bit
//! with [`PwmReference`], written from the counter's rules; the filter with [`PiecewiseRc`] under
//! TRAN_VOLTAGE, its breakpoints at the edges of the reference's high side.

use piperine::ParamValue;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::scenario::Stimulus;

use crate::clock::ClockEdges;
use crate::rc::{PiecewiseRc, RcSegment};
use crate::suite::{Scenario, Suite};
use crate::table::PrintedColumn;

/// The host's two drives of the duty port, in counts: 25 at the start, 60 at 30.3 us.
struct PwmHost;

impl PwmHost {
    /// Each drive: its instant, in seconds, and the duty.
    const DUTIES: [(f64, i64); 2] = [(0.0, 25), (30.3e-6, 60)];

    /// The schedule the run takes.
    fn stimulus() -> Stimulus {
        PwmHost::DUTIES.iter().fold(Stimulus::new(), |stimulus, (at, duty)| stimulus.drive("duty", ParamValue::Int(*duty), *at))
    }
}

/// The scenario: its run takes the host's two drives.
pub struct PwmFilterScenario;

impl Scenario for PwmFilterScenario {
    const RUN: &'static str = "pwm_filter";

    fn stimulus() -> Stimulus {
        PwmHost::stimulus()
    }
}

/// The two sides' registers after one edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sides {
    /// The high side.
    high: bool,
    /// The low side.
    low: bool,
}

/// The counter PWM written from its rules: it counts 0 to 99 (the first edge wraps it to 0), latches
/// the duty at the wrap, the high side is on for counts below the latched duty, the low side from
/// duty + 2 to 97.
struct PwmReference {
    /// The registers after each rising edge, edge 1 first.
    after_edge: Vec<Sides>,
}

impl PwmReference {
    /// The counter's period, in clock periods.
    const PERIOD: usize = 100;
    /// Counts of dead time on each side of the high-side pulse.
    const DEAD: usize = 2;
    /// Rising edges in 60 us at 100 MHz.
    const EDGES: usize = 6000;
    /// Periods in 60 us.
    const PERIODS: usize = PwmReference::EDGES / PwmReference::PERIOD;

    /// The clock: 100 MHz.
    fn clock() -> ClockEdges {
        ClockEdges::of(100e6)
    }

    /// The duty the host's port holds at `time`: the last drive at or before it.
    fn duty_at(time: f64) -> usize {
        PwmHost::DUTIES.iter().filter(|(at, _)| *at <= time).last().map(|(_, duty)| *duty as usize).expect("a drive at time 0")
    }

    /// The duty latched for period `period` (from 0): what the port holds at the wrap that starts it.
    fn duty_of_period(period: usize) -> usize {
        PwmReference::duty_at(PwmReference::clock().rising(period * PwmReference::PERIOD + 1))
    }

    /// Runs the counter through every rising edge of the run.
    fn run() -> PwmReference {
        let clock = PwmReference::clock();
        let (mut count, mut width) = (PwmReference::PERIOD - 1, 0);
        let mut after_edge = Vec::new();
        for k in 1..=PwmReference::EDGES {
            let wrapping = count == PwmReference::PERIOD - 1;
            let next = if wrapping { 0 } else { count + 1 };
            let latched = if wrapping { PwmReference::duty_at(clock.rising(k)) } else { width };
            after_edge.push(Sides { high: next < latched, low: next >= latched + PwmReference::DEAD && next < PwmReference::PERIOD - PwmReference::DEAD });
            count = next;
            width = latched;
        }
        PwmReference { after_edge }
    }

    /// The registers a print row `row` holds: after edge `row` (before the first edge, both off).
    fn row(&self, row: usize) -> Sides {
        if row == 0 { Sides { high: false, low: false } } else { self.after_edge[row - 1] }
    }

    /// The high side's changes as `(instant, level)`: the breakpoints of the filter's drive.
    fn high_changes(&self) -> Vec<(f64, bool)> {
        let clock = PwmReference::clock();
        let mut level = false;
        let mut changes = Vec::new();
        for (index, sides) in self.after_edge.iter().enumerate() {
            if sides.high != level {
                level = sides.high;
                changes.push((clock.rising(index + 1), level));
            }
        }
        changes
    }

    /// The filter's analytic voltage: the rail through `D2A` while the high side is on, ground while off.
    fn filter(&self) -> PiecewiseRc {
        let mut segments = vec![RcSegment { start: 0.0, target: 0.0, tau: PwmReference::TAU }];
        segments.extend(self.high_changes().into_iter().map(|(start, level)| RcSegment { start, target: if level { 1.0 } else { 0.0 }, tau: PwmReference::TAU }));
        PiecewiseRc::new(0.0, segments)
    }

    /// The filter's time constant: (100 ohm + 900 ohm) * 10 nF.
    const TAU: f64 = 10e-6;
}

/// The pairs `(high, low)` a run printed, one per row, as 0 and 1.
struct PrintedSides;

impl PrintedSides {
    /// The pair of each row of the run's table, row 0 first.
    fn of(ran: &piperine_parity::scenario::CompletedRun) -> Vec<(i128, i128)> {
        let high = PrintedColumn::integers(&ran.table, "hs");
        let low = PrintedColumn::integers(&ran.table, "ls");
        high.iter().zip(&low).map(|((_, high), (_, low))| (*high, *low)).collect()
    }

    /// The runs of equal pairs in rows `from..to` of `pairs`, as `(pair, length)`.
    fn runs(pairs: &[(i128, i128)], from: usize, to: usize) -> Vec<((i128, i128), usize)> {
        let mut runs: Vec<((i128, i128), usize)> = Vec::new();
        for pair in &pairs[from..to] {
            match runs.last_mut() {
                Some((last, length)) if last == pair => *length += 1,
                _ => runs.push((*pair, 1)),
            }
        }
        runs
    }

    /// The rows of period `period`: edges `100 * period + 1` to `100 * period + 100`.
    fn period_rows(period: usize) -> (usize, usize) {
        (period * PwmReference::PERIOD + 1, (period + 1) * PwmReference::PERIOD + 1)
    }
}

#[test]
fn each_period_holds_its_duty_on_the_high_and_low_sides() {
    // IT-06 AC 1: with duty d latched, the high side is high for d clock periods and the low side for
    // 100 - d - 4; every row equals the reference's registers, bit for bit
    let ran = Suite::get().ran(PwmFilterScenario::RUN);
    let pairs = PrintedSides::of(ran);
    assert_eq!(pairs.len(), PwmReference::EDGES + 1, "60 us on a 10 ns grid, both ends");
    let reference = PwmReference::run();
    for (row, pair) in pairs.iter().enumerate() {
        let sides = reference.row(row);
        assert_eq!(*pair, (i128::from(sides.high), i128::from(sides.low)), "row {row}");
    }
    for period in 0..PwmReference::PERIODS {
        let (from, to) = PrintedSides::period_rows(period);
        let duty = PwmReference::duty_of_period(period);
        let high = pairs[from..to].iter().filter(|pair| pair.0 == 1).count();
        let low = pairs[from..to].iter().filter(|pair| pair.1 == 1).count();
        assert_eq!((high, low), (duty, PwmReference::PERIOD - duty - 2 * PwmReference::DEAD), "period {period} with duty {duty}");
    }
    println!("pwm_filter: {} periods, every row equal to the reference", PwmReference::PERIODS);
}

#[test]
fn the_dead_time_is_two_clock_periods_on_both_edges() {
    // IT-06 AC 2: in every period the rows with both sides low come as two runs of exactly 2 rows: after the
    // high side falls and after the low side falls
    let ran = Suite::get().ran(PwmFilterScenario::RUN);
    let pairs = PrintedSides::of(ran);
    for period in 0..PwmReference::PERIODS {
        let (from, to) = PrintedSides::period_rows(period);
        let both_low: Vec<usize> = PrintedSides::runs(&pairs, from, to).into_iter().filter(|(pair, _)| *pair == (0, 0)).map(|(_, length)| length).collect();
        assert_eq!(both_low, [PwmReference::DEAD, PwmReference::DEAD], "the dead times of period {period}");
    }
    println!("pwm_filter: both dead times are 2 clock periods in all {} periods", PwmReference::PERIODS);
}

#[test]
fn the_sides_never_overlap_and_every_never_check_passes() {
    // IT-06 AC 3: no row has both sides high; the `checks` stream holds one passing `never` record per rising
    // edge (6 000) and no failing one
    let ran = Suite::get().ran(PwmFilterScenario::RUN);
    assert!(PrintedSides::of(ran).iter().all(|pair| *pair != (1, 1)), "a row with both sides high");
    println!("pwm_filter: {} `checks` records, {} failed; expected {}, 0", ran.checks.seen, ran.checks.failed, PwmReference::EDGES);
    assert_eq!(ran.checks.seen, PwmReference::EDGES, "one `never` record per rising edge");
    assert_eq!(ran.checks.failed, 0, "a `never` record failed");
    assert_eq!(ran.records.iter().filter(|record| record.stream == "checks").count(), PwmReference::EDGES);
}

#[test]
fn the_driven_duty_is_latched_at_the_next_wrap() {
    // IT-06 AC 4: the duty driven at 30.3 us does not touch the period that began at the wrap of 30.005 us
    // (still 25) and every period from the wrap of 31.005 us on holds 60
    let ran = Suite::get().ran(PwmFilterScenario::RUN);
    let high = PrintedColumn::integers(&ran.table, "hs");
    let clock = PwmReference::clock();
    for period in [30, 31] {
        let (from, to) = PrintedSides::period_rows(period);
        let rows = high[from..to].iter().filter(|(_, value)| *value == 1).count();
        println!("pwm_filter: the period of the wrap at {:e} s holds {rows} high-side rows", clock.rising(from));
        assert_eq!(rows, if period == 30 { 25 } else { 60 }, "the period that starts at the wrap of {:e} s", clock.rising(from));
    }
    for period in 31..PwmReference::PERIODS {
        let (from, to) = PrintedSides::period_rows(period);
        assert_eq!(high[from..to].iter().filter(|(_, value)| *value == 1).count(), 60, "period {period}");
    }
}

#[test]
fn the_filter_follows_its_closed_form() {
    // IT-06 AC 5: every printed row of the filter equals the piecewise-exponential closed form under TRAN_VOLTAGE
    let ran = Suite::get().ran(PwmFilterScenario::RUN);
    let rows = PrintedColumn::reals(&ran.table, "filtered");
    assert_eq!(rows.len(), PwmReference::EDGES + 1);
    let filter = PwmReference::run().filter();
    for (time, volts) in &rows {
        let at = Point::new("integrated/pwm_filter", "v(filtered)", Coordinate::Time(*time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, filter.at(*time), *volts);
    }
    println!("pwm_filter: {} rows of the filter held to the closed form under TRAN_VOLTAGE", rows.len());
}
