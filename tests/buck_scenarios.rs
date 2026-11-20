//! Scope: the buck converter's SPEC §10 scenarios S2 to S10 under a Rust host (feature
//! `buck-scenarios`, BS-01 to BS-09; MD-73). The scenario package `tests/e2e/buck/` holds a top
//! `Bench` over `examples/buck` and five runs, one per family of scenarios; the host drives the
//! converter's ports and sets its params during a run (`Session::drive`, `Session::set_at`) and
//! judges what the run printed against SPEC's own pass criteria. The measurements and the
//! stimulus are `piperine_parity::scenario`'s. This target holds the package's own tests and one
//! test per criterion of each family; every tolerance a test judges is SPEC's, never read off a
//! run. The five families run once each, at once, behind one `Suite` (BS-08).
#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use piperine::{Analysis, Overrides, ParamValue, Project, Session};
use piperine_parity::scenario::{DiscreteTrace, Ran, ScenarioBatch, ScenarioRun, Stimulus, Trace};
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

// ---------------------------------------------------------------------------------------------
// The vocabulary the families share
// ---------------------------------------------------------------------------------------------

/// The codes of the supervisor's two enums, in the order of SPEC §8 (`Mode`) and §8.5 (`Fault`).
struct Supervisor;

impl Supervisor {
    /// `Mode::Regulate`: the reference follows the target.
    const REGULATE: i128 = 2;
    /// `Fault::Clear`.
    const CLEAR: i128 = 0;
}

/// SPEC's shared numbers: the grid the bench reads, the sample time and the bounds of §10.1.
struct Timing;

impl Timing {
    /// The first stimulus of every 3.3 V family, after the 1.056 ms soft-start and the 128 µs
    /// power-good delay of §8.6 (BS-03 AC 1).
    const FIRST_STIMULUS: f64 = 1.25e-3;
    /// The print grid of every run (BS-01 AC 2).
    const GRID: f64 = 1e-8;
    /// The sense chain: 0.1 V per ampere (§2), so the inductor current is `isense` times ten.
    const AMPERES_PER_VOLT: f64 = 10.0;
    /// The regulation band of every settling criterion: ± 1 % (§2, §10).
    const BAND: f64 = 0.01;
    /// The 3.3 V of the default setpoint (§7.1, code 100).
    const NOMINAL: f64 = 3.3;
}

/// The buck's stimulus vocabulary over a [`Stimulus`]: what each scenario of SPEC §10 does to the
/// converter's five ports and three params, in the shapes of §10.1.
struct BuckHost {
    stimulus: Stimulus,
}

impl BuckHost {
    /// How long `sp_data` is presented before `sp_wr` rises (§10.1).
    const DATA_LEAD: f64 = 1e-6;
    /// How long `sp_wr` stays high after its rise (§10.1).
    const WRITE_PULSE: f64 = 9e-6;
    /// A host with the five ports at rest at time 0: `en`, `sp_wr`, `load_step` and `inject` low,
    /// `sp_data` at the reset code 100 (§7.1).
    fn at_rest() -> BuckHost {
        let stimulus = Stimulus::new()
            .drive("en", ParamValue::Bool(false), 0.0)
            .drive("sp_wr", ParamValue::Bool(false), 0.0)
            .drive("sp_data", ParamValue::Int(100), 0.0)
            .drive("load_step", ParamValue::Bool(false), 0.0)
            .drive("inject", ParamValue::Bool(false), 0.0);
        BuckHost { stimulus }
    }

    /// Applies `change` to the stimulus kept so far.
    fn push(&mut self, change: impl FnOnce(Stimulus) -> Stimulus) -> &mut BuckHost {
        self.stimulus = change(std::mem::take(&mut self.stimulus));
        self
    }

