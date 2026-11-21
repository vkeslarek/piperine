//! `state_machine` (IT-07): a 10 MHz sequencer steps an enum-carried phase through charge, hold and dump of
//! a 1 nF capacitor behind two switches; an `A2D` at 0.9 V +- 10 mV reports over-voltage, two
//! consecutive edges with it enter a 30-period fault that dumps the capacitor, and the retry after
//! it is clean once the source is back at 1 V. [`PhaseMachine`] is the sequencer written from its
//! rules and fed what the table holds before each edge; the capacitor is judged against
//! [`PiecewiseRc`] built on the reference's phases, under TRAN_VOLTAGE.

use piperine::{Field, ParamValue, Record};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::scenario::{CompletedRun, Stimulus};

use crate::clock::ClockEdges;
use crate::rc::{PiecewiseRc, RcSegment};
use crate::suite::{Scenario, Suite};
use crate::table::PrintedColumn;

/// What the host does: raises `en` and steps the source.
struct StateMachineHost;

impl StateMachineHost {
    /// Each change of the source `dc`: its instant, in seconds, and the volts. The source starts at 0 V so
    /// that the capacitor's operating point is 0 V (a floating node would settle at half the rail).
    const SOURCE: [(f64, f64); 3] = [(0.5e-6, 1.0), (10.0e-6, 2.0), (13.0e-6, 1.0)];

    /// The schedule the run takes.
    fn stimulus() -> Stimulus {
        let stimulus = Stimulus::new().drive("en", ParamValue::Bool(false), 0.0).drive("en", ParamValue::Bool(true), 1.0e-6);
        StateMachineHost::SOURCE.iter().fold(stimulus, |stimulus, (at, volts)| stimulus.set("source.dc", *volts, *at))
    }

    /// The source voltage at `time`.
    fn source_at(time: f64) -> f64 {
        StateMachineHost::SOURCE.iter().filter(|(at, _)| *at <= time).last().map_or(0.0, |(_, volts)| *volts)
    }
}

/// The scenario: its run takes the host's drive of `en` and steps of the source.
pub struct StateMachineScenario;

impl Scenario for StateMachineScenario {
    const RUN: &'static str = "state_machine";

    fn stimulus() -> Stimulus {
        StateMachineHost::stimulus()
    }
}

/// The sequencer's phase, in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Not enabled: both switches open.
    Idle,
    /// The charge switch is closed, for 20 clock periods.
    Charge,
    /// Both switches open, for 10 clock periods.
    Hold,
    /// The dump switch is closed, for 20 clock periods.
    Dump,
    /// An over-voltage was seen on two edges in a row: the dump switch is closed, for 30 clock periods.
    Fault,
}

impl Phase {
    /// The index a table prints: the variant's place in the declaration.
    fn index(self) -> i128 {
        self as i128
    }
}

/// The sequencer's fault, in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultCause {
    /// No fault is recorded.
    Clear,
    /// The over-voltage input was seen on two edges in a row.
    OverVoltage,
}

/// The sequencer written from its rules: what one rising edge does, given `en` and `ov` as the row
/// before the edge holds them.
#[derive(Debug, Clone, Copy)]
struct PhaseMachine {
    /// The phase after the last edge.
    phase: Phase,
    /// The fault after the last edge.
    fault: FaultCause,
    /// Edges spent in the phase.
    ticks: usize,
    /// `ov` as the previous edge read it.
    ov_last: bool,
}

impl PhaseMachine {
    /// Edges a charge phase lasts.
    const CHARGE: usize = 20;
    /// Edges a hold phase lasts.
    const HOLD: usize = 10;
    /// Edges a dump phase lasts.
    const DUMP: usize = 20;
    /// Edges a fault lasts.
    const FAULT: usize = 30;

    /// A machine at rest: idle, clear, no edge seen.
    fn new() -> PhaseMachine {
        PhaseMachine { phase: Phase::Idle, fault: FaultCause::Clear, ticks: 0, ov_last: false }
    }

    /// One rising edge with `enabled` and `over` as read.
    fn step(&mut self, enabled: bool, over: bool) {
        if self.phase != Phase::Fault && self.phase != Phase::Idle && over && self.ov_last {
            self.phase = Phase::Fault;
            self.fault = FaultCause::OverVoltage;
            self.ticks = 0;
        } else if !enabled && self.phase != Phase::Fault {
            self.phase = Phase::Idle;
        } else {
            match self.phase {
                Phase::Idle => {
                    self.phase = Phase::Charge;
                    self.ticks = 0;
                }
                Phase::Charge => self.dwell(PhaseMachine::CHARGE, Phase::Hold),
                Phase::Hold => self.dwell(PhaseMachine::HOLD, Phase::Dump),
                Phase::Dump => self.dwell(PhaseMachine::DUMP, Phase::Charge),
                Phase::Fault => self.dwell(PhaseMachine::FAULT, Phase::Charge),
            }
        }
        self.ov_last = over;
    }

