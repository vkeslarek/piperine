//! Resolved nets run from MSL source on the Rust backend (feature `resolved-nets`, RN-03 to RN-06): each model is
//! written in `tests/resolved/`, elaborated by the front end, compiled on the transpiled backend and run on the
//! solver. Every expected value is written from the rule it cites (§4 rule 8, §5.2.7, §5.2.15, §8.2.3), with its
//! closed form beside the assertion, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Failure, Field, Overrides, Project as HostProject, Record, RecordSink, Session, Streams};
use piperine_transpile::Settings;
use piperine_parity::project::Project;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, Seen};
use support::build;

/// A sink that keeps nothing: a run is judged by what it returns.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// One MSL file of `tests/resolved/`.
struct Model {
    /// The project read from that file.
    project: Project,
}

impl Model {
    /// The project of `tests/resolved/<file>`.
    fn of(file: &str) -> Model {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/resolved").join(file);
        Model { project: Project::new(&root) }
    }

    /// The operating point of `top` in `file`, as the host runs it on the Rust backend.
    fn operating_point(file: &str, top: &str) -> Result<piperine_api::Table, Failure> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/resolved").join(file);
        let project = HostProject::file(&path).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some(top.into()), analysis: Some("op".into()), backend: Some("rust".into()), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}")).run(Rc::new(RefCell::new(Ignore)))
    }

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> Built {
        build(&self.project, top)
    }

    /// The `f64` a net of numbers holds in `bytes`.
    fn real(bytes: &[u8]) -> f64 {
        f64::from_le_bytes(bytes.try_into().expect("eight bytes"))
    }

    /// The value of the net at `path` after each change, the operating point's first, in order.
    fn values(seen: &Seen, path: &str) -> Vec<f64> {
        seen.series_of(path).into_iter().map(|(_, bytes)| Model::real(&bytes)).collect()
    }

    /// The `f64` fields of every record of the stream `name` a run wrote, the operating point's first.
    fn reals(built: &Built, seen: &Seen, name: &str) -> Vec<Vec<(String, f64)>> {
        let streams = Streams::of(built.ir.clone());
        seen.all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == name)
            .map(|r| {
                r.fields
                    .into_iter()
                    .map(|(field, value)| match value {
                        Field::Real(value) => (field, value),
                        other => panic!("no real field `{field}`: {other:?}"),
                    })
                    .collect()
            })
            .collect()
    }
}

#[test]
fn three_summing_drivers_give_their_sum_in_the_microstep_any_changes() {
    // §4 rule 8, §5.2: the drives 4, 1 and 2 combine through `sum`: the net holds 4 + 1 + 2 = 7 from the first
    // microstep, and `changed()` of a reader fires once for it, so the reader sees `total = 7`.
    let built = Model::of("drives.msl").built("ThreeBench");
    let seen = built.run(10e-9, 1e-9);
    let reader = Model::reals(&built, &seen, "changes");
    assert_eq!(reader, vec![vec![("total".to_string(), 7.0)]], "one change, to the sum of the three drives");
}

#[test]
fn an_undriven_resolved_carrier_holds_resolve_of_no_drive() {
    // §5.2.7, F.4.1: a net nobody drives holds `resolve` of no drive: the sum of none is 0, and the wired AND of
    // none is true, the identity of AND, from `(0, 0)` on.
    let built = Model::of("drives.msl").built("UndrivenBench");
    let seen = built.run(10e-9, 1e-9);
    let totals = Model::values(&seen, "UndrivenBench.t");
    assert!(!totals.is_empty() && totals.iter().all(|total| *total == 0.0), "no drive sums to 0");
    assert_eq!(seen.series_of("UndrivenBench.a").last().map(|(_, bytes)| bytes.clone()), Some(vec![1]), "the AND of no drive is true");
}

#[test]
fn resolve_receives_the_drives_in_instance_path_order() {
    // §4 rule 2 (Resolution): the drives reach `resolve` in the order of their holders' instance paths, whatever the order
    // the instances are written in: `c`, `a`, `b` holding 3, 1, 2 are passed as `[a, b, c]`, and the fold gives 123.
    let built = Model::of("drives.msl").built("OrderBench");
    let seen = built.run(10e-9, 1e-9);
    assert_eq!(Model::values(&seen, "OrderBench.n").last().copied(), Some(123.0));
}

#[test]
fn a_drive_without_initial_value_is_its_types_default_until_its_first_commit() {
    // §5.2.12: a drive with no initial value starts at the `Default` of its type: `Cell` is 7 until `Late` commits
    // `Cell { v: 2 }` at the first rising edge, 5 ns, when the net becomes 2.
    let built = Model::of("drives.msl").built("DefaultBench");
    let seen = built.run(10e-9, 1e-9);
    let levels: Vec<f64> = Model::reals(&built, &seen, "levels").into_iter().map(|record| record[0].1).collect();
    assert_eq!(levels, vec![7.0, 2.0], "the default, then the first commit");
}

