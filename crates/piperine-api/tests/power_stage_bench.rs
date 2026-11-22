//! Piperine against ngspice on the analog power stage of the buck (feature
//! `transient-performance`, `diagnosis.md` "Piperine vs ngspice, time per step"): an ideal supply,
//! two ideal switches driven by piecewise-linear controls, an inductor, a capacitor and a load, 100
//! us (50 switching periods) at the same 10 ns step cap, as an MSL design (`power_stage/
//! power_stage.msl`, the controls the host's sources) and as an ngspice deck (`power_stage/
//! power_stage.cir`). The target prints the accepted steps and the time per step of both and
//! asserts no time and no step count: the numbers are a measurement, read by a person. Without
//! `ngspice` on the path only Piperine's line is printed.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::Instant;

use piperine_api::{NodeSource, Overrides, Project, Record, RecordSink, Session};
use piperine_transpile::Settings;

/// The switching period, the time the high side is on and the edge time of the controls, in seconds.
const PERIOD: f64 = 2e-6;
const ON_TIME: f64 = 0.2e-6;
const EDGE: f64 = 10e-9;
/// Periods run, and the stop time they make.
const PERIODS: usize = 50;
const STOP: f64 = 100e-6;

/// A sink that drops every record: the run is timed, not read.
struct Discard;

impl RecordSink for Discard {
    fn record(&mut self, _record: &Record) {}
}

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

/// The accepted steps and the wall time of Piperine's run of the design.
fn piperine_run() -> (u64, f64) {
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
    let (high, low) = controls();
    session.source("ctl", NodeSource::dc(0.0).with_corners(high)).expect("a source on ctl");
    session.source("ctlb", NodeSource::dc(1.0).with_corners(low)).expect("a source on ctlb");
    let started = Instant::now();
    session.run(Rc::new(RefCell::new(Discard))).unwrap_or_else(|f| panic!("{f}"));
    let wall = started.elapsed().as_secs_f64();
    (session.statistics().expect("a transient leaves its statistics").steps().accepted(), wall)
}

/// The data rows (one per accepted timepoint, no interpolation) and the analysis time of ngspice's run
/// of the deck, `None` when ngspice is not installed.
fn ngspice_run() -> Option<(u64, f64)> {
    let deck = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/power_stage/power_stage.cir");
    let output = Command::new("ngspice").arg("-b").arg(&deck).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr);
    let number = |label: &str| -> Option<f64> {
        let line = text.lines().find(|line| line.contains(label))?;
        line.split('=').nth(1).or_else(|| line.split(':').nth(1))?.trim().parse().ok()
    };
    Some((number("No. of Data Rows")? as u64, number("Total analysis time")?))
}

#[test]
fn the_power_stage_time_per_step_against_ngspice_is_printed() {
    let (steps, wall) = piperine_run();
    eprintln!("power stage, piperine: {steps} accepted steps in {wall:.3} s = {:.1} us per step", wall / steps as f64 * 1e6);
    match ngspice_run() {
        Some((rows, seconds)) => {
            eprintln!("power stage, ngspice: {rows} accepted timepoints in {seconds:.3} s = {:.1} us per step", seconds / rows as f64 * 1e6);
            eprintln!("power stage, ratio: {:.2}x the steps, {:.2}x the time per step", steps as f64 / rows as f64, (wall / steps as f64) / (seconds / rows as f64));
        }
        None => eprintln!("power stage, ngspice: not installed, no comparison"),
    }
    assert!(steps > 0, "the run takes steps");
}