    /// Counts one edge in the phase; at `length` edges moves to `next`, and a charge that ends clears the fault.
    fn dwell(&mut self, length: usize, next: Phase) {
        self.ticks += 1;
        if self.ticks == length {
            if self.phase == Phase::Charge {
                self.fault = FaultCause::Clear;
            }
            self.phase = next;
            self.ticks = 0;
        }
    }
}

/// The machine's state after every edge of the run, fed with the table's `en` and `ov` of the row before
/// each edge, and what the table printed.
struct Timeline {
    /// The state after edge `k`, at index `k - 1`.
    after_edge: Vec<PhaseMachine>,
}

impl Timeline {
    /// The clock: 10 MHz.
    fn clock() -> ClockEdges {
        ClockEdges::of(10e6)
    }

    /// Rising edges in 22 us.
    const EDGES: usize = 220;
    /// Rows of the print grid (100 ns, both ends).
    const ROWS: usize = Timeline::EDGES + 1;
    /// The capacitor's charging and discharging time constant: (1 ohm + 1 kohm) * 1 nF.
    const TAU: f64 = 1.001e-6;

    /// Runs the reference over `ran`'s table.
    fn of(ran: &CompletedRun) -> Timeline {
        let enabled = PrintedColumn::integers(&ran.table, "en");
        let over = PrintedColumn::integers(&ran.table, "ov");
        let read = |rows: &[(f64, Option<i128>)], row: usize, name: &str| rows[row].1.unwrap_or_else(|| panic!("`{name}` is unknown at row {row}")) == 1;
        let mut machine = PhaseMachine::new();
        let mut after_edge = Vec::new();
        for edge in 1..=Timeline::EDGES {
            machine.step(read(&enabled, edge - 1, "en"), read(&over, edge - 1, "ov"));
            after_edge.push(machine);
        }
        Timeline { after_edge }
    }

    /// The state a print row `row` holds: after edge `row` (before the first edge, at rest).
    fn row(&self, row: usize) -> PhaseMachine {
        if row == 0 { PhaseMachine::new() } else { self.after_edge[row - 1] }
    }

    /// The edges (from 1) at which the phase changed.
    fn changes(&self) -> Vec<usize> {
        (1..=Timeline::EDGES).filter(|edge| self.row(*edge).phase != self.row(edge - 1).phase).collect()
    }

    /// The capacitor's analytic voltage: breakpoints at every change of phase and every step of the source.
    fn capacitor(&self) -> PiecewiseRc {
        let clock = Timeline::clock();
        let mut times: Vec<f64> = vec![0.0];
        times.extend(self.changes().into_iter().map(|edge| clock.rising(edge)));
        times.extend(StateMachineHost::SOURCE.iter().map(|(at, _)| *at));
        times.sort_by(f64::total_cmp);
        let segments = times
            .into_iter()
            .map(|start| {
                let edges_done = ((start / 1e-7 + 0.5) + 1e-6).floor() as usize;
                let (target, tau) = match self.row(edges_done).phase {
                    Phase::Charge => (StateMachineHost::source_at(start), Timeline::TAU),
                    Phase::Dump | Phase::Fault => (0.0, Timeline::TAU),
                    Phase::Idle | Phase::Hold => (0.0, f64::INFINITY),
                };
                RcSegment { start, target, tau }
            })
            .collect();
        PiecewiseRc::new(0.0, segments)
    }
}

/// The records of one stream of the run, as `(time, fields)`.
struct Stream;

impl Stream {
    /// The records of `stream` in delivery order.
    fn of<'a>(records: &'a [Record], stream: &str) -> Vec<&'a Record> {
        records.iter().filter(|record| record.stream == stream).collect()
    }

    /// The integer field `name` of `record`.
    fn integer(record: &Record, name: &str) -> i128 {
        match record.fields.iter().find(|(label, _)| label == name).map(|(_, value)| value) {
            Some(Field::Int(value)) => *value,
            other => panic!("the `{}` record at {:e} s has no integer `{name}`: {other:?}", record.stream, record.time),
        }
    }
}

/// The printed integer column `name` as a vector indexed by row.
struct Printed;

