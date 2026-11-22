//! `sample_hold` (IT-05): a source the host steps, a switch that tracks it while a 1 MHz clock is high, a
//! 1 nF hold node and an 8-bit mid-tread ADC that samples at each falling edge. Every source value
//! sits at a code centre (`k / 256` volts), so a solver error under half an LSB (1.95 mV) cannot move
//! a code; the hold node is judged against [`PiecewiseRc`] (track with tau = 10 ns, then hold).

use std::cell::RefCell;
use std::rc::Rc;

use piperine::Overrides;
use piperine_parity::scenario::run::RunRecords;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::scenario::Stimulus;

use crate::clock::ClockEdges;
use crate::rc::{PiecewiseRc, RcSegment};
use crate::suite::{IntegratedPackage, Scenario, Suite};
use crate::table::PrintedColumn;

/// The host's five steps of the source, in volts, each inside a hold (`[k, k + 1/2)` microseconds).
struct SampleHoldHost;

impl SampleHoldHost {
    /// The source `dc` the host sets and when: 0 at the start, then one step per 10 us.
    const STEPS: [(f64, f64); 5] = [(0.0, 0.0), (10.25e-6, 1.0 / 256.0), (20.25e-6, 0.5), (30.25e-6, 255.0 / 256.0), (40.25e-6, 1.2)];

    /// The schedule the run takes.
    fn stimulus() -> Stimulus {
        SampleHoldHost::STEPS.iter().fold(Stimulus::new(), |stimulus, (at, volts)| stimulus.set("source.dc", *volts, *at))
    }
}

/// The scenario: its run takes the host's five steps.
pub struct SampleHoldScenario;

impl Scenario for SampleHoldScenario {
    const RUN: &'static str = "sample_hold";

    fn stimulus() -> Stimulus {
        SampleHoldHost::stimulus()
    }
}

/// What the circuit and the host say the run holds, written from the spec and never read back.
struct SampleSchedule;

impl SampleSchedule {
    /// Samples taken in 51 us: one per falling edge, 1 us to 50 us.
    const SAMPLES: usize = 50;
    /// Time constant of the closed switch: 10 ohm * 1 nF.
    const TRACK_TAU: f64 = 10e-9;
    /// Conversion time and strobe width of the ADC, in seconds.
    const TCONV: f64 = 200e-9;
    /// The print grid, in seconds.
    const GRID: f64 = 10e-9;

    /// The clock that closes the switch.
    fn clock() -> ClockEdges {
        ClockEdges::of(1e6)
    }

    /// The source voltage at `time`: the last host step at or before it.
    fn source_at(time: f64) -> f64 {
        SampleHoldHost::STEPS.iter().filter(|(at, _)| *at <= time).last().map(|(_, volts)| *volts).expect("a step at time 0")
    }

    /// The code the ADC gives for the volts it holds: mid-tread, 8 bits, `vref` 1 V.
    fn code_of(volts: f64) -> i128 {
        (volts * 256.0 + 0.5).floor().clamp(0.0, 255.0) as i128
    }

    /// The code committed for sample `k`: the source value the track before it settled to.
    fn code_for(k: usize) -> i128 {
        SampleSchedule::code_of(SampleSchedule::source_at(SampleSchedule::clock().falling(k) - 1e-9))
    }

    /// The hold node's analytic voltage: tracking the source from each rising edge, holding from each falling one.
    fn hold_node() -> PiecewiseRc {
        let clock = SampleSchedule::clock();
        let mut segments = vec![RcSegment { start: 0.0, target: 0.0, tau: f64::INFINITY }];
        for k in 1..=SampleSchedule::SAMPLES + 1 {
            segments.push(RcSegment { start: clock.rising(k), target: SampleSchedule::source_at(clock.rising(k)), tau: SampleSchedule::TRACK_TAU });
            segments.push(RcSegment { start: clock.falling(k), target: 0.0, tau: f64::INFINITY });
        }
        PiecewiseRc::new(0.0, segments)
    }