    /// `en` rises at `at`.
    fn enable(&mut self, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.drive("en", ParamValue::Bool(true), at))
    }

    /// Writes the setpoint `code`: `sp_data` is presented 1 µs before `edge`, `sp_wr` rises at
    /// `edge` and falls 9 µs later (§10.1).
    fn write_setpoint(&mut self, code: i64, edge: f64) -> &mut BuckHost {
        self.push(|stimulus| {
            stimulus
                .drive("sp_data", ParamValue::Int(code), edge - BuckHost::DATA_LEAD)
                .drive("sp_wr", ParamValue::Bool(true), edge)
                .drive("sp_wr", ParamValue::Bool(false), edge + BuckHost::WRITE_PULSE)
        })
    }

    /// The schedule built so far.
    fn stimulus(&self) -> Stimulus {
        self.stimulus.clone()
    }
}

/// The buck's columns of one run, as the traces the measurements read.
struct Waveforms<'a> {
    ran: &'a Ran,
}

impl<'a> Waveforms<'a> {
    /// The columns of `ran`.
    fn of(ran: &'a Ran) -> Waveforms<'a> {
        Waveforms { ran }
    }

    /// The output voltage, in volts.
    fn vout(&self) -> Trace {
        Trace::of(&self.ran.table, "buck.vout")
    }

    /// The inductor current, in amperes (the sense output over 0.1 V/A, §2).
    fn inductor_current(&self) -> Trace {
        Trace::of(&self.ran.table, "buck.isense").scaled(Timing::AMPERES_PER_VOLT)
    }

    /// The ramped reference, in ADC codes.
    fn reference(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.ctl.reference")
    }

    /// The supervisor's mode (`Supervisor::OFF` to `Supervisor::HICCUP`).
    fn mode(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "mode")
    }

    /// The supervisor's fault code.
    fn fault(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "fault")
    }

    /// Power-good.
    fn pgood(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "pgood")
    }

    /// The sink-limit comparator (§8.3).
    fn sink_limit(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.sink_oc")
    }

    /// The value of `trace` at the grid row holding `time`.
    fn at(trace: &Trace, time: f64) -> f64 {
        trace.between(time - Timing::GRID / 2.0, time + Timing::GRID / 2.0).mean()
    }

    /// How long after `from` the output settles into `nominal` ± 1 % for good, judged over
    /// `from` to `to` (§10.1's settling time); panics when its last row is outside the band.
    fn settling_after(&self, from: f64, to: f64, nominal: f64) -> f64 {
        let band = (nominal * (1.0 - Timing::BAND), nominal * (1.0 + Timing::BAND));
        let settled = self.vout().between(from, to).settled_into(band.0, band.1);
        settled.map(|time| time - from).unwrap_or_else(|| panic!("buck.vout never settles into {} V to {} V in [{from:e} s, {to:e} s)", band.0, band.1))
    }
}

/// A family of scenarios: one run of the scenario package under the host's stimulus, and the
/// two checks every family owes (BS-03 AC 1, BS-08 AC 4).
trait Family {
    /// The name of the run in `tests/e2e/buck/Piperine.toml`.
    const RUN: &'static str;

    /// What the host does to the converter during the run.
    fn stimulus() -> Stimulus;