impl Printed {
    /// The value of each row of `name`; an unknown row panics.
    fn column(ran: &CompletedRun, name: &str) -> Vec<i128> {
        PrintedColumn::integers(&ran.table, name).into_iter().enumerate().map(|(row, (_, value))| value.unwrap_or_else(|| panic!("`{name}` is unknown at row {row}"))).collect()
    }
}

#[test]
fn the_phase_follows_the_reference_at_every_edge() {
    // IT-07 AC 1: after every rising edge `phase` is what the reference gives, fed with `en` and `ov` as the
    // table held them before the edge
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let phase = Printed::column(ran, "phase");
    assert_eq!(phase.len(), Timeline::ROWS, "22 us on a 100 ns grid, both ends");
    let timeline = Timeline::of(ran);
    for edge in 1..=Timeline::EDGES {
        assert_eq!(phase[edge], timeline.row(edge).phase.index(), "the phase after edge {edge} at {:e} s", Timeline::clock().rising(edge));
    }
    let changes = timeline.changes();
    println!("state_machine: {} edges checked, {} changes of phase at edges {changes:?}", Timeline::EDGES, changes.len());
    // the design lists twelve changes up to the Charge at 19.75 us; the run goes on to 22 us and the
    // Hold at 21.75 us (edge 218) is the thirteenth (derivation corrected, see the Handoff)
    assert_eq!(changes.len(), 13, "the run goes through thirteen changes of phase: a vacuous machine proves nothing");
}

#[test]
fn the_enums_print_as_their_variant_indices() {
    // IT-07 AC 2: `phase` prints 0 (Idle) to 4 (Fault) and `fault` 0 (Clear) or 1 (OverVoltage), by declaration order
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let mut phases: Vec<i128> = Printed::column(ran, "phase");
    phases.sort_unstable();
    phases.dedup();
    assert_eq!(phases, [Phase::Idle, Phase::Charge, Phase::Hold, Phase::Dump, Phase::Fault].map(Phase::index), "every phase is printed, by its index");
    let mut faults: Vec<i128> = Printed::column(ran, "fault");
    faults.sort_unstable();
    faults.dedup();
    assert_eq!(faults, [FaultCause::Clear as i128, FaultCause::OverVoltage as i128]);
    println!("state_machine: phases printed {phases:?}, faults printed {faults:?}");
}

#[test]
fn the_capacitor_follows_its_closed_form_through_each_phase() {
    // IT-07 AC 3: every row of the capacitor equals the closed form under TRAN_VOLTAGE (so at each phase's end),
    // and a hold phase keeps the value it started with
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let capacitor = PrintedColumn::reals(&ran.table, "cap");
    assert_eq!(capacitor.len(), Timeline::ROWS);
    let timeline = Timeline::of(ran);
    let closed_form = timeline.capacitor();
    for (time, volts) in &capacitor {
        let at = Point::new("integrated/state_machine", "v(cap)", Coordinate::Time(*time));
        ToleranceContract::TRAN_VOLTAGE.assert(&at, closed_form.at(*time), *volts);
    }
    let mut held = 0;
    for row in 1..=Timeline::EDGES {
        if timeline.row(row).phase == Phase::Hold && timeline.row(row - 1).phase == Phase::Hold {
            let entry = (1..=row).rev().find(|candidate| timeline.row(*candidate - 1).phase != Phase::Hold).expect("a hold has an entry");
            let at = Point::new("integrated/state_machine", "v(cap) in a hold", Coordinate::Time(capacitor[row].0));
            ToleranceContract::TRAN_VOLTAGE.assert(&at, capacitor[entry].1, capacitor[row].1);
            held += 1;
        }
    }
    println!("state_machine: {} rows held to the closed form, {held} rows inside holds equal to their entry value", capacitor.len());
    assert!(held > 0, "no hold phase was judged");
}

#[test]
fn the_trip_record_is_where_the_capacitor_crosses_0_91_v() {
    // IT-07 AC 4: the `trips` record is delivered where the analytic capacitor voltage is 0.91 V, under TRAN_VOLTAGE
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let trips = Stream::of(&ran.records, "trips");
    assert_eq!(trips.len(), 1, "one over-voltage in the run: {trips:?}");
    let closed_form = Timeline::of(ran).capacitor();
    let at = Point::new("integrated/state_machine", "v(cap) at the trip record", Coordinate::Time(trips[0].time));
    println!("state_machine: the trip at {:e} s, analytic {:e} V, threshold 0.91 V", trips[0].time, closed_form.at(trips[0].time));
    ToleranceContract::TRAN_VOLTAGE.assert(&at, 0.91, closed_form.at(trips[0].time));
}

