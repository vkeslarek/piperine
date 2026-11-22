//! `pi_loop` (IT-08): the plant of `pwm_filter` closed through an 8-bit ADC (one sample at each wrap of the
//! counter, 100 in 100 us) and a Q6 PI controller in a reaction. [`PiController`] is the controller
//! written from its equations; it is run on the codes the table commits and must give every duty
//! the run committed, bit for bit. The regulation bounds are derived in the design: one count of
//! duty moves the settled output by 10 mV (2.56 codes), so a one-count limit cycle keeps the code
//! within 128 +- 3, and the output within 0.5 V +- 50 mV (25 mV of ripple, 20 mV of limit cycle,
//! one LSB) once the loop has settled, from 50 us.

use piperine::{Field, Record};
use piperine_parity::scenario::{CompletedRun, Stimulus};

use crate::suite::{Scenario, Suite};
use crate::table::PrintedColumn;

/// The scenario: its run takes no host action.
pub struct PiLoopScenario;

impl Scenario for PiLoopScenario {
    const RUN: &'static str = "pi_loop";

    fn stimulus() -> Stimulus {
        Stimulus::new()
    }
}

/// What one sample made the controller do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Decision {
    /// The code the ADC committed.
    code: i64,
    /// The error, `128 - code`.
    error: i64,
    /// The unclamped output, `(48 e + 5 integ) >> 6`.
    output: i64,
    /// The duty, the output clamped to 0 to 100.
    duty: i64,
    /// The integral term before the sample.
    integral_before: i64,
    /// The integral term after the sample.
    integral_after: i64,
}

impl Decision {
    /// Whether the output was saturated in the direction of the error, so the integral held.
    fn saturated(&self) -> bool {
        (self.output > 100 && self.error > 0) || (self.output < 0 && self.error < 0)
    }
}

/// The PI controller in Q6 integers, from the spec's equations.
struct PiController {
    /// The integral term, in codes.
    integral: i64,
}

impl PiController {
    /// The target code: 0.5 V of a 1 V full scale.
    const TARGET: i64 = 128;
    /// The proportional gain, over 64.
    const KP: i64 = 48;
    /// The integral gain, over 64.
    const KI: i64 = 5;
    /// The integral's ceiling.
    const INTEGRAL_MAX: i64 = 1280;

    /// A controller with nothing integrated.
    fn new() -> PiController {
        PiController { integral: 0 }
    }

    /// One sample of `code`.
    fn step(&mut self, code: i64) -> Decision {
        let error = PiController::TARGET - code;
        let output = (PiController::KP * error + PiController::KI * self.integral) >> 6;
        let before = self.integral;
        let saturated = (output > 100 && error > 0) || (output < 0 && error < 0);
        if !saturated {
            self.integral = (self.integral + error).clamp(0, PiController::INTEGRAL_MAX);
        }
        Decision { code, error, output, duty: output.clamp(0, 100), integral_before: before, integral_after: self.integral }
    }
}

/// The run read as samples: the table's committed codes and duties, the `loop` and `checks` records.
struct Samples;

impl Samples {
    /// Samples in 100 us: one per wrap, at `m` us + 5 ns.
    const COUNT: usize = 100;
    /// The print grid, in seconds.
    const GRID: f64 = 10e-9;
    /// The settled window starts here, in seconds.
    const SETTLED: f64 = 50e-6;

    /// The row on which sample `m` is fully committed: 0.3 us after its wrap (conversion ends at 0.205 us).
    fn row_after(m: usize) -> usize {
        m * 100 + 30
    }

    /// The integer column `name`, indexed by row.
    fn column(ran: &CompletedRun, name: &str) -> Vec<i64> {
        PrintedColumn::integers(&ran.table, name).into_iter().enumerate().map(|(row, (_, value))| value.unwrap_or_else(|| panic!("`{name}` is unknown at row {row}")) as i64).collect()
    }

    /// The `loop` records, in delivery order.
    fn loop_records(ran: &CompletedRun) -> Vec<&Record> {
        ran.records.iter().filter(|record| record.stream == "loop").collect()
    }

    /// The integer field `name` of `record`.
    fn field(record: &Record, name: &str) -> i64 {
        match record.fields.iter().find(|(label, _)| label == name).map(|(_, value)| value) {
            Some(Field::Int(value)) => *value as i64,
            other => panic!("the `{}` record at {:e} s has no integer `{name}`: {other:?}", record.stream, record.time),
        }
    }

    /// The reference run over the codes the table committed, one decision per sample.
    fn reference(ran: &CompletedRun) -> Vec<Decision> {
        let code = Samples::column(ran, "code");
        let mut controller = PiController::new();
        (0..Samples::COUNT).map(|m| controller.step(code[Samples::row_after(m)])).collect()
    }
}

