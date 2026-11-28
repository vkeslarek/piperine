//! Places and copies run from MSL source on the Rust backend (feature `value-semantics`, VS-03,
//! VS-04, VS-07): one test per run-time criterion, each model written in `tests/value_semantics/`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites, never read back from the
//! model.
#![cfg(unix)]

use piperine_api::Field;

#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "source/operators.rs"]
mod operators;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

use bytes::CarrierBytes;
use operators::PlaceOperators;
use run::SourceRun;

/// The model files of `tests/value_semantics/`.
const RUN: SourceRun = SourceRun::in_directory("tests/value_semantics");

/// How far the solver may move a value it was given as a current into 1 ohm: its `gmin` and
/// round-off, not a tolerance of the rules (a reaction's values are bit for bit).
const SOLVE: f64 = 1e-9;

#[test]
fn a_let_without_mut_runs_in_a_function_a_reaction_and_analog() {
    // VS-03 AC 1 (§1.5.12): `x = e` and every compound operator on a `let` with no marker, in a function, in a
    // reaction (bit for bit) and in an `analog` block (after the solve); a name a `match` arm or an `if let`
    // binds is assigned too (AC 3).
    let names = ["f", "g", "local_n", "in_arm", "in_if"];
    let committed = RUN.first_commits("places.msl", &names);
    let wanted = [CarrierBytes::real(PlaceOperators::real(2.0)), CarrierBytes::byte(PlaceOperators::integer(20)), CarrierBytes::byte(PlaceOperators::integer(20)), CarrierBytes::byte(4 + 1), CarrierBytes::byte(4 * 3)];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
    let currents = RUN.operating_point("places.msl", "AnalogBench", &["a", "b"]);
    let analog = [PlaceOperators::real(2.0), 20.0f64 - 7.0 + 1.0];
    for (k, want) in analog.iter().enumerate() {
        assert!((currents[k] - want).abs() <= SOLVE * want.abs().max(1.0), "analog node {k}: {want:?}, the operating point has {:?}", currents[k]);
    }
}

#[test]
fn an_assigned_loop_variable_does_not_change_the_loop() {
    // VS-03 AC 3, VS-12 (§1.6.9): `for i in 0..n { i += 10; .. }` changes the copy the body names, not the
    // loop: the body runs for 0, 1, 2 and 3, and sees each of them plus ten.
    let committed = RUN.first_commits("places.msl", &["loop_sum", "loop_again"]);
    let sum: u8 = (0..4u8).map(|i| i + 10).sum();
    assert_eq!(committed[0], CarrierBytes::byte(sum), "the body of shifted_sum sees 10, 11, 12, 13");
    let steps = (0..4u8).count() as u8;
    assert_eq!(committed[1], CarrierBytes::byte(steps), "the loop counts 0 to 3 whatever the body does to its variable");
}

#[test]
fn a_closure_keeps_the_value_it_captured() {
    // VS-04 AC 1 (§1.5.14): the closure holds the value where it was written; the later assignment does not
    // reach it, and the local itself changes.
    let committed = RUN.first_commits("places.msl", &["kept", "latest"]);
    let captured = 3u8;
    let plus = move |x: u8| x + captured;
    let local = 10u8;
    assert_eq!(committed[0], CarrierBytes::byte(plus(1)), "the closure read 3");
    assert_eq!(committed[1], CarrierBytes::byte(local), "the local reads 10");
}

#[test]
fn a_callee_cannot_change_the_callers_local() {
    // VS-04 AC 2 (§1.4.6, §3.1.10): the argument is a copy; a `let` of it assigned in the callee leaves the
    // caller's local as it was.
    let committed = RUN.first_commits("places.msl", &["callee", "caller"]);
    assert_eq!(committed[0], CarrierBytes::byte(5 + 1), "the callee returns its own changed copy");
    assert_eq!(committed[1], CarrierBytes::byte(5), "the caller's local is as it was");
}