#[test]
fn two_edges_with_ov_enter_fault_with_the_dump_switch_on() {
    // IT-07 AC 5: `ov` high before two consecutive edges makes the second one enter Fault with OverVoltage,
    // the charge switch off and the dump switch on
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let (phase, fault, over) = (Printed::column(ran, "phase"), Printed::column(ran, "fault"), Printed::column(ran, "ov"));
    let (charge, dump) = (Printed::column(ran, "charge"), Printed::column(ran, "dump"));
    let entry = (1..=Timeline::EDGES).find(|edge| phase[*edge] == Phase::Fault.index()).expect("the run enters Fault: the over-voltage never tripped twice");
    println!("state_machine: Fault entered at edge {entry} ({:e} s); ov before edges {} and {entry}: {} and {}", Timeline::clock().rising(entry), entry - 1, over[entry - 2], over[entry - 1]);
    assert_eq!((over[entry - 2], over[entry - 1]), (1, 1), "`ov` is high before the two edges");
    assert_eq!(over[entry - 3], 0, "and was low before the edge ahead of them");
    assert_eq!(phase[entry - 1], Phase::Charge.index(), "the phase before the second edge is Charge");
    assert_eq!((fault[entry], charge[entry], dump[entry]), (FaultCause::OverVoltage as i128, 0, 1));
}

#[test]
fn the_hiccup_lasts_30_clock_periods_then_retries() {
    // IT-07 AC 6: Fault holds for exactly 30 edges and the 30th enters Charge
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let phase = Printed::column(ran, "phase");
    let entry = (1..=Timeline::EDGES).find(|edge| phase[*edge] == Phase::Fault.index()).expect("the run enters Fault");
    let length = phase[entry..].iter().take_while(|value| **value == Phase::Fault.index()).count();
    println!("state_machine: Fault from edge {entry} for {length} rows, then phase {}", phase[entry + length]);
    assert_eq!(length, PhaseMachine::FAULT, "Fault lasts 30 clock periods");
    assert_eq!(phase[entry + length], Phase::Charge.index(), "the retry is a Charge");
}

#[test]
fn a_clean_retry_clears_the_fault_and_resumes_the_cycle() {
    // IT-07 AC 7: the retry's Charge ends with the source back at 1 V and no over-voltage: `fault` is Clear from
    // that edge and Charge, Hold, Dump resume
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let (phase, fault) = (Printed::column(ran, "phase"), Printed::column(ran, "fault"));
    let entry = (1..=Timeline::EDGES).find(|edge| phase[*edge] == Phase::Fault.index()).expect("the run enters Fault");
    let retry = entry + PhaseMachine::FAULT;
    assert!((retry..retry + PhaseMachine::CHARGE).all(|row| phase[row] == Phase::Charge.index() && fault[row] == FaultCause::OverVoltage as i128), "the retry charges with the fault still latched");
    let resumed = retry + PhaseMachine::CHARGE;
    println!("state_machine: retry charge from edge {retry}, clear from edge {resumed} ({:e} s)", Timeline::clock().rising(resumed));
    assert_eq!((phase[resumed], fault[resumed]), (Phase::Hold.index(), FaultCause::Clear as i128), "the clean charge clears the fault as it ends");
    assert!((resumed..resumed + PhaseMachine::HOLD).all(|row| phase[row] == Phase::Hold.index()), "the hold resumes");
    assert!((resumed + PhaseMachine::HOLD..resumed + PhaseMachine::HOLD + PhaseMachine::DUMP).all(|row| phase[row] == Phase::Dump.index()), "the dump resumes");
}

#[test]
fn one_phase_record_per_change_at_its_edge() {
    // IT-07 AC 8: the `phase` stream holds one record per change of phase, at the edge that made it, with the new index
    let ran = Suite::get().ran(StateMachineScenario::RUN);
    let phase = Printed::column(ran, "phase");
    let records = Stream::of(&ran.records, "phase");
    let changes: Vec<usize> = (1..=Timeline::EDGES).filter(|edge| phase[*edge] != phase[edge - 1]).collect();
    println!("state_machine: {} phase records for {} changes in the table", records.len(), changes.len());
    assert_eq!(records.len(), changes.len(), "one record per change");
    for (record, edge) in records.iter().zip(changes) {
        assert!((record.time - Timeline::clock().rising(edge)).abs() < 1e-12, "the record at {:e} s is at edge {edge} ({:e} s)", record.time, Timeline::clock().rising(edge));
        assert_eq!(Stream::integer(record, "to"), phase[edge], "the new phase of edge {edge}");
    }
}
