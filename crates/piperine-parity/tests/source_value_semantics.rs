//! Places and copies run from MSL source on the Rust backend (feature `value-semantics`, VS-03,
//! VS-04, VS-07): one test per run-time criterion, each model written in `tests/value_semantics/`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites, never read back from the
//! model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Record, Streams};
use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{build, transient};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// How far the solver may move a value it was given as a current into 1 ohm: its `gmin` and
/// round-off, not a tolerance of the rules (a reaction's values are bit for bit).
const SOLVE: f64 = 1e-9;

/// The lone file `file` of `tests/value_semantics/`, as a project.
fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/value_semantics").join(file);
    Project::new(&path)
}

/// The bytes of a known `f64` carrier value: the value, then the known byte.
fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

/// The bytes of a known one-byte carrier value (`u8`, `bool`).
fn byte(v: u8) -> Vec<u8> {
    vec![v, 1]
}

/// What `Bench` of `file` committed at its first rising edge (2.5 ns of a 200 MHz oscillator): the
/// bytes of each carrier in `names`, which must change exactly once.
fn first_commits(file: &str, names: &[&str]) -> Vec<Vec<u8>> {
    let seen = build(&project(file), "Bench").run(10e-9, 1e-9);
    names
        .iter()
        .map(|name| {
            let changes = transient(&seen, &format!("Bench.{name}"));
            assert_eq!(changes.len(), 1, "{name} changes once: {changes:?}");
            assert_eq!(changes[0].0, 5 * NS / 2, "{name} changes at the first rising edge");
            changes[0].1.clone()
        })
        .collect()
}

/// The operating point of `top` of `file`: the voltage of each node in `names`.
fn operating_point(file: &str, top: &str, names: &[&str]) -> Vec<f64> {
    let mut running = project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    names
        .iter()
        .map(|name| {
            let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
            solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
        })
        .collect()
}

/// The sequence of `places.msl`'s integer operators, in Rust.
fn integer_operators(start: u8) -> u8 {
    let mut n = start;
    n += 3;
    n -= 1;
    n *= 2;
    n /= 3;
    n %= 5;
    n &= 7;
    n |= 8;
    n ^= 5;
    n <<= 1;
    n >>= 1;
    n
}

/// The sequence of `places.msl`'s real operators, in Rust.
fn real_operators(start: f64) -> f64 {
    let mut x = start;
    x += 1.5;
    x -= 0.25;
    x *= 3.0;
    x /= 2.0;
    x
}

#[test]
fn a_let_without_mut_runs_in_a_function_a_reaction_and_analog() {
    // VS-03 AC 1 (§1.5.12): `x = e` and every compound operator on a `let` with no marker, in a function, in a
    // reaction (bit for bit) and in an `analog` block (after the solve); a name a `match` arm or an `if let`
    // binds is assigned too (AC 3).
    let names = ["f", "g", "local_n", "in_arm", "in_if"];
    let committed = first_commits("places.msl", &names);
    let wanted = [real_bytes(real_operators(2.0)), byte(integer_operators(20)), byte(integer_operators(20)), byte(4 + 1), byte(4 * 3)];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
    let currents = operating_point("places.msl", "AnalogBench", &["a", "b"]);
    let analog = [real_operators(2.0), 20.0f64 - 7.0 + 1.0];
    for (k, want) in analog.iter().enumerate() {
        assert!((currents[k] - want).abs() <= SOLVE * want.abs().max(1.0), "analog node {k}: {want:?}, the operating point has {:?}", currents[k]);
    }
}

#[test]
fn an_assigned_loop_variable_does_not_change_the_loop() {
    // VS-03 AC 3, VS-12 (§1.6.9): `for i in 0..n { i += 10; .. }` changes the copy the body names, not the
    // loop: the body runs for 0, 1, 2 and 3, and sees each of them plus ten.
    let committed = first_commits("places.msl", &["loop_sum", "loop_again"]);
    let sum: u8 = (0..4u8).map(|i| i + 10).sum();
    assert_eq!(committed[0], byte(sum), "the body of shifted_sum sees 10, 11, 12, 13");
    let steps = (0..4u8).count() as u8;
    assert_eq!(committed[1], byte(steps), "the loop counts 0 to 3 whatever the body does to its variable");
}

#[test]
fn a_closure_keeps_the_value_it_captured() {
    // VS-04 AC 1 (§1.5.14): the closure holds the value where it was written; the later assignment does not
    // reach it, and the local itself changes.
    let committed = first_commits("places.msl", &["kept", "latest"]);
    let captured = 3u8;
    let plus = move |x: u8| x + captured;
    let local = 10u8;
    assert_eq!(committed[0], byte(plus(1)), "the closure read 3");
    assert_eq!(committed[1], byte(local), "the local reads 10");
}

#[test]
fn a_callee_cannot_change_the_callers_local() {
    // VS-04 AC 2 (§1.4.6, §3.1.10): the argument is a copy; a `let` of it assigned in the callee leaves the
    // caller's local as it was.
    let committed = first_commits("places.msl", &["callee", "caller"]);
    assert_eq!(committed[0], byte(5 + 1), "the callee returns its own changed copy");
    assert_eq!(committed[1], byte(5), "the caller's local is as it was");
}

