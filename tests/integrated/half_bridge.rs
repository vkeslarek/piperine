//! `half_bridge` (IT-14): two levels of hierarchy under one top (`HalfBridge.leg.high_driver`), complementary
//! commands behind 50 ns gate drivers into a 1 us RL load, an interlock `Never` over the two gates and
//! the host's supply ramp. The load current is judged through the shunt node, `v(coil) = R i`, which is
//! continuous and so follows [`PiecewiseRc`] exactly: in the high state it relaxes to `R / (R + ron)` of
//! the supply, in the low state to zero, with `tau = L / (R + ron) = 1 us` in both.

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::scenario::{Ramp, Stimulus, Trace};

use crate::clock::ClockEdges;
use crate::rc::{PiecewiseRc, RcSegment};
use crate::suite::{Scenario, Suite};
use crate::table::PrintedColumn;

/// The scenario: its run takes the host's supply ramp.
pub struct HalfBridgeScenario;

impl Scenario for HalfBridgeScenario {
    const RUN: &'static str = "half_bridge";

    fn stimulus() -> Stimulus {
        Stimulus::new().ramp("source.dc", Circuit::SUPPLY_RAMP)
    }
}

/// The circuit's numbers and the closed form they give.
struct Circuit;

impl Circuit {
    /// The commander's clock, in hertz.
    const FREQUENCY: f64 = 10e6;
    /// The gate driver's delay, in seconds.
    const DRIVER_DELAY: f64 = 50e-9;
    /// The time constant: 10 uH over (9 ohm load + 1 ohm switch).
    const TAU: f64 = 1e-6;
    /// The share of the supply the shunt sees once the current has settled: 9 / (9 + 1).
    const SHARE: f64 = 0.9;
    /// Clock periods per command: `hi` for ten, `lo` for ten.
    const HALF_PERIOD: usize = 10;
    /// The run's length, in seconds.
    const STOP: f64 = 12e-6;
    /// The print grid, in seconds.
    const GRID: f64 = 10e-9;
    /// The host's staircase of the supply: 0.5 to 1.0 V in five steps from 3 us to 8 us.
    const SUPPLY_RAMP: Ramp = Ramp { from: 0.5, to: 1.0, start: 3e-6, duration: 5e-6, step: 0.1 };

    /// The clock the commander counts.
    fn clock() -> ClockEdges {
        ClockEdges::of(Circuit::FREQUENCY)
    }

    /// The instants the gates change: command edge `1 + m * HALF_PERIOD` plus the driver delay, for `m = 0, 1, ...`;
    /// even `m` closes the high switch, odd `m` the low one.
    fn gate_changes() -> Vec<(f64, bool)> {
        (0..).map(|m| (Circuit::clock().rising(1 + m * Circuit::HALF_PERIOD) + Circuit::DRIVER_DELAY, m % 2 == 0)).take_while(|(at, _)| *at < Circuit::STOP).collect()
    }

    /// The supply at `time`: 0.5 V, then the host's staircase.
    fn supply_at(time: f64) -> f64 {
        Circuit::SUPPLY_RAMP.points().iter().filter(|(at, _)| *at <= time).last().map_or(Circuit::SUPPLY_RAMP.from, |(_, volts)| *volts)
    }

    /// Whether the high switch is closed at `time`.
    fn high_at(time: f64) -> bool {
        Circuit::gate_changes().iter().filter(|(at, _)| *at <= time).last().is_some_and(|(_, high)| *high)
    }

    /// The shunt's analytic voltage: every gate change and every supply step starts a segment that relaxes towards
    /// `SHARE * supply` while the high switch is closed and towards 0 while the low one is.
    fn shunt() -> PiecewiseRc {
        let mut starts: Vec<f64> = Circuit::gate_changes().iter().map(|(at, _)| *at).chain(Circuit::SUPPLY_RAMP.points().iter().map(|(at, _)| *at)).collect();
        starts.sort_by(f64::total_cmp);
        let mut segments = vec![RcSegment { start: 0.0, target: 0.0, tau: Circuit::TAU }];
        segments.extend(starts.into_iter().map(|start| RcSegment {
            start,
            target: if Circuit::high_at(start) { Circuit::SHARE * Circuit::supply_at(start) } else { 0.0 },
            tau: Circuit::TAU,
        }));
        PiecewiseRc::new(0.0, segments)
    }

