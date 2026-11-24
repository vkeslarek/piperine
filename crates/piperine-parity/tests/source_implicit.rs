//! The run settings run from MSL source on the Rust backend through the host library (generics phase D, GN-24 to
//! GN-26): `temperature()` and the stdlib `multiplicity()` (the session parameter `multiplicity`) are read by the
//! model and nothing is scaled by the system. Each bench is a module of `tests/implicit/runs.msl` (or `placed.msl` for the refused one); every expected value is
//! the same expression evaluated by Rust here, from the criterion it cites.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Keeps nothing: these tests read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// The runs of this target: one module of `tests/implicit/runs.msl` per bench.
struct Bench;

impl Bench {
    /// The relative bound an operating point stops at: `reltol` 1e-3, doubled for the sum of two readings.
    const RELATIVE: f64 = 2e-3;

    /// The compile cache the runs share.
    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The session of the bench `top` under `overrides`, or the failure that refused it. The project it opened is kept
    /// alive by the session.
    fn session(top: &str, overrides: Overrides) -> Result<Session, Failure> {
        let name = if top == "PlacedTemperature" { "placed.msl" } else { "runs.msl" };
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/implicit").join(name);
        let project = Project::file(&file)?;
        let overrides = Overrides { top: Some(top.into()), print: vec!["o".into()], ..overrides };
        let spec = project.request(None, &overrides)?;
        Session::open(&project, &spec, &Self::settings())
    }

    /// The codes of the diagnostics that refused the bench `top`, whether at the open or at the run.
    fn refusal(top: &str, overrides: Overrides) -> Vec<String> {
        let failure = match Self::session(top, overrides) {
            Err(failure) => failure,
            Ok(mut session) => match session.run(Rc::new(RefCell::new(Ignore))) {
                Err(failure) => failure,
                Ok(_) => panic!("the bench {top} ran"),
            },
        };
        match failure {
            Failure::Diagnostics { found, .. } => found.iter().map(|found| found.diagnostic.code.to_string()).collect(),
            other => panic!("the bench {top} was refused by {other:#}, not by a diagnostic"),
        }
    }

    /// Every real value of the net `Top.o`'s rows in the table of the run of the bench `top`.
    fn rows(top: &str, overrides: Overrides) -> Vec<f64> {
        let mut session = Self::session(top, overrides).unwrap_or_else(|f| panic!("{f:#}"));
        let table: Table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        table.rows.iter().filter_map(|row| row.iter().rev().find_map(|c| if let Cell::Real(v) = c { Some(*v) } else { None })).collect()
    }

    /// The value of `o` at the end of the run of the bench `top`.
    fn output(top: &str, overrides: Overrides) -> f64 {
        *Self::rows(top, overrides).last().unwrap_or_else(|| panic!("no row for {top}"))
    }

    /// An operating point.
    fn op() -> Overrides {
        Overrides { analysis: Some("op".into()), ..Overrides::default() }
    }

    /// An operating point of a run whose `multiplicity` session parameter is `value`.
    fn with_multiplicity(value: f64) -> Overrides {
        Overrides { simparam: vec![("multiplicity".into(), value)], ..Self::op() }
    }

    /// Whether `actual` is `expected` to the solver's stopping bound.
    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() <= Self::RELATIVE * expected.abs() + 1e-6
    }

    /// The divider of one volt over a resistor of effective multiplicity `count` and a fixed one: the node's value.
    fn divider(count: f64) -> f64 {
        let (top, bottom) = (1000.0 / count, 1000.0);
        bottom / (top + bottom)
    }
}