#[test]
fn a_value_is_read_twice() {
    // VS-04 AC 3 (§1.4.6): a value of any type is read twice with no move; a copy that is assigned leaves the
    // other reads of the original as they were.
    let committed = RUN.first_commits("places.msl", &["twice", "copied"]);
    assert_eq!(committed[0], CarrierBytes::byte(7 + 7), "x is read twice by read_twice");
    assert_eq!(committed[1], CarrierBytes::byte(6 + 5 + 5), "p changed to 6; q and a are 5");
}

#[test]
fn an_enum_without_copy_is_carried_and_compared() {
    // VS-05 AC 2 (§1.4.6, H.8): an enum that derives `Repr` and `PartialEq` and not `Clone` or `Copy` is carried
    // on a signal, compared with `==` and copied into a second local, as before: `Mode::Run` is variant 1 of
    // three, equal to its copy and not to `Mode::Hold`.
    let committed = RUN.first_commits("enums.msl", &["m", "same", "differs"]);
    let variant_run = 1u8;
    assert_eq!(committed[0], CarrierBytes::byte(variant_run), "the carried enum is its variant");
    assert_eq!(committed[1], CarrierBytes::byte((variant_run == variant_run) as u8), "a value equals its copy");
    assert_eq!(committed[2], CarrierBytes::byte((variant_run == 2) as u8), "Run is not Hold");
}

#[test]
fn a_string_is_passed_and_compared_by_value() {
    // VS-07 AC 1 (§1.2.9): a `str` is passed to a function, held by a `let` and a `const`, and compared with `==`
    // and `!=`; the comparison is the one Rust makes of the same two strings.
    let names = ["same", "other", "named", "held", "unlike"];
    let committed = RUN.first_commits("strings.msl", &names);
    let wanted = [u8::from("a" == "a"), u8::from("a" == "b"), u8::from("ab" == "ab"), u8::from("held" == "held"), u8::from("x" != "y")];
    for (k, label) in names.iter().enumerate() {
        assert_eq!(committed[k], CarrierBytes::byte(wanted[k]), "{label} is the comparison Rust makes");
    }
}

#[test]
fn the_reporting_functions_emit_the_same_records() {
    // VS-07 AC 2 (§22.6, §19): `log`, `require`, `cover` and `target` take a `str` and write the records the
    // standard library gave them: the text fields stay on the site, the values are the call's.
    let records = RUN.records_of("reports.msl", "ReportBench");
    let first = |stream: &str| records.iter().filter(|r| r.stream == stream).map(|r| r.fields.clone()).collect::<Vec<_>>();
    let text = |value: &str| Field::Text(value.to_string());
    let field = |name: &str, value: Field| (name.to_string(), value);
    assert_eq!(first("log")[0], vec![field("severity", Field::Variant("Warning".to_string())), field("msg", text("hello"))], "log: Severity::Warning is read by its name (CV-22)");
    let checks = first("checks");
    assert_eq!(checks[0], vec![field("kind", text("require")), field("name", text("held")), field("pass", Field::Bool(true))]);
    assert_eq!(checks[1], vec![field("kind", text("cover")), field("name", text("seen")), field("pass", Field::Bool(true))]);
    assert_eq!(checks[2], vec![field("kind", text("target")), field("name", text("margin")), field("margin", Field::Real(0.25))]);
}

/// A pattern of four levels written as `0`, `1` and `x` characters, the leftmost the highest level.
struct LevelPattern;

#[allow(dead_code)] // the oracle of `logic_and_matches_bits_give_the_table_values`, which T45 writes (suite_hygiene `RemovedUnknownForms::SUCCESSORS`)
impl LevelPattern {
    /// The value of a pattern of `0` and `1` characters.
    fn value(pattern: &str) -> i128 {
        pattern.chars().fold(0, |value, level| value * 2 + i128::from(level == '1'))
    }

    /// Whether `value` (four levels) matches `pattern`, an `x` matching any level (§22.1.4).
    fn matches(value: u8, pattern: &str) -> bool {
        pattern.chars().enumerate().all(|(place, level)| level == 'x' || (value >> (3 - place)) & 1 == u8::from(level == '1'))
    }
}