    /// The index of `time` on the print grid.
    fn row_of(time: f64) -> i64 {
        (time / Circuit::GRID).round() as i64
    }
}

/// The rows of a discrete column as `(time, 0 or 1)`; an unknown cell panics.
struct Levels;

impl Levels {
    /// The rows of the column `name` of the run's table.
    fn of(name: &str) -> Vec<(f64, i128)> {
        let ran = Suite::get().ran(HalfBridgeScenario::RUN);
        PrintedColumn::integers(&ran.table, name)
    }

    /// The print rows on which `rows` goes from `from` to `to`, as row indices.
    fn changes(rows: &[(f64, i128)], from: i128, to: i128) -> Vec<i64> {
        rows.windows(2).filter(|pair| pair[0].1 == from && pair[1].1 == to).map(|pair| Circuit::row_of(pair[1].0)).collect()
    }
}

#[test]
fn the_nested_instance_paths_print_and_the_driver_sees_the_command() {
    // IT-14 AC 1: signals two and three levels below the top (`HalfBridge.leg.hs`, `HalfBridge.leg.high_driver.armed`)
    // print; the driver's own copy of the command is the command, row for row
    let (hi, armed) = (Levels::of("hi"), Levels::of("armed"));
    assert_eq!(hi.len(), armed.len());
    let differing: Vec<f64> = hi.iter().zip(&armed).filter(|(command, seen)| command.1 != seen.1).map(|(command, _)| command.0).collect();
    println!("half_bridge: {} rows of `hi` and of `leg.high_driver.armed`, {} differ", hi.len(), differing.len());
    assert!(differing.is_empty(), "`armed` differs from `hi` at {differing:?}");
    assert!(hi.iter().any(|(_, level)| *level == 1) && hi.iter().any(|(_, level)| *level == 0), "`hi` takes both values");
}

#[test]
fn each_gate_follows_its_command_by_the_driver_delay() {
    // IT-14 AC 2: the gate rises and falls exactly the 50 ns driver delay after its command, on the print grid
    for (command, gate) in [("hi", "hs"), ("lo", "ls")] {
        let (commands, gates) = (Levels::of(command), Levels::of(gate));
        let delay_rows = Circuit::row_of(Circuit::DRIVER_DELAY);
        for (from, to, label) in [(0, 1, "rises"), (1, 0, "falls")] {
            let wanted: Vec<i64> = Levels::changes(&commands, from, to).into_iter().map(|row| row + delay_rows).collect();
            let seen = Levels::changes(&gates, from, to);
            println!("half_bridge: `{gate}` {label} at rows {seen:?}, `{command}` + {delay_rows} rows = {wanted:?}");
            assert!(!wanted.is_empty(), "`{command}` never {label}");
            assert_eq!(seen, wanted, "`{gate}` {label} {delay_rows} rows after `{command}`");
        }
    }
    let changes = Circuit::gate_changes();
    let first_high = Levels::changes(&Levels::of("hs"), 0, 1)[0];
    assert_eq!(first_high, Circuit::row_of(changes[0].0), "the first high-side gate is at edge 1 plus the delay");
}

#[test]
fn the_interlock_holds_on_every_clock_edge_and_on_every_row() {
    // IT-14 AC 3: both gates are never high on one row, and the `Never` of the leg reports one passing record on
    // each of the 120 rising edges of the 12 us run
    let (hs, ls) = (Levels::of("hs"), Levels::of("ls"));
    let both: Vec<f64> = hs.iter().zip(&ls).filter(|(high, low)| high.1 == 1 && low.1 == 1).map(|(high, _)| high.0).collect();
    assert!(both.is_empty(), "both gates are high at {both:?}");
    let ran = Suite::get().ran(HalfBridgeScenario::RUN);
    let edges = (1..).take_while(|k| Circuit::clock().rising(*k) <= Circuit::STOP).count();
    println!("half_bridge: {} `checks` records, {} failed; expected {edges}, 0", ran.checks.seen, ran.checks.failed);
    assert_eq!(edges, 120);
    assert_eq!(ran.checks.seen, edges, "one `never` record per rising edge");
    assert_eq!(ran.checks.failed, 0, "a `never` record failed");
}