    /// The index of `time` on the print grid.
    fn row_of(time: f64) -> i64 {
        (time / SampleSchedule::GRID).round() as i64
    }
}

#[test]
fn each_sample_commits_the_code_of_its_held_voltage() {
    // IT-05 AC 1: samples 1-10, 11-20, 21-30, 31-40 and 41-50 commit 0, 1, 128, 255 and 255
    let wanted: Vec<i128> = [0, 1, 128, 255, 255].iter().flat_map(|code| std::iter::repeat_n(*code, 10)).collect();
    let schedule: Vec<i128> = (1..=SampleSchedule::SAMPLES).map(SampleSchedule::code_for).collect();
    assert_eq!(schedule, wanted, "the schedule the host's steps give is the spec's");
    let ran = Suite::get().ran(SampleHoldScenario::RUN);
    let code = PrintedColumn::integers(&ran.table, "code");
    for k in 1..=SampleSchedule::SAMPLES {
        // the commit of sample `k` is at k us + tconv; the next sample's is a period later
        let at = SampleSchedule::clock().falling(k) + 0.3e-6;
        let committed = code.iter().rev().find(|(time, _)| *time <= at).map(|(_, value)| *value).expect("a row");
        println!("sample_hold: sample {k}: committed {committed:?}, expected {}", wanted[k - 1]);
        assert_eq!(committed, wanted[k - 1], "the code of sample {k}");
    }
}

#[test]
fn the_code_appears_after_tconv_with_a_50_ns_done_strobe() {
    // IT-05 AC 2: a new code shows on the row 200 ns after its sample's edge, and not on the row before;
    // `done` is high on exactly the five rows 200 ns to 240 ns after each edge
    let ran = Suite::get().ran(SampleHoldScenario::RUN);
    let code = PrintedColumn::integers(&ran.table, "code");
    let clock = SampleSchedule::clock();
    for k in (1..=SampleSchedule::SAMPLES).filter(|k| *k > 1 && SampleSchedule::code_for(*k) != SampleSchedule::code_for(k - 1)) {
        let edge_row = SampleSchedule::row_of(clock.falling(k));
        let before = code.iter().find(|(time, _)| SampleSchedule::row_of(*time) == edge_row + 19).map(|(_, value)| *value);
        let after = code.iter().find(|(time, _)| SampleSchedule::row_of(*time) == edge_row + 20).map(|(_, value)| *value);
        println!("sample_hold: sample {k}: code {before:?} at +190 ns, {after:?} at +200 ns");
        assert_ne!(before, Some(SampleSchedule::code_for(k)), "the code of sample {k} is not there at +190 ns");
        assert_eq!(after, Some(SampleSchedule::code_for(k)), "the code of sample {k} is there at +200 ns");
    }
    let done = PrintedColumn::integers(&ran.table, "done");
    let high: Vec<i64> = done.iter().filter(|(_, value)| *value == 1).map(|(time, _)| SampleSchedule::row_of(*time)).collect();
    let wanted: Vec<i64> = (1..=SampleSchedule::SAMPLES).flat_map(|k| (20..=24).map(move |offset| SampleSchedule::row_of(clock.falling(k)) + offset)).collect();
    println!("sample_hold: `done` high on {} rows, expected {}", high.len(), wanted.len());
    assert_eq!(high, wanted, "`done` is high on exactly rows +200 ns to +240 ns after each falling edge (TCONV {:e} s)", SampleSchedule::TCONV);
}

/// The first sample's `done` strobe read on a 5 ns print grid, finer than the suite's 10 ns: the one place
/// the strobe's width is pinned to the nanosecond.
struct FineStrobe;

impl FineStrobe {
    /// The fine print grid, in seconds.
    const GRID: f64 = 5e-9;