#[test]
fn a_value_is_read_twice() {
    // VS-04 AC 3 (§1.4.6): a value of any type is read twice with no move; a copy that is assigned leaves the
    // other reads of the original as they were.
    let committed = first_commits("places.msl", &["twice", "copied"]);
    assert_eq!(committed[0], byte(7 + 7), "x is read twice by read_twice");
    assert_eq!(committed[1], byte(6 + 5 + 5), "p changed to 6; q and a are 5");
}

#[test]
fn an_enum_without_copy_is_carried_and_compared() {
    // VS-05 AC 2 (§1.4.6, H.8): an enum that derives `Repr` and `PartialEq` and not `Clone` or `Copy` is carried
    // on a signal, compared with `==` and copied into a second local, as before: `Mode::Run` is variant 1 of
    // three, equal to its copy and not to `Mode::Hold`.
    let committed = first_commits("enums.msl", &["m", "same", "differs"]);
    let variant_run = 1u8;
    assert_eq!(committed[0], byte(variant_run), "the carried enum is its variant");
    assert_eq!(committed[1], byte((variant_run == variant_run) as u8), "a value equals its copy");
    assert_eq!(committed[2], byte((variant_run == 2) as u8), "Run is not Hold");
}

#[test]
fn a_string_is_passed_and_compared_by_value() {
    // VS-07 AC 1 (§1.2.9): a `str` is passed to a function, held by a `let` and a `const`, and compared with `==`
    // and `!=`; the comparison is the one Rust makes of the same two strings.
    let names = ["same", "other", "named", "held", "unlike"];
    let committed = first_commits("strings.msl", &names);
    let wanted = [u8::from("a" == "a"), u8::from("a" == "b"), u8::from("ab" == "ab"), u8::from("held" == "held"), u8::from("x" != "y")];
    for (k, label) in names.iter().enumerate() {
        assert_eq!(committed[k], byte(wanted[k]), "{label} is the comparison Rust makes");
    }
}

/// Every record of the 4 ns transient of `top` of `file`, decoded as the host decodes it.
fn records_of(file: &str, top: &str) -> Vec<Record> {
    let built = build(&project(file), top);
    let streams = Streams::of(built.ir.clone());
    let seen = built.run(4e-9, 0.5e-9);
    seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect()
}

#[test]
fn the_reporting_functions_emit_the_same_records() {
    // VS-07 AC 2 (§22.6, §19): `log`, `require`, `cover` and `target` take a `str` and write the records the
    // standard library gave them: the text fields stay on the site, the values are the call's.
    let records = records_of("reports.msl", "ReportBench");
    let first = |stream: &str| records.iter().filter(|r| r.stream == stream).map(|r| r.fields.clone()).collect::<Vec<_>>();
    let text = |value: &str| Field::Text(value.to_string());
    let field = |name: &str, value: Field| (name.to_string(), value);
    assert_eq!(first("log")[0], vec![field("severity", Field::Variant("Warning".to_string())), field("msg", text("hello"))], "log: Severity::Warning is read by its name (CV-22)");
    let checks = first("checks");
    assert_eq!(checks[0], vec![field("kind", text("require")), field("name", text("held")), field("pass", Field::Bool(true))]);
    assert_eq!(checks[1], vec![field("kind", text("cover")), field("name", text("seen")), field("pass", Field::Bool(true))]);
    assert_eq!(checks[2], vec![field("kind", text("target")), field("name", text("margin")), field("margin", Field::Real(0.25))]);
}

/// The value of a four-level pattern of `0` and `1` characters, the leftmost the highest level.
fn pattern_value(pattern: &str) -> i128 {
    pattern.chars().fold(0, |value, level| value * 2 + i128::from(level == '1'))
}

/// Whether `value` (four levels) matches `pattern`, an `x` matching any level (§22.1.4).
fn pattern_matches(value: u8, pattern: &str) -> bool {
    pattern.chars().enumerate().all(|(place, level)| level == 'x' || (value >> (3 - place)) & 1 == u8::from(level == '1'))
}

#[test]
fn the_library_items_that_took_borrows_give_the_same_results() {
    // VS-08 AC 4 (§22.1.4): `logic` and `matches_bits` take their pattern as a `str` and give the values of the
    // table: `logic("1010")` is 10, `matches_bits` lets `x` match any level.
    let records = records_of("library.msl", "LibraryBench");
    let first = |stream: &str| records.iter().find(|r| r.stream == stream).map(|r| r.fields.clone()).unwrap_or_else(|| panic!("a `{stream}` record"));
    let field = |name: &str, value: Field| (name.to_string(), value);
    assert_eq!(first("logic"), vec![field("ones", Field::Int(pattern_value("1010"))), field("zeros", Field::Int(pattern_value("0000"))), field("bit", Field::Bool(true))]);
    assert_eq!(first("matches"), vec![field("exact", Field::Bool(pattern_matches(5, "0101"))), field("wild", Field::Bool(pattern_matches(5, "01xx"))), field("wrong", Field::Bool(pattern_matches(5, "0111")))]);
}