#[test]
fn the_shunt_follows_its_closed_form_through_the_ramp() {
    // IT-14 AC 4: every printed row of the shunt equals the closed form under TRAN_VOLTAGE: the RL load through the
    // gate-driver delays, with the supply moved by the host's five steps
    let ran = Suite::get().ran(HalfBridgeScenario::RUN);
    let rows = PrintedColumn::reals(&ran.table, "coil");
    assert_eq!(rows.len(), 1201, "12 us on a 10 ns grid, both ends");
    let shunt = Circuit::shunt();
    for (time, voltage) in rows {
        let at = Point::new("integrated/half_bridge", "v(coil)", Coordinate::Time(time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, shunt.at(time), voltage);
    }
    println!("half_bridge: 1201 rows of the shunt held to the closed form under TRAN_VOLTAGE");
}

#[test]
fn the_supply_ramp_is_five_steps_ending_at_one_volt() {
    // IT-14 AC 5: `Stimulus::ramp` gives the staircase 0.6, 0.7, 0.8, 0.9, 1.0 V at 4, 5, 6, 7 and 8 us, and the
    // run's current at the end of the last high phase is the settled 0.9 V of the shunt less what one
    // time constant of the last phase left
    let points = Circuit::SUPPLY_RAMP.points();
    let wanted = [(4e-6, 0.6), (5e-6, 0.7), (6e-6, 0.8), (7e-6, 0.9), (8e-6, 1.0)];
    assert_eq!(points.len(), wanted.len());
    for ((at, volts), (wanted_at, wanted_volts)) in points.iter().zip(wanted) {
        assert!((at - wanted_at).abs() < 1e-15 && (volts - wanted_volts).abs() < 1e-12, "point {at:e} s, {volts} V; expected {wanted_at:e} s, {wanted_volts} V");
    }
    let ran = Suite::get().ran(HalfBridgeScenario::RUN);
    let last_high = Trace::of(&ran.table, "coil").between(10.2e-6, 11.05e-6).maximum();
    println!("half_bridge: the last high phase peaks at {:e} V at {:e} s", last_high.value, last_high.time);
    let at = Point::new("integrated/half_bridge", "peak of v(coil) in the last high phase", Coordinate::Time(last_high.time));
    ToleranceContract::TRAN_VOLTAGE.assert(&at, Circuit::shunt().at(last_high.time), last_high.value);
    assert!(last_high.value < Circuit::SHARE * 1.0, "the shunt stays under its settled value");
}

#[test]
fn the_low_phase_decays_with_the_load_time_constant() {
    // IT-14 AC 6: in the last low phase the shunt decays exponentially with L / (R + ron) = 1 us, read from the
    // trace over 11.2 us to 12 us; and in the last high phase it settles into the band 0.4 to 0.9 V from the
    // instant the closed form crosses 0.4 V (to within a grid row)
    let ran = Suite::get().ran(HalfBridgeScenario::RUN);
    let trace = Trace::of(&ran.table, "coil");
    let tau = trace.between(11.2e-6, 12e-6).decay_time_constant();
    println!("half_bridge: decay constant {tau:e} s, expected {:e} s", Circuit::TAU);
    assert!((tau / Circuit::TAU - 1.0).abs() < 0.01, "the decay constant {tau:e} s is not {:e} s", Circuit::TAU);

    let start = Circuit::gate_changes().into_iter().filter(|(_, high)| *high).map(|(at, _)| at).filter(|at| *at < 11e-6).last().expect("a high phase");
    let (band_low, band_high) = (0.4, Circuit::SHARE);
    let shunt = Circuit::shunt();
    assert!(shunt.at(start) < band_low, "the phase starts below the band");
    let target = Circuit::SHARE * Circuit::supply_at(start);
    let crossing = start + Circuit::TAU * ((target - shunt.at(start)) / (target - band_low)).ln();
    let settled = trace.between(start + 2e-8, start + 0.95e-6).settled_into(band_low, band_high).expect("the shunt settles into the band");
    println!("half_bridge: settled into [{band_low}, {band_high}] at {settled:e} s, closed form {crossing:e} s");
    assert!(settled >= crossing - 1e-12 && settled < crossing + 2.0 * Circuit::GRID, "settled at {settled:e} s, the closed form crosses at {crossing:e} s");
}
