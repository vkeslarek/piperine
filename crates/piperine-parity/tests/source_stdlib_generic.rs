//! The standard library items built on generics, run from MSL source on the Rust backend (feature `generics` phase E, GN-28):
//! each model is written in `tests/stdlib_generic/`, elaborated by the front end, compiled on the transpiled backend and run
//! on the solver. Every expected value is computed here by Rust from the rule of Part VII §22 the test cites, never read back
//! from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use std::path::{Path, PathBuf};

use piperine_api::{Field, Record, Streams};
use piperine_parity::project::Project;
use support::build;

/// The models of `tests/stdlib_generic/` and what they record.
struct StdModels;

impl StdModels {
    /// The project of the model file `file`.
    fn project(file: &str) -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/stdlib_generic").join(file);
        Project::new(&path)
    }

    /// Every record of the transient of `top` of `file` up to `stop` seconds at `step`, decoded as the host decodes it.
    fn records(file: &str, top: &str, stop: f64, step: f64) -> Vec<Record> {
        let built = build(&Self::project(file), top);
        let streams = Streams::of(built.ir.clone());
        let seen = built.run(stop, step);
        seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect()
    }

    /// The records of the stream `stream`, in time order.
    fn of_stream<'r>(records: &'r [Record], stream: &str) -> Vec<&'r Record> {
        records.iter().filter(|record| record.stream == stream).collect()
    }

    /// The real field `name` of `record`.
    fn real(record: &Record, name: &str) -> f64 {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Real(value))) => *value,
            other => panic!("{name} is a real: {other:?}"),
        }
    }

    /// The boolean field `name` of `record`.
    fn boolean(record: &Record, name: &str) -> bool {
        match record.fields.iter().find(|(field, _)| field == name) {
            Some((_, Field::Bool(value))) => *value,
            other => panic!("{name} is a bool: {other:?}"),
        }
    }
}

/// Rust's own reading of the §22 rules the tests cite.
struct Section22;

impl Section22 {
    /// §22.1.8: `since` is `now - t` of the last change recorded at or before `now`, `inf` while none is.
    fn since(changes: &[f64], now: f64) -> f64 {
        changes.iter().rev().find(|change| **change <= now).map_or(f64::INFINITY, |change| now - change)
    }

    /// §22.1.8 rule 2: a reaction on the change `k` reads the time since the change before it (`inf` for the first, rule 3).
    fn since_before(changes: &[f64], k: usize) -> f64 {
        Self::since(&changes[..k], changes[k])
    }

    /// §22.5.7: at each change of `d` the level that ends lasted the time since the change before; the check passes when
    /// that width is at least `w`.
    fn min_pulse_passes(changes: &[f64], w: f64) -> Vec<bool> {
        (0..changes.len()).map(|k| Self::since_before(changes, k) >= w).collect()
    }

    /// Whether two readings are the same real, `inf` included, up to the femtosecond the solver counts time in.
    fn same_reading(left: f64, right: f64) -> bool {
        left == right || (left - right).abs() < 1e-14
    }
}

#[test]
fn since_reads_the_time_from_the_last_change() {
    // §22.1.8: `d` commits its first value at (0, 0) (a change, §13 rule 6), rises at 3 ns and falls at 8 ns. A reaction on
    // each change reads the time since the one before (`inf` at the first, rules 2 and 3); a read at 2, 5 and 9 ns reads
    // `now - t_change`
    let changes = [0.0, 3e-9, 8e-9];
    let probes = [2e-9, 5e-9, 9e-9];
    let records = StdModels::records("since.msl", "SinceBench", 20e-9, 1e-9);
    let at_change = StdModels::of_stream(&records, "at_change");
    assert_eq!(at_change.len(), changes.len(), "one reading per change: {records:?}");
    for (k, record) in at_change.iter().enumerate() {
        assert!(Section22::same_reading(record.time, changes[k]), "the change {k} at {}", record.time);
        let (value, wanted) = (StdModels::real(record, "value"), Section22::since_before(&changes, k));
        assert!(Section22::same_reading(value, wanted), "since in the reaction of change {k}: {value} against {wanted}");
    }
    let read = StdModels::of_stream(&records, "age");
    assert_eq!(read.len(), probes.len(), "one reading per probe: {records:?}");
    for (record, probe) in read.iter().zip(probes) {
        assert!(Section22::same_reading(record.time, probe), "read at {probe}: {}", record.time);
        let (value, wanted) = (StdModels::real(record, "value"), Section22::since(&changes, probe));
        assert!(Section22::same_reading(value, wanted), "since at {probe}: {value} against {wanted}");
    }
}

#[test]
fn min_pulse_fails_exactly_for_a_narrow_pulse() {
    // §22.5.7: `MinPulse { w }` checks at each change of `d` that the level that just ended lasted at least `w` = 1.5 ns:
    // `d` commits at (0, 0) and changes at 2, 3, 10, 11, 15, 19 and 20 ns, so the levels last inf, 2, 1, 7, 1, 4, 4 and 1 ns
    let changes = [0.0, 2e-9, 3e-9, 10e-9, 11e-9, 15e-9, 19e-9, 20e-9];
    let records = StdModels::records("since.msl", "PulseBench", 30e-9, 1e-9);
    let checks = StdModels::of_stream(&records, "checks");
    let wanted = Section22::min_pulse_passes(&changes, 1.5e-9);
    assert_eq!(checks.len(), changes.len(), "one check per change: {records:?}");
    for ((record, change), pass) in checks.iter().zip(changes).zip(&wanted) {
        assert!(Section22::same_reading(record.time, change), "checked at {change}: {}", record.time);
        assert_eq!(StdModels::boolean(record, "pass"), *pass, "the check at {change}");
    }
    assert!(wanted.iter().any(|pass| !pass) && wanted.iter().any(|pass| *pass), "the table holds narrow and wide levels");
}