#[test]
fn a_drive_change_that_leaves_the_value_fires_no_changed() {
    // §5.3: `changed()` is a change of the net's value. `Up` drives 1 then 2 and `Down` 2 then 1 at the first rise, so
    // the sum is 3 before and after: the reader sees one change, at `(0, 0)`, and none at the rise.
    let built = Model::of("drives.msl").built("SwapBench");
    let seen = built.run(10e-9, 1e-9);
    assert_eq!(Model::reals(&built, &seen, "changes"), vec![vec![("total".to_string(), 3.0)]]);
}

#[test]
fn an_initial_value_is_the_declaring_modules_first_drive() {
    // §5.2.3: `signal n: Total = 10.0` is the declaring module's drive of the net at `(0, 0)`: with two sources of 1 and 2
    // the net holds 10 + 1 + 2 = 13.
    let built = Model::of("drives.msl").built("InitialBench");
    let seen = built.run(10e-9, 1e-9);
    assert_eq!(Model::reals(&built, &seen, "changes"), vec![vec![("total".to_string(), 13.0)]]);
}

#[test]
fn others_is_resolve_of_every_drive_but_the_readers() {
    // §5.2.7: the `others()` of a drive is `resolve` of every drive but its own. With the drives 1, 2 and 4 the three
    // modules read 2 + 4 = 6, 1 + 4 = 5 and 1 + 2 = 3.
    let built = Model::of("others.msl").built("EchoBench");
    let seen = built.run(10e-9, 1e-9);
    let mut echoes: Vec<(f64, f64)> = Model::reals(&built, &seen, "others").into_iter().map(|record| (record[0].1, record[1].1)).collect();
    echoes.sort_by(|left, right| left.0.total_cmp(&right.0));
    assert_eq!(echoes, vec![(1.0, 6.0), (2.0, 5.0), (4.0, 3.0)], "each drive's own amount and the sum of the others");
}

#[test]
fn others_of_a_module_without_a_drive_is_the_value() {
    // §5.2.7: a module that drives nothing has no drive to leave out, so its `others()` is the net's value: 1 + 2 = 3.
    let built = Model::of("others.msl").built("OnlookerBench");
    let seen = built.run(10e-9, 1e-9);
    assert_eq!(Model::reals(&built, &seen, "looks"), vec![vec![("rest".to_string(), 3.0), ("whole".to_string(), 3.0)]]);
}

#[test]
fn a_loop_of_resolved_drives_is_accepted_and_settles() {
    // §8.2.3: three pass elements, each driving what the other two drive, form a loop of resolved drives. It starts from
    // nothing (`resolve` of no drive is true), every pass drives true, and it settles on true within the bound.
    let built = Model::of("relax.msl").built("RingBench");
    let seen = built.run(10e-9, 1e-9);
    assert_eq!(seen.series_of("RingBench.n").last().map(|(_, bytes)| bytes.clone()), Some(vec![1]), "the ring settles on true");
}

#[test]
fn a_released_ring_latches_nothing() {
    // §8.2.3: each relaxation starts from nothing, so a loop cannot latch. The seed pulls the net low at the rise,
    // 5 ns, and releases it at the fall, 10 ns: the ring follows both, true, false, true, and keeps no low level.
    let built = Model::of("relax.msl").built("ReleaseBench");
    let seen = built.run(12e-9, 1e-9);
    let series = seen.series_of("ReleaseBench.n");
    assert_eq!(series, vec![(0, vec![1]), (5_000_000, vec![0]), (10_000_000, vec![1])], "high, low while held, high again once released");
}

#[test]
fn a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives() {
    // §8.2.3 step 4: two drives that each drive the negation of the other never settle (0, 1, 0, 1, ...): the run stops
    // with `E0511` naming both drives by their holders' instance paths, `OscillatorBench.a` before `OscillatorBench.b`
    // (instance-path order, §4 rule 2) though `b` is written first, and no name of a drive net the author never wrote.
    match Model::operating_point("unsettled.msl", "OscillatorBench") {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0511");
            assert!(message.contains("OscillatorBench.a, OscillatorBench.b"), "{message}");
            assert!(!message.contains("$drive"), "{message}");
        }
        other => panic!("E0511, found {other:?}"),
    }
}

#[test]
fn a_loop_settling_on_its_first_pass_raises_nothing() {
    // §8.2.3: a loop that settles is no fault: two pass elements drive true at once and the run returns.
    Model::operating_point("unsettled.msl", "SettlingBench").unwrap_or_else(|f| panic!("{f}"));
}