#[test]
fn every_duty_follows_the_pi_reference() {
    // IT-08 AC 1: for each of the 100 samples the duty the run committed equals the Q6 PI reference's on the
    // committed codes, bit for bit, in the table and in the `loop` record
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let (code, duty) = (Samples::column(ran, "code"), Samples::column(ran, "duty"));
    let records = Samples::loop_records(ran);
    assert_eq!(records.len(), Samples::COUNT);
    for (m, decision) in Samples::reference(ran).into_iter().enumerate() {
        let row = Samples::row_after(m);
        assert_eq!(duty[row], decision.duty, "the duty of sample {m} in the table (code {})", code[row]);
        assert_eq!(Samples::field(records[m], "duty"), decision.duty, "the duty of sample {m} in its record");
        assert_eq!(Samples::field(records[m], "err"), decision.error, "the error of sample {m}");
        assert_eq!(Samples::field(records[m], "code"), decision.code, "the code of sample {m}");
    }
    println!("pi_loop: 100 duties equal to the reference; first decisions {:?}", &Samples::reference(ran)[..3]);
}

#[test]
fn the_code_stays_within_three_of_128_after_50_us() {
    // IT-08 AC 2: every code committed after 50 us is within 128 +- 3
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let code = PrintedColumn::integers(&ran.table, "code");
    let settled: Vec<i64> = code.iter().filter(|(time, _)| *time >= Samples::SETTLED).map(|(_, value)| value.expect("a known code") as i64).collect();
    let (lowest, highest) = (settled.iter().min().expect("rows after 50 us"), settled.iter().max().expect("rows after 50 us"));
    println!("pi_loop: codes after 50 us range {lowest} to {highest}, bound 125 to 131");
    assert!(settled.iter().all(|code| (PiController::TARGET - 3..=PiController::TARGET + 3).contains(code)), "codes after 50 us: {lowest} to {highest}");
}

#[test]
fn every_regulated_check_passes_after_50_us() {
    // IT-08 AC 3: after 50 us the `regulated` check delivers records, none before, and every one passes
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let regulated: Vec<&Record> = ran.records.iter().filter(|record| record.stream == "checks" && record.fields.iter().any(|(name, field)| name == "name" && *field == Field::Text("regulated".to_string()))).collect();
    let failed = regulated.iter().filter(|record| !record.is_passing_check()).count();
    println!("pi_loop: {} `regulated` records, {failed} failed", regulated.len());
    assert!(!regulated.is_empty(), "the `regulated` check delivered no record");
    assert!(regulated.iter().all(|record| record.time > Samples::SETTLED), "a `regulated` record before 50 us");
    assert_eq!(failed, 0, "a `regulated` check failed");
    assert_eq!(ran.checks.failed, 0, "a check of the run failed");
}

#[test]
fn one_loop_record_per_sample() {
    // IT-08 AC 4: the `loop` stream holds exactly 100 records, each with the sample's code, error and duty, one
    // per wrap, 205 ns after it (the end of the conversion)
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let records = Samples::loop_records(ran);
    println!("pi_loop: {} loop records, expected {}", records.len(), Samples::COUNT);
    assert_eq!(records.len(), Samples::COUNT);
    for (m, record) in records.iter().enumerate() {
        let at = m as f64 * 1e-6 + 205e-9;
        assert!((record.time - at).abs() < 1e-12, "the record of sample {m} at {:e} s, expected {at:e} s", record.time);
        for name in ["code", "err", "duty"] {
            assert!(record.fields.iter().any(|(label, _)| label == name), "sample {m} has `{name}`");
        }
    }
}

#[test]
fn the_table_has_10001_rows_on_the_10_ns_grid() {
    // IT-08 AC 5: 100 us on a 10 ns grid, both ends, row `k` at `k * 10 ns`
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let times: Vec<f64> = PrintedColumn::reals(&ran.table, "filtered").into_iter().map(|(time, _)| time).collect();
    println!("pi_loop: {} rows, expected 10001", times.len());
    assert_eq!(times.len(), 10_001);
    for (row, time) in times.iter().enumerate() {
        assert!((time - row as f64 * Samples::GRID).abs() < 1e-12, "row {row} at {time:e} s");
    }
}

#[test]
fn the_duty_saturates_and_the_integrator_holds() {
    // IT-08 AC 6: the first samples see the full error: the duty saturates at 100 and the integral holds while
    // the output is saturated in the error's direction: the run's own integral, read from its `loop` records
    // (the term the sample saw, before its update), does not move across a saturated sample
    let ran = Suite::get().ran(PiLoopScenario::RUN);
    let reference = Samples::reference(ran);
    let duty = Samples::column(ran, "duty");
    let records = Samples::loop_records(ran);
    let saturated: Vec<usize> = (0..Samples::COUNT).filter(|m| reference[*m].saturated()).collect();
    println!("pi_loop: saturated samples {saturated:?}; first decisions {:?}", &reference[..3]);
    assert!(!saturated.is_empty(), "the duty never saturated");
    assert_eq!(saturated[0], 1, "the first sample's output is 96 (integral 0); the second saturates");
    for m in &saturated {
        assert_eq!(duty[Samples::row_after(*m)], 100, "sample {m} saturates the duty at 100");
        assert_eq!(reference[*m].integral_before, reference[*m].integral_after, "the reference's integral holds at sample {m}");
        let seen = Samples::field(records[*m], "integ");
        let next = Samples::field(records[*m + 1], "integ");
        assert_eq!(seen, reference[*m].integral_before, "the run's integral at sample {m} is the reference's");
        assert_eq!(next, seen, "the run's integral holds across saturated sample {m}");
    }
    assert!(saturated.len() < 20, "the loop leaves saturation: {saturated:?}");
}
