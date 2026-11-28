//! Scope: chapter 11 of the Book, "Inside a simulation": the chapter's one circuit, `divider.msl`, run
//! through `piperine::` for every number the chapter states of it (its operating point, the first and
//! the longest step of its transient, what `max_step` and `reltol` change, where its crossing is
//! reported, its small-signal response), the circuit with no operating point the chapter shows refused,
//! and a transient that runs out of attempts. The sentences that tell how the algorithms work (Newton,
//! the two rescues, the two stages of a step, the turn-taking of the two sides) are `prose` rows: no
//! value of a run states them. The run errors the chapter's last table names are held where chapter 14
//! holds them, in `book_errors`. Each test opens with the inventory ids it holds, from
//! `tests/book/inventory.tsv`.

use crate::support;

use piperine::Overrides;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Ran;
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;

/// The chapter's circuit, run as the chapter runs it.
struct Divider;

impl Divider {
    /// The time constant of the middle node: 1 kOhm in parallel with 4 kOhm, times 10 nF.
    const TAU: f64 = 800.0 * 10e-9;

    /// The run `overrides` ask of `divider.msl` as the Book writes it, which must succeed.
    fn run(overrides: Overrides) -> Ran {
        Chapter::read().marked("divider.msl", "simulation#1").run(overrides).unwrap_or_else(|failure| panic!("{failure}"))
    }

    /// The overrides of a transient to `stop` that prints the middle node, one row per accepted step.
    fn transient(stop: f64) -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(stop), print: vec!["b".to_string()], ..Overrides::default() }
    }

    /// The length of every step of `ran`: the differences of its `time` column.
    fn steps(ran: &Ran) -> Vec<f64> {
        let times: Vec<f64> = (0..ran.table.rows.len()).map(|row| ran.real(row, "time")).collect();
        times.windows(2).map(|pair| pair[1] - pair[0]).collect()
    }

    /// The time of the one record the monitor writes.
    fn crossing(ran: &Ran) -> f64 {
        let halves = ran.stream("half");
        assert_eq!(halves.len(), 1, "one record of the monitor: {:?}", ran.records);
        halves[0].time
    }

    /// Where the middle node passes 2 V by the formula: `tau * ln 2`.
    fn crossing_by_the_formula() -> f64 {
        Self::TAU * std::f64::consts::LN_2
    }
}

#[test]
fn the_divider_rests_at_four_fifths_of_its_source_and_the_source_delivers_1_ma() {
    // simulation#1; simulation~d78d9147 "a 5 V source, two resistors that divide it, and a capacitor on
    // the middle node"; simulation~d2ecd8c7 "Four fifths of 5 V, as the divider rule says.";
    // simulation~5345ca14 "The third unknown, the current through v1, is -1 mA: add i(v1) to the run's
    // print list to see it."
    let asked = Overrides { analysis: Some("op".to_string()), print: vec!["a".to_string(), "b".to_string(), "i(v1)".to_string()], ..Overrides::default() };
    let ran = Chapter::read().marked("divider.msl", "simulation#1").run(asked).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("5.000000e0", ran.signal("Divider.a"), "the source's node");
    Printed::holds("4.000000e0", ran.signal("Divider.b"), "four fifths of 5 V");
    Printed::holds("-1.000000e-3", ran.signal("i(Divider.v1)"), "the current through v1");
}

