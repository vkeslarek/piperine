//! `threshold` (IT-04): a 100 kHz square through `D2A` and 900 ohms into 1 nF, watched by an `A2D` at
//! 0.5 V +- 0.05 V. The closed form is [`PiecewiseRc`] with tau = 1 us; a crossing is judged by the
//! analytic voltage at the instant of its record (the rule of `buck_boundary`), so no time tolerance
//! is written anywhere.

use piperine::{Field, Record};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::scenario::{DiscreteTrace, Stimulus};

use crate::clock::ClockEdges;
use crate::rc::{PiecewiseRc, RcSegment};
use crate::suite::{Scenario, Suite};
use crate::table::PrintedColumn;

/// The scenario: its run takes no host action.
pub struct ThresholdScenario;

impl Scenario for ThresholdScenario {
    const RUN: &'static str = "threshold";

    fn stimulus() -> Stimulus {
        Stimulus::new()
    }
}

/// The circuit's numbers and the closed form they give.
struct Circuit;

impl Circuit {
    /// The square's frequency, in hertz.
    const FREQUENCY: f64 = 100e3;
    /// The time constant: (100 ohm + 900 ohm) * 1 nF.
    const TAU: f64 = 1e-6;
    /// The comparator's rising threshold, `vth + hyst / 2`.
    const RISING: f64 = 0.55;
    /// The comparator's falling threshold, `vth - hyst / 2`.
    const FALLING: f64 = 0.45;
    /// Edges of each kind in 42 us.
    const EDGES: usize = 4;

    /// The clock that drives the capacitor.
    fn clock() -> ClockEdges {
        ClockEdges::of(Circuit::FREQUENCY)
    }

    /// The capacitor's analytic voltage: the rail through `D2A` while the clock is high, ground while low.
    fn capacitor() -> PiecewiseRc {
        let clock = Circuit::clock();
        let mut segments = vec![RcSegment { start: 0.0, target: 0.0, tau: Circuit::TAU }];
        for k in 1..=Circuit::EDGES {
            segments.push(RcSegment { start: clock.rising(k), target: 1.0, tau: Circuit::TAU });
            segments.push(RcSegment { start: clock.falling(k), target: 0.0, tau: Circuit::TAU });
        }
        PiecewiseRc::new(0.0, segments)
    }
}

/// The `edges` records of the run, as `(time, rising, count)`.
struct Edges;

impl Edges {
    /// The records of the `edges` stream, in delivery order.
    fn of(records: &[Record]) -> Vec<(f64, bool, i128)> {
        records
            .iter()
            .filter(|record| record.stream == "edges")
            .map(|record| {
                let field = |name: &str| record.fields.iter().find(|(label, _)| label == name).map(|(_, value)| value.clone()).unwrap_or_else(|| panic!("an `edges` record has `{name}`"));
                match (field("rising"), field("n")) {
                    (Field::Bool(rising), Field::Int(count)) => (record.time, rising, count),
                    other => panic!("an `edges` record at {:e} s: {other:?}", record.time),
                }
            })
            .collect()
    }
}

#[test]
fn the_counters_read_four_rises_and_four_falls() {
    // IT-04 AC 1: the 42 us run holds four rising and four falling edges of the comparator
    let ran = Suite::get().ran(ThresholdScenario::RUN);
    for (counter, label) in [("rises", "rising"), ("falls", "falling")] {
        let trace = DiscreteTrace::of(&ran.table, counter);
        let last = trace.value_at(42e-6);
        println!("threshold: {label} edges counted {last:?}, expected {}", Circuit::EDGES);
        assert_eq!(last, Circuit::EDGES as i128, "the {label} counter at 42 us");
    }
}

#[test]
fn the_edges_stream_holds_eight_alternating_records() {
    // IT-04 AC 2: eight records, the first a rise, alternating, each kind counting 1 to 4
    let ran = Suite::get().ran(ThresholdScenario::RUN);
    let edges = Edges::of(&ran.records);
    println!("threshold: {} edge records, expected 8", edges.len());
    assert_eq!(edges.len(), 2 * Circuit::EDGES, "the `edges` stream: {edges:?}");
    for (index, (time, rising, count)) in edges.iter().enumerate() {
        assert_eq!(*rising, index % 2 == 0, "record {index} at {time:e} s alternates from a rise");
        assert_eq!(*count, (index / 2 + 1) as i128, "record {index} counts its kind");
    }
}

#[test]
fn each_edge_record_is_where_the_capacitor_crosses_its_threshold() {
    // IT-04 AC 3: the analytic voltage at a record's instant is its threshold, under TRAN_VOLTAGE
    let ran = Suite::get().ran(ThresholdScenario::RUN);
    let edges = Edges::of(&ran.records);
    assert!(!edges.is_empty(), "no edge happened: the comparator never tripped");
    let capacitor = Circuit::capacitor();
    for (time, rising, _) in edges {
        let wanted = if rising { Circuit::RISING } else { Circuit::FALLING };
        let analytic = capacitor.at(time);
        println!("threshold: {} edge at {time:e} s: analytic {analytic:e} V, threshold {wanted} V", if rising { "rising" } else { "falling" });
        let at = Point::new("integrated/threshold", "v(cap) at an edge record", Coordinate::Time(time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, wanted, analytic);
    }
}

#[test]
fn each_edge_falls_in_its_half_period() {
    // IT-04 AC 4: the n-th rise lies in the n-th high half period, the n-th fall in the low half period after it
    let ran = Suite::get().ran(ThresholdScenario::RUN);
    let edges = Edges::of(&ran.records);
    assert!(!edges.is_empty(), "no edge happened");
    let clock = Circuit::clock();
    for (time, rising, count) in edges {
        let k = count as usize;
        let (from, to) = if rising { (clock.rising(k), clock.falling(k)) } else { (clock.falling(k), clock.rising(k + 1)) };
        println!("threshold: edge {k} ({}) at {time:e} s in [{from:e}, {to:e})", if rising { "rising" } else { "falling" });
        assert!(from <= time && time < to, "edge {k} at {time:e} s is outside [{from:e}, {to:e})");
    }
}

#[test]
fn the_capacitor_follows_its_closed_form() {
    // IT-04 AC 5: every printed row of the capacitor equals the closed form under TRAN_VOLTAGE
    let ran = Suite::get().ran(ThresholdScenario::RUN);
    let rows = PrintedColumn::reals(&ran.table, "cap");
    assert_eq!(rows.len(), 421, "42 us on a 100 ns grid, both ends");
    let capacitor = Circuit::capacitor();
    for (time, voltage) in rows {
        let at = Point::new("integrated/threshold", "v(cap)", Coordinate::Time(time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, capacitor.at(time), voltage);
    }
    println!("threshold: 421 rows of the capacitor held to the closed form under TRAN_VOLTAGE");
}
