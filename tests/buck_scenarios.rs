//! Scope: the buck converter's SPEC §10 scenarios S2 to S10 under a Rust host (feature
//! `buck-scenarios`, BS-01 to BS-09; MD-73). The scenario package `tests/e2e/buck/` holds a top
//! `Bench` over `examples/buck` and five runs, one per family of scenarios; the host drives the
//! converter's ports and sets its params during a run (`Session::drive`, `Session::set_at`) and
//! judges what the run printed against SPEC's own pass criteria. The measurements and the
//! stimulus are `piperine_parity::scenario`'s. This target holds the package's own tests; every
//! tolerance a later test judges is SPEC's, never read off a run.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use piperine::{Analysis, Overrides, ParamValue, Project, Session};
use piperine_transpile::Settings;

/// The scenario package and what its tests ask of it.
struct Package;

impl Package {
    /// The five runs, one per family of scenarios (spec "Decisions taken" D4).
    const RUNS: [&'static str; 5] = ["brownout", "faults", "load_line", "setpoint", "steady_state"];

    /// The signals every run prints (BS-01 AC 2), relative to the top `Bench`.
    const PRINTED: [&'static str; 13] = [
        "buck.vout",
        "buck.vin",
        "buck.isense",
        "buck.ctl.reference",
        "buck.code",
        "mode",
        "fault",
        "pgood",
        "buck.hs_cmd",
        "buck.ls_cmd",
        "buck.oc",
        "buck.sink_oc",
        "buck.ctl.cut",
    ];

    /// The directory of the package.
    fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/buck")
    }

    /// The package opened.
    fn project() -> Project {
        Project::at(&Package::dir()).unwrap_or_else(|failure| panic!("{failure}"))
    }

    /// A session of the run `run`, built into `cache`.
    fn open(run: &str, cache: &Path) -> Session {
        let project = Package::project();
        let spec = project.request(Some(run), &Overrides::default()).unwrap_or_else(|failure| panic!("{run}: {failure}"));
        Session::open(&project, &spec, &Settings { rustc: None, cache: cache.to_path_buf() }).unwrap_or_else(|failure| panic!("{run}: {failure}"))
    }

    /// Every file under `dir`, relative to it.
    fn files(dir: &Path) -> BTreeSet<PathBuf> {
        let mut found = BTreeSet::new();
        let mut pending = vec![dir.to_path_buf()];
        while let Some(next) = pending.pop() {
            for entry in std::fs::read_dir(&next).unwrap_or_else(|error| panic!("{}: {error}", next.display())) {
                let path = entry.expect("a directory entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    found.insert(path.strip_prefix(dir).expect("under the package").to_path_buf());
                }
            }
        }
        found
    }
}

#[test]
fn the_package_declares_the_five_scenario_runs() {
    // BS-01 AC 1: the manifest depends on `examples/buck` by path and declares exactly these runs
    let manifest = std::fs::read_to_string(Package::dir().join("Piperine.toml")).expect("the manifest");
    assert!(manifest.contains("buck = { path = \"../../../examples/buck\" }"), "{manifest}");
    let mut declared: Vec<String> = Package::project().runs().iter().map(|run| run.name.clone()).collect();
    declared.sort();
    assert_eq!(declared, Package::RUNS);
}

#[test]
fn every_run_samples_the_thirteen_signals_on_a_10_ns_grid() {
    // BS-01 AC 2: top `Bench`, a transient with `max_step` and `print_step` 1e-8, the `checks`
    // stream, the step budget of spec D14 and the print list
    let project = Package::project();
    for run in Package::RUNS {
        let spec = project.request(Some(run), &Overrides::default()).unwrap_or_else(|failure| panic!("{run}: {failure}"));
        assert_eq!(spec.top.as_deref(), Some("Bench"), "{run}");
        match spec.analysis {
            Analysis::Transient { max_step, print_step, .. } => assert_eq!((max_step, print_step), (Some(1e-8), Some(1e-8)), "{run}"),
            other => panic!("{run}: a transient, found {other:?}"),
        }
        assert_eq!(spec.streams, ["checks"], "{run}");
        assert_eq!(spec.options.max_steps, Some(15_000_000), "{run}");
        assert_eq!(spec.print, Package::PRINTED, "{run}");
    }
}

#[test]
fn every_run_accepts_the_five_ports_and_three_params() {
    // BS-01 AC 3: a drive of `en`, `sp_wr`, `sp_data`, `load_step` and `inject` at 0, and a timed
    // set of `buck.vin_nom`, `buck.r_load` and `buck.r_step` at 1 us; the run is opened, not run
    let cache = tempfile::tempdir().expect("a build cache");
    for run in Package::RUNS {
        let mut session = Package::open(run, cache.path());
        let ports = [("en", ParamValue::Bool(false)), ("sp_wr", ParamValue::Bool(false)), ("sp_data", ParamValue::Int(100)), ("load_step", ParamValue::Bool(false)), ("inject", ParamValue::Bool(false))];
        for (port, value) in ports {
            session.drive(port, value, 0.0).unwrap_or_else(|failure| panic!("{run}: the drive of `{port}`: {failure}"));
        }
        for (param, value) in [("buck.vin_nom", 12.0), ("buck.r_load", 6.6), ("buck.r_step", 0.3)] {
            session.set_at(param, value, 1e-6).unwrap_or_else(|failure| panic!("{run}: the set of `{param}`: {failure}"));
        }
    }
}

#[test]
fn the_scenario_package_checks_with_no_diagnostic() {
    // BS-01 AC 4: every run elaborates with no diagnostic
    let checked = Package::project().check().unwrap_or_else(|failures| panic!("{}", failures.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")));
    assert_eq!((checked.package.as_str(), checked.runs, checked.typed), ("buck_scenarios", Package::RUNS.len(), true));
}

#[test]
fn opening_a_run_leaves_only_the_package_sources() {
    // BS-01 AC 6: no lock file and no build output lands in the tree
    let cache = tempfile::tempdir().expect("a build cache");
    let session = Package::open("setpoint", cache.path());
    let expected: BTreeSet<PathBuf> = ["Piperine.toml", "src/lib.msl"].map(PathBuf::from).into();
    assert_eq!(Package::files(&Package::dir()), expected);
    drop(session);
}