#[test]
fn a_transient_starts_at_a_thousandth_of_its_stop_and_no_step_exceeds_a_hundredth() {
    // simulation~f65fc805 "The first step of a run is one thousandth of stop, and no step is longer than
    // one hundredth of stop, ..."; simulation~30614142 "The node b starts at 0 V, because its
    // declaration says so, and charges toward 4 V."; simulation~ffb969ab "... one row per accepted step,
    // 112 rows for this run"; simulation~0225bb03 "The first step is 40 ns, a thousandth of the 40 us.";
    // simulation~700ac9f6 "... from about 3.6 us on they sit at 400 ns, a hundredth of stop";
    // simulation~885dd92c "Find the 40 ns first step and the 400 ns steps in the time column."
    let ran = Divider::run(Divider::transient(40e-6));
    assert_eq!(ran.table.rows.len(), 112, "one row per accepted step");
    Printed::holds("0.000000e0", ran.real(0, "Divider.b"), "b starts at 0 V");
    let last = ran.real(111, "Divider.b");
    assert!(last > 3.97 && last < 4.0, "b charges toward 4 V: {last}");
    let steps = Divider::steps(&ran);
    Printed::holds("4.000000e-8", steps[0], "the first step");
    // a step of a design that locates a crossing ends on a tick: a few femtoseconds past the bound
    let longest = steps.iter().cloned().fold(0.0, f64::max);
    assert!((longest - 400e-9).abs() < 1e-11, "no step is longer than a hundredth of the stop: {longest:e}");
    let from = (0..112).find(|row| ran.real(*row, "time") > 3.6e-6).expect("a row after 3.6 us");
    let at_the_cap = steps[from..].iter().filter(|step| (**step - 400e-9).abs() < 1e-11).count();
    assert!(steps[..from - 1].iter().all(|step| *step < 399e-9), "the steps before 3.6 us are under the cap");
    assert!(at_the_cap >= steps[from..].len() - 3, "from 3.6 us on the steps sit at 400 ns, but the ones around the crossing and the last: {at_the_cap} of {}", steps[from..].len());
}

#[test]
fn max_step_is_the_first_step_and_the_longest() {
    // simulation~f65fc805 "... unless the run gives max_step: then max_step is both the first step and
    // the longest."; simulation~fe78b35e "0.22 ns late" (the row `--max-step 100e-9`: 403 rows, the
    // crossing reported at 5.545401 us)
    let ran = Divider::run(Overrides { max_step: Some(100e-9), ..Divider::transient(40e-6) });
    assert_eq!(ran.table.rows.len(), 403, "the rows of the capped run");
    let steps = Divider::steps(&ran);
    Printed::holds("1.000000e-7", steps[0], "the first step is max_step");
    assert!(steps.iter().all(|step| *step < 100e-9 + 1e-11), "no step is longer than max_step");
    Printed::holds("5.545401e-6", Divider::crossing(&ran), "the crossing of the capped run");
    let late = Divider::crossing(&ran) - Divider::crossing_by_the_formula();
    assert!((late - 0.22e-9).abs() < 0.005e-9, "0.22 ns late: {late:e}");
}

#[test]
fn the_crossing_is_reported_where_the_node_passes_2_v_to_the_accuracy_of_the_waveform() {
    // simulation~9ad7af02 "It must fire at the moment the voltage passes 2 V, ..."; simulation~9ed04be9
    // "The 0.32 ns between them is not an error of the locating: it is the accuracy of the waveform
    // itself, which was computed to reltol."; simulation~b707c0fc "0.32 ns early" (the row of the
    // defaults: 112 rows, 5.544857 us); simulation~e14946a7 "0.005 ns early" (the row `--reltol 1e-5`:
    // 437 rows, 5.545173 us)
    let formula = Divider::crossing_by_the_formula();
    Printed::holds("5.545177e-6", formula, "8 us * ln 2");
    let ran = Divider::run(Divider::transient(40e-6));
    Printed::holds("5.544857e-6", Divider::crossing(&ran), "the crossing of the default run");
    Printed::holds("2.000000e0", Fields::real(ran.stream("half")[0], "v"), "the reaction reads 2 V at the crossing");
    let early = formula - Divider::crossing(&ran);
    assert!((early - 0.32e-9).abs() < 0.005e-9, "0.32 ns early: {early:e}");
    let tight = Divider::run(Overrides { reltol: Some(1e-5), ..Divider::transient(40e-6) });
    assert_eq!(tight.table.rows.len(), 437, "the rows of the run at reltol 1e-5");
    Printed::holds("5.545173e-6", Divider::crossing(&tight), "the crossing at reltol 1e-5");
    let early = formula - Divider::crossing(&tight);
    assert!((early - 0.005e-9).abs() < 0.0005e-9, "0.005 ns early: {early:e}");
}