    /// The times at which `done` prints 1 in a run of the first sample (falling edge at 1 us) on the fine grid.
    fn high_times() -> Vec<f64> {
        let cache = tempfile::tempdir().expect("a build directory");
        let overrides = Overrides { stop: Some(1.5e-6), print_step: Some(FineStrobe::GRID), print: vec!["SampleHold.done".to_string()], ..Overrides::default() };
        let mut session = IntegratedPackage::open_with(SampleHoldScenario::RUN, &overrides, cache.path());
        let table = session.run(Rc::new(RefCell::new(RunRecords::default()))).unwrap_or_else(|failure| panic!("{failure}"));
        PrintedColumn::integers(&table, "done").into_iter().filter(|(_, value)| *value == 1).map(|(time, _)| time).collect()
    }
}

#[test]
fn the_done_strobe_is_exactly_50_ns_wide() {
    // IT-05 AC 2: on a 5 ns grid `done` is high from 200 ns to 250 ns after the edge (the strobe is TCONV
    // wide, 50 ns, not the 40 to 50 ns the suite's 10 ns grid bounds)
    let high = FineStrobe::high_times();
    let edge = SampleSchedule::clock().falling(1);
    let first = high.first().copied().expect("`done` goes high");
    let last = high.last().copied().expect("`done` goes high");
    println!("sample_hold: `done` high on {} fine rows, {:e} s to {:e} s after the edge", high.len(), first - edge, last - edge);
    assert_eq!(high.len(), 10, "ten 5 ns rows are high: {high:?}");
    assert!((first - edge - SampleSchedule::TCONV).abs() < 1e-12, "`done` rises TCONV after the edge: {:e}", first - edge);
    assert!((last + FineStrobe::GRID - edge - SampleSchedule::TCONV - 50e-9).abs() < 1e-12, "`done` falls 50 ns after it rose: last high row {:e}", last - edge);
}

#[test]
fn the_hold_node_stays_put_while_holding() {
    // IT-05 AC 3: in every hold the node equals the value it had when the switch opened, under
    // TRAN_VOLTAGE, across the source steps that fall inside holds
    let ran = Suite::get().ran(SampleHoldScenario::RUN);
    let hold = PrintedColumn::reals(&ran.table, "hold");
    let clock = SampleSchedule::clock();
    let mut checked = 0;
    for k in 1..=SampleSchedule::SAMPLES {
        let start = hold.iter().find(|(time, _)| SampleSchedule::row_of(*time) == SampleSchedule::row_of(clock.falling(k))).expect("the row at the start of the hold").1;
        for (time, volts) in hold.iter().filter(|(time, _)| *time > clock.falling(k) + 1e-9 && *time < clock.rising(k + 1) - 1e-9) {
            let at = Point::new("integrated/sample_hold", "v(hold) in a hold", Coordinate::Time(*time));
            ToleranceContract::TRAN_VOLTAGE.assert(&at, start, *volts);
            checked += 1;
        }
    }
    println!("sample_hold: {checked} rows inside the holds held to their start value under TRAN_VOLTAGE");
    assert_eq!(checked, SampleSchedule::SAMPLES * 49, "49 rows inside each hold");
}

#[test]
fn the_hold_node_tracks_the_source() {
    // IT-05 AC 4: at the end of each track the node equals the source (to the 50 time constants
    // the track lasts), and every row of the run equals the closed form, under TRAN_VOLTAGE
    let ran = Suite::get().ran(SampleHoldScenario::RUN);
    let hold = PrintedColumn::reals(&ran.table, "hold");
    let clock = SampleSchedule::clock();
    for k in 1..=SampleSchedule::SAMPLES {
        let end = hold.iter().find(|(time, _)| SampleSchedule::row_of(*time) == SampleSchedule::row_of(clock.falling(k))).expect("the row at the end of the track").1;
        let at = Point::new("integrated/sample_hold", "v(hold) at the end of a track", Coordinate::Time(clock.falling(k)));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, SampleSchedule::source_at(clock.falling(k) - 1e-9), end);
        println!("sample_hold: track {k} ends at {end:e} V, source {:e} V", SampleSchedule::source_at(clock.falling(k) - 1e-9));
    }
    let node = SampleSchedule::hold_node();
    for (time, volts) in &hold {
        let at = Point::new("integrated/sample_hold", "v(hold)", Coordinate::Time(*time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, node.at(*time), *volts);
    }
}