#[test]
fn an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel() {
    // GN-24 AC 4 (§8.5.5): `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)`: placed with `m: Some(4.0)` it is four
    // resistors in parallel, whatever the run says (its own `m` wins); with no `m` it follows the run's one
    // multiplicity; with nothing set it is one
    let own = Bench::output("OwnMultiplicity", Bench::op());
    assert!(Bench::close(own, Bench::divider(4.0)), "`m: Some(4.0)` is four in parallel: {own}");
    let own_over_run = Bench::output("OwnMultiplicity", Bench::with_multiplicity(3.0));
    assert!(Bench::close(own_over_run, Bench::divider(4.0)), "its own `m` wins over the run's: {own_over_run}");
    let followed = Bench::output("RunMultiplicity", Bench::with_multiplicity(3.0));
    assert!(Bench::close(followed, Bench::divider(3.0)), "no `m`: the run's multiplicity 3: {followed}");
    let alone = Bench::output("RunMultiplicity", Bench::op());
    assert!(Bench::close(alone, Bench::divider(1.0)), "nothing set: one resistor: {alone}");
    // GN-24 AC 5 (DQ1): a second resistor of the same run with no `m` reads the same run multiplicity: two of effective
    // multiplicity 2 over one kilo-ohm fixed resistor
    let top = 1000.0 / 2.0;
    let (bottom, extra) = (1000.0 / 2.0, 1000.0);
    let parallel = bottom * extra / (bottom + extra);
    let two = Bench::output("TwoResistors", Bench::with_multiplicity(2.0));
    assert!(Bench::close(two, parallel / (top + parallel)), "both read the one run multiplicity: {two}");
}

#[test]
fn multiplicity_in_a_param_default_is_e0321() {
    // GN-24 AC 4 (§8.5.5, §8.1.16): a param default is an expression over params and constants, where a session
    // parameter is not known: the refusal is `E0321` (`E0188` is the code of a `const` item, §3.9; a param default has its own)
    assert_eq!(Bench::refusal("DefaultReadsMultiplicity", Bench::op()), ["E0321"]);
}

#[test]
fn nothing_set_reads_none() {
    // GN-24 AC 1, GN-25 AC 1: `temperature()` and `multiplicity()` are `None` when the run sets neither (read as -1 each:
    // -1 + 10 * -1), and `Some` of the run's values when it sets them (75 C is 348.15 K, multiplicity 2)
    let unset = Bench::output("Absent", Bench::op());
    assert!(Bench::close(unset, -1.0 + 10.0 * -1.0), "neither set: both `None`: {unset}");
    let both = Overrides { temp: Some(75.0), simparam: vec![("multiplicity".into(), 2.0)], ..Bench::op() };
    let set = Bench::output("Absent", both);
    assert!(Bench::close(set, 348.15 + 10.0 * 2.0), "both set: both `Some`: {set}");
}

#[test]
fn a_model_chooses_how_to_use_its_temperature() {
    // GN-25 AC 1, AC 2: three models given the same run read it all along, as an initial state, or ignore it; each
    // value is the one Rust computes from the run's kelvin (75 C is 348.15 K, no `temp` is the ambient 300.15 K)
    let hot = Overrides { temp: Some(75.0), ..Bench::op() };
    for (top, expected_hot, expected_unset) in [("AllAlong", 0.34815, 0.30015), ("InitialState", 0.34815, 0.30015), ("Ignored", 1.0, 1.0)] {
        let given = Bench::output(top, hot.clone());
        assert!(Bench::close(given, expected_hot), "{top} with `temp` 75: {given}");
        let unset = Bench::output(top, Bench::op());
        assert!(Bench::close(unset, expected_unset), "{top} with no `temp`: {unset}");
    }
}

#[test]
fn the_run_settings_hold_for_the_whole_run() {
    // GN-26 AC 1, AC 2: the temperature and the multiplicity are set before the run starts and hold during it: a transient
    // reads one value at every point; and neither has a placement spelling (`E0320`, no param)
    let transient = Overrides { analysis: Some("transient".into()), stop: Some(1e-6), max_step: Some(0.1e-6), temp: Some(75.0), ..Overrides::default() };
    let rows = Bench::rows("AllAlong", transient);
    assert!(rows.len() > 2 && rows.iter().all(|value| Bench::close(*value, 0.34815)), "{rows:?}");
    assert_eq!(Bench::refusal("PlacedTemperature", Bench::op()), ["E0320"]);
}