#[test]
fn a_longer_stop_scales_both_bounds_and_the_error_control_sets_the_steps() {
    // simulation~eca3157d "... (400 ns and 4 us: both scale with stop."; simulation~445b2b41 "The first
    // 40 us now have 41 rows instead of 112."
    let ran = Divider::run(Divider::transient(400e-6));
    let steps = Divider::steps(&ran);
    Printed::holds("4.000000e-7", steps[0], "the first step is a thousandth of 400 us");
    let longest = steps.iter().cloned().fold(0.0, f64::max);
    assert!((longest - 4e-6).abs() < 1e-10, "the largest step is a hundredth of 400 us: {longest:e}");
    let early = (0..ran.table.rows.len()).filter(|row| ran.real(*row, "time") < 40e-6).count();
    assert_eq!(early, 41, "the rows of the first 40 us");
}

#[test]
fn the_small_signal_of_the_source_is_read_by_an_ac_run_alone() {
    // simulation~c86d7533 "... the ac_stim(1.0, 0.0) in Vsource is 1 V at phase 0, and it is ignored by
    // every other analysis.": the operating point is the 5 V of `dc` alone, and at 1 kHz, far below the
    // corner, the middle node carries the divider's 0.8 of that 1 V
    let rest = Divider::run(Faulty::operating_point_of("Divider"));
    Printed::holds("5.000000e0", rest.signal("Divider.a"), "the operating point reads dc alone");
    let swept = Divider::run(Overrides { analysis: Some("ac".to_string()), from: Some(1e3), to: Some(1e6), per_decade: Some(1), print: vec!["b".to_string()], ..Overrides::default() });
    Printed::holds("7.989913e-1", swept.real(0, "mag(Divider.b)"), "the divider's 0.8 at 1 kHz");
    let corner = 1.0 / (2.0 * std::f64::consts::PI * Divider::TAU);
    let expected = 0.8 / (1.0 + (1e3 / corner).powi(2)).sqrt();
    assert!((swept.real(0, "mag(Divider.b)") - expected).abs() < 1e-6, "0.8 / sqrt(1 + (f / fc)^2) at 1 kHz: {expected}");
}

#[test]
fn a_circuit_with_no_operating_point_stops_with_e0505_naming_its_unknowns() {
    // simulation#4; simulation~69743e37 "... the run stops with E0505 and names the unknowns that would
    // not settle."; simulation~d3db4907 "This circuit has no operating point at all: ..."; simulation~c00d7d3c
    // "The number in the middle is the bound: one solve is allowed 500 iterations."; simulation~5c44ec1d
    // "E0505"
    let model = Chapter::read().marked("stuck.msl", "simulation#4");
    Refusal::expect(model.run(Faulty::operating_point_of("Stuck")), "simulation#4", "E0505", "the op analysis failed: Newton: E0505: Newton failed to converge after 500 iterations: node of Stuck.r1.p, Stuck.c1.p");
}

#[test]
fn a_transient_out_of_attempts_says_where_it_was_and_how_small_its_step_had_become() {
    // simulation~10069752 "The budget of attempts after which a transient gives up and says so.";
    // simulation~d967a27c "One more message has no code: a transient that uses up max_steps stops and
    // prints where it was, how small its step had become and how many attempts were accepted and
    // rejected."
    let model = Chapter::read().marked("divider.msl", "simulation#1");
    let failure = model.run(Overrides { max_steps: Some(20), ..Divider::transient(40e-6) }).err().expect("20 attempts do not reach 40 us");
    let said = format!("{failure:#}");
    assert!(said.contains("exceeded the 20 step budget at t = "), "{said}");
    assert!(said.contains("with dt = ") && said.contains("smallest dt seen"), "how small the step had become: {said}");
    assert!(said.contains("accepted ") && said.contains("rejected "), "the attempts accepted and rejected: {said}");
    assert!(!said.contains("E05"), "the message has no code: {said}");
}
