//! Piperine against ngspice on the analog power stage of the buck (feature
//! `transient-performance`, `diagnosis.md` "Piperine vs ngspice, time per step"): an ideal supply,
//! two ideal switches driven by piecewise-linear controls, an inductor, a capacitor and a load, 100
//! us (50 switching periods) at the same 10 ns step cap, as an MSL design (`power_stage/
//! power_stage.msl`, the controls the host's sources) and as an ngspice deck (`power_stage/
//! power_stage.cir`). The target prints the accepted steps and the time per step of both and
//! asserts no time and no step count: the numbers are a measurement, read by a person. Without
//! `ngspice` on the path only Piperine's line is printed. Each run is made three times and the best
//! time is the one printed (run it with `--release` for numbers a person can quote).
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::Instant;

use piperine_api::{NodeSource, Overrides, Project, Session};
use piperine_transpile::Settings;
use piperine_parity::sinks::Ignore;

/// The switching period, the time the high side is on and the edge time of the controls, in seconds.
const PERIOD: f64 = 2e-6;
const ON_TIME: f64 = 0.2e-6;
const EDGE: f64 = 10e-9;
/// Periods run, and the stop time they make.
const PERIODS: usize = 50;
const STOP: f64 = 100e-6;

/// One run of the power stage: its accepted steps and its wall time in seconds.
#[derive(Clone, Copy, Debug)]
struct Measured {
    steps: u64,
    seconds: f64,
}

impl Measured {
    /// The wall time per accepted step, in microseconds.
    fn microseconds_per_step(&self) -> f64 {
        self.seconds / self.steps as f64 * 1e6
    }

    /// The run with the shortest wall time of `runs`; the step count of every run is the same.
    fn best(runs: impl IntoIterator<Item = Measured>) -> Option<Measured> {
        runs.into_iter().min_by(|a, b| a.seconds.total_cmp(&b.seconds))
    }
}

/// The power stage as an MSL design and as an ngspice deck.
struct PowerStage;

impl PowerStage {
    /// How many times each run is made.
    const RUNS: usize = 3;

    /// The corners of the high-side control and of its complement (10 ns of dead time at each edge), as
    /// `power_stage.cir` writes them.
    fn controls() -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let (mut high, mut low) = (Vec::new(), Vec::new());
        for k in 0..PERIODS {
            let t = k as f64 * PERIOD;
            high.extend([(t, 0.0), (t + EDGE, 1.0), (t + ON_TIME, 1.0), (t + ON_TIME + EDGE, 0.0)]);
            low.extend([(t, 1.0), (t + EDGE, 0.0), (t + ON_TIME + 2.0 * EDGE, 0.0), (t + ON_TIME + 3.0 * EDGE, 1.0)]);
        }
        high.push((STOP, 0.0));
        low.push((STOP, 1.0));
        (high, low)
    }

    /// The accepted steps and the wall time of one run of Piperine on the design.
    fn piperine_run() -> Measured {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/power_stage");
        let project = Project::file(&dir.join("power_stage.msl")).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides {
            top: Some("PowerStage".into()),
            analysis: Some("transient".into()),
            stop: Some(STOP),
            max_step: Some(10e-9),
            print: vec!["vo".into()],
            print_step: Some(1e-6),
            ..Overrides::default()
        };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let cache = tempfile::tempdir().expect("tmp");
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        let (high, low) = PowerStage::controls();
        session.source("ctl", NodeSource::dc(0.0).with_corners(high)).expect("a source on ctl");
        session.source("ctlb", NodeSource::dc(1.0).with_corners(low)).expect("a source on ctlb");
        let started = Instant::now();
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let seconds = started.elapsed().as_secs_f64();
        Measured { steps: session.statistics().expect("a transient leaves its statistics").steps().accepted(), seconds }
    }

    /// The data rows (one per accepted timepoint, no interpolation) and the analysis time of one run
    /// of ngspice on the deck, `None` when ngspice is not installed.
    fn ngspice_run() -> Option<Measured> {
        let deck = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/power_stage/power_stage.cir");
        let output = Command::new("ngspice").arg("-b").arg(&deck).output().ok()?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr);
        let number = |label: &str| -> Option<f64> {
            let line = text.lines().find(|line| line.contains(label))?;
            line.split('=').nth(1).or_else(|| line.split(':').nth(1))?.trim().parse().ok()
        };
        Some(Measured { steps: number("No. of Data Rows")? as u64, seconds: number("Total analysis time")? })
    }
}

#[test]
fn the_power_stage_time_per_step_against_ngspice_is_printed() {
    let piperine = Measured::best((0..PowerStage::RUNS).map(|_| PowerStage::piperine_run())).expect("a run");
    eprintln!("power stage, piperine: {} accepted steps in {:.3} s = {:.1} us per step (best of {})", piperine.steps, piperine.seconds, piperine.microseconds_per_step(), PowerStage::RUNS);
    match Measured::best((0..PowerStage::RUNS).filter_map(|_| PowerStage::ngspice_run())) {
        Some(ngspice) => {
            eprintln!("power stage, ngspice: {} accepted timepoints in {:.3} s = {:.1} us per step (best of {})", ngspice.steps, ngspice.seconds, ngspice.microseconds_per_step(), PowerStage::RUNS);
            eprintln!(
                "power stage, ratio: {:.2}x the steps, {:.2}x the time per step",
                piperine.steps as f64 / ngspice.steps as f64,
                piperine.microseconds_per_step() / ngspice.microseconds_per_step()
            );
        }
        None => eprintln!("power stage, ngspice: not installed, no comparison"),
    }
    assert!(piperine.steps > 0, "the run takes steps");
}
