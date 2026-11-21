//! The scenario package and the one batch that runs its designs: [`IntegratedPackage`] says where
//! the package is and what it declares, [`Suite`] runs every scenario once, at once, behind a
//! `OnceLock`, and [`Scenario`] is what each scenario module gives the suite.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use piperine::{Overrides, Project, Session};
use piperine_parity::scenario::{CompletedRun, ScenarioBatch, ScenarioRun, Stimulus};
use piperine_transpile::Settings;

/// The scenario package `tests/e2e/integrated/` and what its tests ask of it.
pub struct IntegratedPackage;

impl IntegratedPackage {
    /// The runs the package declares so far, sorted by name; a scenario adds its own.
    pub const RUNS: [&'static str; 0] = [];

    /// The directory of the package.
    pub fn dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/integrated")
    }

    /// The package opened.
    pub fn project() -> Project {
        Project::at(&IntegratedPackage::dir()).unwrap_or_else(|failure| panic!("{failure}"))
    }

    /// A session of the run `run`, built into `cache`.
    pub fn open(run: &str, cache: &Path) -> Session {
        let project = IntegratedPackage::project();
        let spec = project.request(Some(run), &Overrides::default()).unwrap_or_else(|failure| panic!("{run}: {failure}"));
        Session::open(&project, &spec, &Settings { rustc: None, cache: cache.to_path_buf() }).unwrap_or_else(|failure| panic!("{run}: {failure}"))
    }

    /// Every file under `dir`, relative to it.
    pub fn files(dir: &Path) -> BTreeSet<PathBuf> {
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

/// What a scenario module gives the suite: the name of its run and what the host does during it.
// Implemented by the scenario modules, which arrive one task at a time (T11 to T15).
#[allow(dead_code)]
pub trait Scenario {
    /// The run's name, as the package's manifest declares it.
    const RUN: &'static str;

    /// What the host does to the session during the run.
    fn stimulus() -> Stimulus;
}

/// The results of every run of the package, run once by one [`ScenarioBatch`] at the first access
/// (IT-03 AC 4 and 5).
pub struct Suite {
    /// Each run's name with its completed run or the message of its failure.
    results: Vec<(&'static str, Result<CompletedRun, String>)>,
}

impl Suite {
    /// The suite; the first caller runs every scenario, the others wait for it.
    pub fn get() -> &'static Suite {
        static SUITE: OnceLock<Suite> = OnceLock::new();
        SUITE.get_or_init(|| {
            let suite = Suite::run();
            eprintln!("{}", suite.report());
            suite
        })
    }

    /// The runs of the scenarios, each with its stimulus; a scenario adds its own.
    fn plan() -> Vec<(&'static str, Stimulus)> {
        Vec::new()
    }

    /// Runs every scenario at once, each in a build directory of its own.
    fn run() -> Suite {
        let plan = Suite::plan();
        let runs = plan.iter().map(|(run, stimulus)| ScenarioRun::new(&IntegratedPackage::dir(), run, stimulus.clone())).collect();
        let names: Vec<&'static str> = plan.iter().map(|(run, _)| *run).collect();
        Suite { results: names.into_iter().zip(ScenarioBatch::new(runs).execute()).collect() }
    }

    /// The names of the runs the suite ran, in plan order.
    pub fn runs(&self) -> Vec<&'static str> {
        self.results.iter().map(|(name, _)| *name).collect()
    }

    /// The run `run`; panics with the stored message when it failed, so every test of the run
    /// reports the failure once and none runs it again (IT-03 AC 5).
    #[allow(dead_code)] // read by the scenario modules, which arrive one task at a time (T11 to T15)
    pub fn ran(&self, run: &str) -> &CompletedRun {
        match self.results.iter().find(|(name, _)| *name == run) {
            Some((_, Ok(ran))) => ran,
            Some((_, Err(message))) => panic!("{message}"),
            None => panic!("no scenario runs `{run}`"),
        }
    }

    /// Every run that completed.
    pub fn completed(&self) -> Vec<&CompletedRun> {
        self.results.iter().filter_map(|(_, result)| result.as_ref().ok()).collect()
    }

    /// One line per run (its wall time, its rows and its interval, in seconds from the first start),
    /// a line for a run that failed, and the batch's wall time (IT-03 AC 6).
    pub fn report(&self) -> String {
        let origin = self.completed().iter().map(|ran| ran.started).min();
        let mut lines: Vec<String> = self
            .results
            .iter()
            .map(|(name, result)| match (result, origin) {
                (Ok(ran), Some(origin)) => format!("scenario `{name}`: wall time {:.1} s, {} rows, ran {:.1} s to {:.1} s", ran.wall().as_secs_f64(), ran.table.rows.len(), (ran.started - origin).as_secs_f64(), (ran.ended - origin).as_secs_f64()),
                (Ok(_), None) => unreachable!("a completed run gives an origin"),
                (Err(message), _) => format!("scenario `{name}`: failed: {message}"),
            })
            .collect();
        let end = self.completed().iter().map(|ran| ran.ended).max();
        if let (Some(origin), Some(end)) = (origin, end) {
            lines.push(format!("batch: wall time {:.1} s", (end - origin).as_secs_f64()));
        }
        lines.join("\n")
    }
}
