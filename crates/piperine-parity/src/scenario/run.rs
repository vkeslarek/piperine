//! [`ScenarioRun`] and [`ScenarioBatch`]: runs of a scenario package, each executed once under its
//! stimulus, several at the same time.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use piperine_api::{Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

use super::stimulus::Stimulus;

/// The `checks` records of a run: how many arrived and how many did not pass (SPEC
/// `examples/buck/SPEC.md` §10 S10: the monitors check at every clock, so a run that holds none
/// proves nothing).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CheckTally {
    /// The `checks` records seen.
    pub seen: usize,
    /// Those that are not passing checks.
    pub failed: usize,
}

impl RecordSink for CheckTally {
    fn record(&mut self, record: &Record) {
        if record.stream == "checks" {
            self.seen += 1;
            if !record.is_passing_check() {
                self.failed += 1;
            }
        }
    }
}

/// Every record a run delivers, in delivery order, with the tally of its `checks` (a [`RecordSink`]).
#[derive(Debug, Default)]
pub struct RunRecords {
    /// The records of every stream, as delivered.
    records: Vec<Record>,
    /// The `checks` among them.
    checks: CheckTally,
}

impl RecordSink for RunRecords {
    fn record(&mut self, record: &Record) {
        self.checks.record(record);
        self.records.push(record.clone());
    }
}

/// What one run left.
#[derive(Debug)]
pub struct CompletedRun {
    /// The name of the run.
    pub run: String,
    /// The rows the run printed.
    pub table: Table,
    /// Its `checks` records.
    pub checks: CheckTally,
    /// Every record its streams delivered, in delivery order (the `checks` included).
    pub records: Vec<Record>,
    /// The transient steps the solver accepted over the run (0 when it ran no transient).
    pub accepted_steps: u64,
    /// When the run began to open (build included).
    pub started: Instant,
    /// When the run ended.
    pub ended: Instant,
    /// The build directory the run used, its own and gone by the time the run is returned.
    pub build: PathBuf,
}

impl CompletedRun {
    /// How long the run took, from opening to its end.
    pub fn wall(&self) -> Duration {
        self.ended - self.started
    }
}

/// One named run of a scenario package under one stimulus.
#[derive(Debug, Clone)]
pub struct ScenarioRun {
    /// The directory of the scenario package.
    package: PathBuf,
    /// The name of the run, as the package declares it.
    run: String,
    /// What the host does to the session during the run.
    stimulus: Stimulus,
}

impl ScenarioRun {
    /// The run `run` of the package at `package`, under `stimulus`.
    pub fn new(package: &Path, run: &str, stimulus: Stimulus) -> ScenarioRun {
        ScenarioRun { package: package.to_path_buf(), run: run.to_string(), stimulus }
    }

    /// Opens the run in a build directory of its own, applies the stimulus, runs it with a
    /// [`CheckTally`] and returns what it left. Every failure becomes a message naming the run.
    pub fn execute(&self) -> Result<CompletedRun, String> {
        let started = Instant::now();
        let fail = |stage: &str, what: &dyn std::fmt::Display| format!("run `{}`: {stage}: {what}", self.run);
        let build = tempfile::tempdir().map_err(|error| fail("build directory", &error))?;
        let project = Project::at(&self.package).map_err(|failure| fail("project", &failure))?;
        let spec = project.request(Some(&self.run), &Overrides::default()).map_err(|failure| fail("request", &failure))?;
        let settings = Settings { rustc: None, cache: build.path().to_path_buf() };
        let mut session = Session::open(&project, &spec, &settings).map_err(|failure| fail("open", &failure))?;
        self.stimulus.apply(&mut session).map_err(|refusal| fail("stimulus", &refusal))?;
        let delivered = Rc::new(RefCell::new(RunRecords::default()));
        let table = session.run(delivered.clone()).map_err(|failure| fail("run", &failure))?;
        let accepted_steps = session.statistics().map_or(0, |statistics| statistics.steps().accepted());
        // the session holds the build's libraries: it goes before the directory does
        drop(session);
        let RunRecords { records, checks } = std::mem::take(&mut *delivered.borrow_mut());
        Ok(CompletedRun { run: self.run.clone(), table, checks, records, accepted_steps, started, ended: Instant::now(), build: build.path().to_path_buf() })
    }
}

/// A set of runs executed at once, one thread each, so the wall time of the set is that of its
/// longest run and not their sum.
#[derive(Debug)]
pub struct ScenarioBatch {
    /// The runs, executed at once, one thread each.
    runs: Vec<ScenarioRun>,
}

impl ScenarioBatch {
    /// A batch of `runs`.
    pub fn new(runs: Vec<ScenarioRun>) -> ScenarioBatch {
        ScenarioBatch { runs }
    }

