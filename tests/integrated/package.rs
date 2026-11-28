//! The package itself (IT-03): it declares the runs, checks clean, every design compiles with no
//! warning and valid shapes, opening leaves only its sources, the runs execute at once in build
//! directories of their own, and the batch reports what it measured.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::mixed::Built;
use piperine_parity::project::Project as ParityProject;

use crate::suite::{IntegratedPackage, Suite};

/// The designs of the package as the compiler sees them.
struct Designs;

impl Designs {
    /// The top of the run `run`, from the package's manifest.
    fn top_of(run: &str) -> String {
        let project = IntegratedPackage::project();
        let spec = project.request(Some(run), &piperine::Overrides::default()).unwrap_or_else(|failure| panic!("{run}: {failure}"));
        spec.top.unwrap_or_else(|| panic!("the run `{run}` names a top"))
    }

    /// `top` of the package's root crate, elaborated and compiled once.
    fn build(top: &str) -> Built {
        let root = IntegratedPackage::dir().join("src/lib.msl");
        let ir = ParityProject::new(&root).elaborate(top).unwrap_or_else(|rejection| panic!("{rejection}"));
        Built::new(ir)
    }

    /// How many bodies of `built` run and have a shape `piperine_abi::validate` accepts; panics on one that is not.
    fn valid_shapes(built: &Built) -> usize {
        let mut shapes = 0;
        for body in built.ir.bodies() {
            let runs = body.analog().is_some() || body.nodes().iter().any(|node| node.init.is_some()) || !body.carriers().is_empty();
            if !runs {
                continue;
            }
            let plan = piperine_plan::BodyPlan::build(body).expect("the body plans");
            assert_eq!(piperine_abi::validate::Validator::validate(plan.shape().shape()), Ok(()), "the shape of `{}`", body.name());
            shapes += 1;
        }
        shapes
    }
}

#[test]
fn the_package_declares_its_runs_and_checks_clean() {
    // IT-03 AC 1: the manifest declares exactly the runs the suite has so far, and every one elaborates
    let mut declared: Vec<String> = IntegratedPackage::project().runs().iter().map(|run| run.name.clone()).collect();
    declared.sort();
    assert_eq!(declared, IntegratedPackage::RUNS);
    let checked = IntegratedPackage::project().check().unwrap_or_else(|failures| panic!("{}", failures.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")));
    assert_eq!((checked.package.as_str(), checked.runs, checked.typed), ("integrated", IntegratedPackage::RUNS.len(), !IntegratedPackage::RUNS.is_empty()));
}

#[test]
fn every_run_compiles_with_no_warning_and_valid_shapes() {
    // IT-03 AC 2: `Built::new` compiles the generated source and asserts `rustc` printed no warning;
    // every body that runs has a shape `piperine_abi::validate` accepts; the circuit binds
    for run in IntegratedPackage::RUNS {
        let built = Designs::build(&Designs::top_of(run));
        assert_eq!(built.ir.check(), vec![], "{run}: the IR passes check()");
        assert!(Designs::valid_shapes(&built) > 0, "{run}: a body that runs");
        built.compiled.circuit().unwrap_or_else(|error| panic!("{run}: {error}"));
    }
}

#[test]
fn opening_every_run_leaves_only_the_package_sources() {
    // IT-03 AC 3: no lock file and no build output lands in the tree: the manifest and one source per run
    let cache = tempfile::tempdir().expect("a build cache");
    let sessions: Vec<_> = IntegratedPackage::RUNS.iter().map(|run| IntegratedPackage::open(run, cache.path())).collect();
    let mut expected: BTreeSet<PathBuf> = ["Piperine.toml", "src/lib.msl"].map(PathBuf::from).into();
    expected.extend(IntegratedPackage::RUNS.iter().map(|run| PathBuf::from(format!("src/{run}.msl"))));
    assert_eq!(IntegratedPackage::files(&IntegratedPackage::dir()), expected);
    drop(sessions);
}

#[test]
fn the_runs_execute_at_once_in_their_own_build_directories() {
    // IT-03 AC 4: one batch, every run once, in parallel: their intervals overlap pairwise and no two
    // share a build directory
    let suite = Suite::get();
    assert_eq!(suite.runs(), IntegratedPackage::RUNS, "the suite ran the declared runs");
    let completed = suite.completed();
    assert_eq!(completed.len(), IntegratedPackage::RUNS.len(), "every run completed: {}", suite.report());
    for (index, first) in completed.iter().enumerate() {
        for second in &completed[index + 1..] {
            assert_ne!(first.build, second.build, "`{}` and `{}` share a build directory", first.run, second.run);
            assert!(first.started < second.ended && second.started < first.ended, "`{}` and `{}` ran one after the other", first.run, second.run);
        }
    }
}

#[test]
fn every_run_reports_its_wall_time_and_rows() {
    // IT-03 AC 6: one line per run with its wall time and rows, and the batch's wall time
    let suite = Suite::get();
    let report = suite.report();
    for ran in suite.completed() {
        let line = report.lines().find(|line| line.starts_with(&format!("scenario `{}`", ran.run))).unwrap_or_else(|| panic!("a line for `{}` in:\n{report}", ran.run));
        assert!(line.contains("wall time") && line.contains(&format!("{} rows", ran.table.rows.len())), "{line}");
        assert!(!ran.table.rows.is_empty(), "`{}` printed no row", ran.run);
    }
    assert_eq!(report.lines().any(|line| line.starts_with("batch: wall time")), !IntegratedPackage::RUNS.is_empty(), "{report}");
}