    /// What the run left; panics with the run's failure message when it failed (BS-08 AC 2).
    fn ran() -> &'static Ran {
        Suite::get().ran(Self::RUN)
    }

    /// The traces of the run.
    fn waves() -> Waveforms<'static> {
        Waveforms::of(Self::ran())
    }

    /// BS-03 AC 1: at the first stimulus the converter regulates 3.3 V ± 1 % with `pgood` high.
    fn assert_regulating_before_the_first_stimulus() {
        let (waves, at) = (Self::waves(), Timing::FIRST_STIMULUS);
        let (mode, pgood, vout) = (waves.mode().value_at(at), waves.pgood().value_at(at), Waveforms::at(&waves.vout(), at));
        eprintln!("{}: at {at:e} s mode {mode:?} (Regulate = 2), pgood {pgood:?}, vout {vout:.6} V (3.3 V +- 1 %)", Self::RUN);
        assert_eq!(mode, Some(Supervisor::REGULATE), "mode at {at:e} s");
        assert_eq!(pgood, Some(1), "pgood at {at:e} s");
        assert!((vout - Timing::NOMINAL).abs() <= Timing::NOMINAL * Timing::BAND, "vout {vout} V at {at:e} s");
    }

    /// BS-08 AC 4 (S10): the run's `checks` hold records and none failed.
    fn assert_every_check_passed() {
        let checks = Self::ran().checks;
        eprintln!("{}: {} checks, {} failed", Self::RUN, checks.seen, checks.failed);
        assert!(checks.seen > 0, "the `{}` run emitted no `checks` record", Self::RUN);
        assert_eq!(checks.failed, 0, "{} of {} checks failed in the `{}` run", checks.failed, checks.seen, Self::RUN);
    }
}

/// The five results, run once by one [`ScenarioBatch`] at the first access (BS-08).
struct Suite {
    results: Vec<(&'static str, Result<Ran, String>)>,
}

impl Suite {
    /// The suite; the first caller runs every family, the others wait for it.
    fn get() -> &'static Suite {
        static SUITE: OnceLock<Suite> = OnceLock::new();
        SUITE.get_or_init(|| {
            let suite = Suite::run();
            eprintln!("{}", suite.report());
            suite
        })
    }

    /// The runs of the families this target judges, with each one's stimulus.
    fn plan() -> Vec<(&'static str, Stimulus)> {
        vec![
            (SetpointFamily::RUN, SetpointFamily::stimulus()),
        ]
    }

    /// Runs every family at once, each in a build directory of its own.
    fn run() -> Suite {
        let plan = Suite::plan();
        let runs = plan.iter().map(|(run, stimulus)| ScenarioRun::new(&Package::dir(), run, stimulus.clone())).collect();
        let names: Vec<&'static str> = plan.iter().map(|(run, _)| *run).collect();
        Suite { results: names.into_iter().zip(ScenarioBatch::new(runs).execute()).collect() }
    }

    /// The run `run`; panics with the stored message when it failed, so every test of the family
    /// reports the failure once and none runs it again (BS-08 AC 2).
    fn ran(&self, run: &str) -> &Ran {
        match self.results.iter().find(|(name, _)| *name == run) {
            Some((_, Ok(ran))) => ran,
            Some((_, Err(message))) => panic!("{message}"),
            None => panic!("no family runs `{run}`"),
        }
    }

    /// Every run that completed.
    fn completed(&self) -> Vec<&Ran> {
        self.results.iter().filter_map(|(_, result)| result.as_ref().ok()).collect()
    }