    /// Executes every run once, in parallel, and returns the results in the order the runs were
    /// given. A run that fails, or whose thread panics, is its message; the others are
    /// unaffected, so a test of a healthy run never waits on a failed one.
    pub fn execute(self) -> Vec<Result<CompletedRun, String>> {
        std::thread::scope(|scope| {
            let threads: Vec<_> = self.runs.iter().map(|run| scope.spawn(move || run.execute())).collect();
            threads
                .into_iter()
                .zip(&self.runs)
                .map(|(thread, run)| thread.join().unwrap_or_else(|payload| Err(format!("run `{}`: panicked: {}", run.run, ScenarioBatch::panic_text(&payload)))))
                .collect()
        })
    }

    /// The text of a panic payload.
    fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
        payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string())).unwrap_or_else(|| "no message".to_string())
    }
}

#[cfg(test)]
mod tests {
    use piperine_api::{Field, ParamValue};

    use super::*;
    use crate::scenario::planted::PlantedPackage;

    /// A check record that did or did not pass, or a record of another stream.
    struct Records;

    impl Records {
        /// A record of `stream` with the field `pass`, when given.
        fn of(stream: &str, pass: Option<bool>) -> Record {
            Record { time: 0.0, instance: "top.mon".into(), stream: stream.into(), fields: pass.map(|flag| ("pass".to_string(), Field::Bool(flag))).into_iter().collect(), maybe: false }
        }
    }

    #[test]
    fn the_tally_counts_checks_and_the_failed_ones() {
        let mut tally = CheckTally::default();
        for record in [
            Records::of("checks", Some(true)),
            Records::of("checks", Some(true)),
            Records::of("checks", Some(false)),
            // a check with no `pass` is not passing
            Records::of("checks", None),
            // another stream is not a check
            Records::of("edge", Some(false)),
        ] {
            tally.record(&record);
        }
        assert_eq!(tally, CheckTally { seen: 4, failed: 2 });
    }

    #[test]
    fn a_failed_run_is_kept_as_its_message_and_the_others_still_run() {
        let package = PlantedPackage::write();
        let driven = Stimulus::new().drive("level", ParamValue::Int(3), 0.0);
        let results = ScenarioBatch::new(vec![
            ScenarioRun::new(package.path(), "short", driven.clone()),
            ScenarioRun::new(package.path(), "absent", Stimulus::new()),
            ScenarioRun::new(package.path(), "short", Stimulus::new().drive("nothing", ParamValue::Bool(true), 0.0)),
        ])
        .execute();
        let ran = results[0].as_ref().unwrap_or_else(|message| panic!("{message}"));
        assert_eq!(ran.run, "short");
        assert_eq!(ran.table.rows.len(), 5, "a 2 us run printed every 0.5 us");
        assert!(results[1].as_ref().is_err_and(|message| message.contains("`absent`")), "{:?}", results[1]);
        assert!(results[2].as_ref().is_err_and(|message| message.contains("stimulus") && message.contains("E0801")), "{:?}", results[2]);
    }

    #[test]
    fn a_completed_run_keeps_every_record_in_delivery_order() {
        // the clock of `ticks` rises at 0.5, 1.5, 2.5 and 3.5 us (first change at half its period of
        // 1 us); its counter emits `tick` with the edge it counted, 0 to 3, and no run emits a check
        let package = PlantedPackage::write();
        let results = ScenarioBatch::new(vec![ScenarioRun::new(package.path(), "ticks", Stimulus::new())]).execute();
        let ran = results[0].as_ref().unwrap_or_else(|message| panic!("{message}"));
        assert_eq!(ran.records.len(), 4, "{:?}", ran.records);
        for (index, record) in ran.records.iter().enumerate() {
            assert_eq!(record.stream, "tick");
            assert_eq!(record.fields, vec![("edge".to_string(), Field::Int(index as i128))], "record {index}");
            assert!((record.time - (index as f64 + 0.5) * 1e-6).abs() < 1e-12, "record {index} at {}", record.time);
        }
        assert_eq!(ran.checks, CheckTally::default(), "no stream of the run is `checks`");
    }

    #[test]
    fn each_run_builds_in_its_own_directory() {
        let package = PlantedPackage::write();
        let results = ScenarioBatch::new(vec![
            ScenarioRun::new(package.path(), "short", Stimulus::new()),
            ScenarioRun::new(package.path(), "twin", Stimulus::new()),
        ])
        .execute();
        let [first, second] = results.as_slice() else { panic!("two results") };
        let (first, second) = (first.as_ref().unwrap_or_else(|m| panic!("{m}")), second.as_ref().unwrap_or_else(|m| panic!("{m}")));
        assert_ne!(first.build, second.build);
        assert!(first.started < second.ended && second.started < first.ended, "the runs overlap: {:?} {:?}", first.wall(), second.wall());
    }
}