    /// One line per family: its wall time, its rows and its interval, in seconds from the first
    /// start (BS-08 AC 3), and one line for a family that failed.
    fn report(&self) -> String {
        let origin = self.completed().iter().map(|ran| ran.started).min();
        let lines: Vec<String> = self
            .results
            .iter()
            .map(|(name, result)| match (result, origin) {
                (Ok(ran), Some(origin)) => format!("scenario `{name}`: wall time {:.1} s, {} rows, ran {:.1} s to {:.1} s", ran.wall().as_secs_f64(), ran.table.rows.len(), (ran.started - origin).as_secs_f64(), (ran.ended - origin).as_secs_f64()),
                (Ok(_), None) => unreachable!("a completed run gives an origin"),
                (Err(message), _) => format!("scenario `{name}`: failed: {message}"),
            })
            .collect();
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------------------------
// The setpoint family: S2 and S2b (BS-03)
// ---------------------------------------------------------------------------------------------

/// S2 and S2b on one run: 3.3 V to 5.0 V at 1.25 ms, then 5.0 V to 1.0 V at 2.00 ms.
struct SetpointFamily;

impl SetpointFamily {
    /// The edge of the write of 168 (5.0 V).
    const WRITE_UP: f64 = 1.25e-3;
    /// The edge of the write of 8 (1.0 V).
    const WRITE_DOWN: f64 = 2.00e-3;
    /// The end of the run.
    const END: f64 = 3.40e-3;
}

impl Family for SetpointFamily {
    const RUN: &'static str = "setpoint";

    fn stimulus() -> Stimulus {
        let mut host = BuckHost::at_rest();
        host.enable(1e-6).write_setpoint(168, SetpointFamily::WRITE_UP).write_setpoint(8, SetpointFamily::WRITE_DOWN);
        host.stimulus()
    }
}

#[test]
fn setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus() {
    SetpointFamily::assert_regulating_before_the_first_stimulus();
}

#[test]
fn s2_reference_reaches_800_codes_0_544_ms_after_the_write() {
    // BS-03 AC 2, SPEC §7.2: 528 to 800 codes at one code per sample is 0.544 ms
    let reached = SetpointFamily::waves().reference().first_time(800, SetpointFamily::WRITE_UP).expect("the reference never reads 800 codes");
    let delay = reached - SetpointFamily::WRITE_UP;
    eprintln!("S2: reference reads 800 codes {:.3} us after the write (0.544 ms +- 2 us)", delay * 1e6);
    assert!((delay - 0.544e-3).abs() <= 2e-6, "the reference reads 800 codes {delay:e} s after the write");
}

#[test]
fn s2_vout_settles_into_5_v_within_1_percent_at_0_54_ms() {
    // BS-03 AC 3, SPEC §10 S2: within 1 % of 5.0 V at 0.54 ms +- 0.05 ms
    let settling = SetpointFamily::waves().settling_after(SetpointFamily::WRITE_UP, SetpointFamily::WRITE_DOWN, 5.0);
    eprintln!("S2: vout settles into 5.0 V +- 1 % {:.3} ms after the write (0.54 ms +- 0.05 ms)", settling * 1e3);
    assert!((settling - 0.54e-3).abs() <= 0.05e-3, "vout settles {settling:e} s after the write");
}

#[test]
fn s2_overshoot_is_at_most_15_mv() {
    // BS-03 AC 4, SPEC §10 S2: from 1.25 ms to 2.00 ms vout is at most 15 mV above 5.0 V
    let peak = SetpointFamily::waves().vout().between(SetpointFamily::WRITE_UP, SetpointFamily::WRITE_DOWN).maximum();
    eprintln!("S2: the largest vout is {:.6} V at {:e} s (limit 5.015 V)", peak.value, peak.time);
    assert!(peak.value <= 5.015, "vout reaches {} V at {:e} s", peak.value, peak.time);
}

#[test]
fn s2_pgood_stays_high() {
    // BS-03 AC 5, SPEC §10 S2: `pgood` stays high through the ramp
    let held = SetpointFamily::waves().pgood().between(SetpointFamily::WRITE_UP, SetpointFamily::WRITE_DOWN).holds(1);
    eprintln!("S2: pgood over 1.25 ms to 2.00 ms: {held:?}");
    assert!(held.is_ok(), "pgood departs: {held:?}");
}

#[test]
fn s2b_reference_reaches_160_codes_1_28_ms_after_the_write() {
    // BS-03 AC 6, SPEC §7.2: 800 to 160 codes at one code per sample is 1.28 ms
    let reached = SetpointFamily::waves().reference().first_time(160, SetpointFamily::WRITE_DOWN).expect("the reference never reads 160 codes");
    let delay = reached - SetpointFamily::WRITE_DOWN;
    eprintln!("S2b: reference reads 160 codes {:.3} us after the write (1.28 ms +- 2 us)", delay * 1e6);
    assert!((delay - 1.28e-3).abs() <= 2e-6, "the reference reads 160 codes {delay:e} s after the write");
}

#[test]
fn s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms() {
    // BS-03 AC 7, SPEC §10 S2b: within 1 % of 1.0 V at 1.29 ms +- 0.05 ms
    let settling = SetpointFamily::waves().settling_after(SetpointFamily::WRITE_DOWN, SetpointFamily::END, 1.0);
    eprintln!("S2b: vout settles into 1.0 V +- 1 % {:.3} ms after the write (1.29 ms +- 0.05 ms)", settling * 1e3);
    assert!((settling - 1.29e-3).abs() <= 0.05e-3, "vout settles {settling:e} s after the write");
}

#[test]
fn s2b_inductor_current_goes_negative() {
    // BS-03 AC 8, SPEC §10 S2b: FCCM sinks, so the inductor current goes below 0 A (about -0.6 A,
    // an estimate that is reported, not judged: §10.1)
    let lowest = SetpointFamily::waves().inductor_current().between(SetpointFamily::WRITE_DOWN, SetpointFamily::END).minimum();
    eprintln!("S2b: the inductor current reaches {:.4} A at {:e} s (below 0 A; SPEC estimates -0.6 A)", lowest.value, lowest.time);
    assert!(lowest.value < 0.0, "the inductor current stays at or above 0 A: {} A", lowest.value);
}

#[test]
fn s2b_sink_limit_never_trips() {
    // BS-03 AC 9, SPEC §10 S2b: the -3 A limit is never hit
    let held = SetpointFamily::waves().sink_limit().between(SetpointFamily::WRITE_DOWN, SetpointFamily::END).holds(0);
    eprintln!("S2b: buck.sink_oc over 2.00 ms to 3.40 ms: {held:?}");
    assert!(held.is_ok(), "the sink limit trips: {held:?}");
}

#[test]
fn s2b_no_over_voltage_fault() {
    // BS-03 AC 10, SPEC §10 S2b: no OVP, so `fault` stays `Clear` and `mode` stays `Regulate`
    let waves = SetpointFamily::waves();
    let (fault, mode) = (waves.fault().between(SetpointFamily::WRITE_DOWN, SetpointFamily::END).holds(Supervisor::CLEAR), waves.mode().between(SetpointFamily::WRITE_DOWN, SetpointFamily::END).holds(Supervisor::REGULATE));
    eprintln!("S2b: fault {fault:?}, mode {mode:?} over 2.00 ms to 3.40 ms");
    assert!(fault.is_ok(), "fault departs from Clear: {fault:?}");
    assert!(mode.is_ok(), "mode departs from Regulate: {mode:?}");
}

#[test]
fn setpoint_passes_every_check() {
    // BS-08 AC 4, SPEC §10 S10
    SetpointFamily::assert_every_check_passed();
}

#[test]
fn the_families_run_at_once_in_their_own_build_directories() {
    // BS-08 AC 1: every run's interval overlaps every other's, and no two share a build directory
    let ran = Suite::get().completed();
    let (latest_start, earliest_end) = (ran.iter().map(|ran| ran.started).max().expect("a run"), ran.iter().map(|ran| ran.ended).min().expect("a run"));
    assert!(latest_start < earliest_end, "the runs do not all overlap: the last starts before the first ends");
    let directories: BTreeSet<&PathBuf> = ran.iter().map(|ran| &ran.build).collect();
    assert_eq!(directories.len(), ran.len(), "two runs share a build directory");
}

#[test]
fn every_family_reports_its_wall_time_and_rows() {
    // BS-08 AC 3: one line per family with its wall time and its rows
    let suite = Suite::get();
    let report = suite.report();
    eprintln!("{report}");
    for ran in suite.completed() {
        let line = report.lines().find(|line| line.starts_with(&format!("scenario `{}`", ran.run))).unwrap_or_else(|| panic!("no line for `{}` in {report}", ran.run));
        assert!(line.contains(&format!("wall time {:.1} s", ran.wall().as_secs_f64())), "{line}");
        assert!(line.contains(&format!("{} rows", ran.table.rows.len())), "{line}");
    }
}
