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

use piperine::{Analysis, Cell, Overrides, ParamValue, Project, Session};
use piperine_parity::scenario::{CompletedRun, Crossing, DiscreteTrace, NamedColumn, Ramp, ScenarioBatch, ScenarioRun, Stimulus, Trace};
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
    /// `Mode::Off`: both switches off.
    const OFF: i128 = 0;
    /// `Mode::SoftStart`: the reference ramps up from zero.
    const SOFT_START: i128 = 1;
    /// `Mode::Regulate`: the reference follows the target.
    const REGULATE: i128 = 2;
    /// `Mode::Hiccup`: both switches off, counting down to a retry.
    const HICCUP: i128 = 3;
    /// `Fault::Clear`.
    const CLEAR: i128 = 0;
    /// `Fault::OverCurrent`: eight consecutive cut periods (§8.2).
    const OVER_CURRENT: i128 = 1;
    /// `Fault::OverVoltage`: two consecutive samples over the threshold (§8.4).
    const OVER_VOLTAGE: i128 = 2;
}

/// SPEC's shared numbers: the grid the bench reads, the sample time and the bounds of §10.1.
struct Timing;

impl Timing {
    /// The first stimulus of every 3.3 V family, after the 1.056 ms soft-start and the 128 µs
    /// power-good delay of §8.6 (BS-03 AC 1).
    const FIRST_STIMULUS: f64 = 1.25e-3;
    /// The print grid of every run (BS-01 AC 2).
    const GRID: f64 = 1e-8;
    /// One sample, 2 µs (§4.2).
    const SAMPLE: f64 = 2e-6;
    /// A mode that changes at the next sample shows within this long of its cause (§10.1).
    const NEXT_SAMPLE: f64 = 2.02e-6;
    /// A fault that needs two consecutive samples shows within this long of its cause (§10.1).
    const TWO_SAMPLES: f64 = 4.52e-6;
    /// The sense chain: 0.1 V per ampere (§2), so the inductor current is `isense` times ten.
    const AMPERES_PER_VOLT: f64 = 10.0;
    /// The regulation band of every settling criterion: ± 1 % (§2, §10).
    const BAND: f64 = 0.01;
    /// The solver's voltage tolerance `vntol`, 1 µV: a difference of two rows below it is noise of
    /// the solution, so a fall is judged to it.
    const VOLTAGE_RESOLUTION: f64 = 1e-6;
    /// The 3.3 V of the default setpoint (§7.1, code 100).
    const NOMINAL: f64 = 3.3;
}

/// The buck's stimulus vocabulary over a [`Stimulus`]: what each scenario of SPEC §10 does to the
/// converter's five ports and three params, in the shapes of §10.1.
struct BuckHost {
    /// The schedule built so far.
    stimulus: Stimulus,
}

impl BuckHost {
    /// How long `sp_data` is presented before `sp_wr` rises (§10.1).
    const DATA_LEAD: f64 = 1e-6;
    /// How long `sp_wr` stays high after its rise (§10.1).
    const WRITE_PULSE: f64 = 9e-6;
    /// The size of one step of a supply ramp (§10.1).
    const SUPPLY_STEP: f64 = 0.1;

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

    /// `en` falls at `at`.
    fn disable(&mut self, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.drive("en", ParamValue::Bool(false), at))
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

    /// `load_step` takes `high` at `at`: the load step of S3, the overload of S5.
    fn load_step(&mut self, high: bool, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.drive("load_step", ParamValue::Bool(high), at))
    }

    /// `inject` rises at `at`: the back-drive of S6.
    fn inject(&mut self, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.drive("inject", ParamValue::Bool(true), at))
    }

    /// The base load becomes `ohms` at `at`.
    fn load(&mut self, ohms: f64, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.set("buck.r_load", ohms, at))
    }

    /// The resistor `load_step` switches in becomes `ohms` at `at`.
    fn step_resistance(&mut self, ohms: f64, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.set("buck.r_step", ohms, at))
    }

    /// The supply is `volts` at `at` (time 0 sets its initial value).
    fn supply(&mut self, volts: f64, at: f64) -> &mut BuckHost {
        self.push(|stimulus| stimulus.set("buck.vin_nom", volts, at))
    }

    /// The supply moves from `from` to `to` as a staircase of 0.1 V steps over `duration` from
    /// `start` (§10.1: the line step, the brown-out).
    fn line(&mut self, from: f64, to: f64, start: f64, duration: f64) -> &mut BuckHost {
        let ramp = Ramp { from, to, start, duration, step: BuckHost::SUPPLY_STEP };
        self.push(|stimulus| stimulus.ramp("buck.vin_nom", ramp))
    }

    /// The schedule built so far.
    fn stimulus(&self) -> Stimulus {
        self.stimulus.clone()
    }
}

/// The buck's columns of one run, as the traces the measurements read.
struct Waveforms<'a> {
    /// The completed run whose table the traces read.
    ran: &'a CompletedRun,
}

impl<'a> Waveforms<'a> {
    /// The columns of `ran`.
    fn of(ran: &'a CompletedRun) -> Waveforms<'a> {
        Waveforms { ran }
    }

    /// The output voltage, in volts.
    fn vout(&self) -> Trace {
        Trace::of(&self.ran.table, "buck.vout")
    }

    /// The input node, in volts.
    fn vin(&self) -> Trace {
        Trace::of(&self.ran.table, "buck.vin")
    }

    /// The inductor current, in amperes (the sense output over 0.1 V/A, §2).
    fn inductor_current(&self) -> Trace {
        Trace::of(&self.ran.table, "buck.isense").scaled(Timing::AMPERES_PER_VOLT)
    }

    /// The ramped reference, in ADC codes.
    fn reference(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.ctl.reference")
    }

    /// The supervisor's mode (`Supervisor::OFF` to `Supervisor::HICCUP`): the table prints the variant's
    /// name, read here as its place in the declaration.
    fn mode(&self) -> DiscreteTrace {
        DiscreteTrace::of_enum(&self.ran.table, "mode", &["Off", "SoftStart", "Regulate", "Hiccup"])
    }

    /// The supervisor's fault code, read as the declaration index of the variant the table prints.
    fn fault(&self) -> DiscreteTrace {
        DiscreteTrace::of_enum(&self.ran.table, "fault", &["Clear", "OverCurrent", "OverVoltage"])
    }

    /// Power-good.
    fn pgood(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "pgood")
    }

    /// The high-side gate command.
    fn high_side(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.hs_cmd")
    }

    /// The low-side gate command.
    fn low_side(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.ls_cmd")
    }

    /// The cycle-by-cycle overcurrent comparator (§8.2).
    fn over_current(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.oc")
    }

    /// The sink-limit comparator (§8.3).
    fn sink_limit(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.sink_oc")
    }

    /// The flag of a cut period (§8.2).
    fn cut(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.ctl.cut")
    }

    /// The ADC code of every sample.
    fn code(&self) -> DiscreteTrace {
        DiscreteTrace::of(&self.ran.table, "buck.code")
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

    /// The first time at or after `after` that the ADC code is outside the power-good window
    /// `reference ± reference / 16` (§8.6, in the integer arithmetic of the supervisor).
    fn first_code_outside_window(&self, after: f64) -> Option<f64> {
        let (code, reference) = (NamedColumn::find(&self.ran.table, "buck.code"), NamedColumn::find(&self.ran.table, "buck.ctl.reference"));
        self.ran.table.rows.iter().find_map(|row| match (&row[0], &row[code.index()], &row[reference.index()]) {
            (Cell::Real(time), Cell::Int(code), Cell::Int(reference)) if *time >= after && (code + reference / 16 < *reference || *code > reference + reference / 16) => Some(*time),
            _ => None,
        })
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
    fn ran() -> &'static CompletedRun {
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
    /// Each run's name with its completed run or the message of its failure.
    results: Vec<(&'static str, Result<CompletedRun, String>)>,
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
            (BrownoutFamily::RUN, BrownoutFamily::stimulus()),
            (FaultsFamily::RUN, FaultsFamily::stimulus()),
            (LoadLineFamily::RUN, LoadLineFamily::stimulus()),
            (SetpointFamily::RUN, SetpointFamily::stimulus()),
            (SteadyStateFamily::RUN, SteadyStateFamily::stimulus()),
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
    fn ran(&self, run: &str) -> &CompletedRun {
        match self.results.iter().find(|(name, _)| *name == run) {
            Some((_, Ok(ran))) => ran,
            Some((_, Err(message))) => panic!("{message}"),
            None => panic!("no family runs `{run}`"),
        }
    }

    /// Every run that completed.
    fn completed(&self) -> Vec<&CompletedRun> {
        self.results.iter().filter_map(|(_, result)| result.as_ref().ok()).collect()
    }

    /// One line per family: its wall time, its accepted steps, its rows and its interval, in
    /// seconds from the first start (BS-08 AC 3), and one line for a family that failed.
    fn report(&self) -> String {
        let origin = self.completed().iter().map(|ran| ran.started).min();
        let lines: Vec<String> = self
            .results
            .iter()
            .map(|(name, result)| match (result, origin) {
                (Ok(ran), Some(origin)) => format!("scenario `{name}`: wall time {:.1} s, {} accepted steps, {} rows, ran {:.1} s to {:.1} s", ran.wall().as_secs_f64(), ran.accepted_steps, ran.table.rows.len(), (ran.started - origin).as_secs_f64(), (ran.ended - origin).as_secs_f64()),
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
fn s2b_vout_settles_into_1_v_within_1_percent_at_1_38_ms() {
    // BS-03 AC 7, SPEC §10 S2b: within 1 % of 1.0 V at 1.38 ms +- 0.05 ms
    let settling = SetpointFamily::waves().settling_after(SetpointFamily::WRITE_DOWN, SetpointFamily::END, 1.0);
    eprintln!("S2b: vout settles into 1.0 V +- 1 % {:.3} ms after the write (1.38 ms +- 0.05 ms)", settling * 1e3);
    assert!((settling - 1.38e-3).abs() <= 0.05e-3, "vout settles {settling:e} s after the write");
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

// ---------------------------------------------------------------------------------------------
// The load and line family: S3, S4 and S8 at 3.3 V (BS-04, BS-06 AC 6 to AC 9)
// ---------------------------------------------------------------------------------------------

/// S3, S4 and S8 on one run: the load steps, the line steps, then `en` falls.
struct LoadLineFamily;

impl LoadLineFamily {
    /// The load step rises (S3 up).
    const LOAD_UP: f64 = 1.25e-3;
    /// The load step falls (S3 down).
    const LOAD_DOWN: f64 = 1.55e-3;
    /// The end of S3's windows.
    const LOAD_END: f64 = 1.85e-3;
    /// The load becomes 1.65 Ω, the 2 A of S4, 200 µs before the line step (§10.1).
    const LOAD_2_AMPERES: f64 = 1.85e-3;
    /// The line step up starts (S4 up).
    const LINE_UP: f64 = 2.05e-3;
    /// The line step down starts (S4 down).
    const LINE_DOWN: f64 = 2.35e-3;
    /// The end of S4's windows.
    const LINE_END: f64 = 2.65e-3;
    /// The 10 µs a line step takes (§10.1).
    const LINE_STEP: f64 = 10e-6;
    /// `en` falls (S8).
    const DISABLE: f64 = 2.80e-3;
    /// The end of the run.
    const END: f64 = 2.92e-3;
    /// The base load again, 0.5 A, once S4 is judged.
    const LOAD_HALF_AMPERE: f64 = 2.65e-3;
    /// The deviation bound of S3 and S4: 5 % of 3.3 V (§10).
    const DEVIATION: f64 = 0.165;
    /// The time to be back within the band: 200 µs (§10).
    const RECOVERY: f64 = 200e-6;

    /// The time `mode` first reads `Off` after `en` falls.
    fn off_entry() -> f64 {
        Self::waves().mode().first_time(Supervisor::OFF, Self::DISABLE).expect("mode never reads Off after en falls")
    }
}

impl Family for LoadLineFamily {
    const RUN: &'static str = "load_line";

    fn stimulus() -> Stimulus {
        let mut host = BuckHost::at_rest();
        host.enable(1e-6).load_step(true, Self::LOAD_UP).load_step(false, Self::LOAD_DOWN).load(1.65, Self::LOAD_2_AMPERES);
        host.line(12.0, 16.0, Self::LINE_UP, Self::LINE_STEP).line(16.0, 12.0, Self::LINE_DOWN, Self::LINE_STEP);
        host.load(6.6, Self::LOAD_HALF_AMPERE).disable(Self::DISABLE);
        host.stimulus()
    }
}

#[test]
fn load_line_regulates_at_3_3_v_with_pgood_before_its_first_stimulus() {
    LoadLineFamily::assert_regulating_before_the_first_stimulus();
}

#[test]
fn s3_load_step_up_stays_within_165_mv() {
    // BS-04 AC 1, SPEC §10 S3: deviation at most 165 mV (5 %) after 0.5 A to 4.5 A
    let deviation = LoadLineFamily::waves().vout().between(LoadLineFamily::LOAD_UP, LoadLineFamily::LOAD_DOWN).deviation_from(Timing::NOMINAL);
    eprintln!("S3 up: deviation {:.2} mV at {:e} s (limit 165 mV)", deviation.value * 1e3, deviation.time);
    assert!(deviation.value <= LoadLineFamily::DEVIATION, "vout is {} V from 3.3 V at {:e} s", deviation.value, deviation.time);
}

#[test]
fn s3_load_step_down_stays_within_165_mv() {
    // BS-04 AC 2, SPEC §10 S3: deviation at most 165 mV after 4.5 A to 0.5 A
    let deviation = LoadLineFamily::waves().vout().between(LoadLineFamily::LOAD_DOWN, LoadLineFamily::LOAD_END).deviation_from(Timing::NOMINAL);
    eprintln!("S3 down: deviation {:.2} mV at {:e} s (limit 165 mV)", deviation.value * 1e3, deviation.time);
    assert!(deviation.value <= LoadLineFamily::DEVIATION, "vout is {} V from 3.3 V at {:e} s", deviation.value, deviation.time);
}

#[test]
fn s3_load_step_up_recovers_within_200_us() {
    // BS-04 AC 3, SPEC §10 S3: back within 1 % no later than 200 us after the edge
    let settling = LoadLineFamily::waves().settling_after(LoadLineFamily::LOAD_UP, LoadLineFamily::LOAD_DOWN, Timing::NOMINAL);
    eprintln!("S3 up: vout settles into 3.3 V +- 1 % {:.1} us after the edge (limit 200 us)", settling * 1e6);
    assert!(settling <= LoadLineFamily::RECOVERY, "vout settles {settling:e} s after the edge");
}

#[test]
fn s3_load_step_down_recovers_within_200_us() {
    // BS-04 AC 4, SPEC §10 S3: back within 1 % no later than 200 us after the edge
    let settling = LoadLineFamily::waves().settling_after(LoadLineFamily::LOAD_DOWN, LoadLineFamily::LOAD_END, Timing::NOMINAL);
    eprintln!("S3 down: vout settles into 3.3 V +- 1 % {:.1} us after the edge (limit 200 us)", settling * 1e6);
    assert!(settling <= LoadLineFamily::RECOVERY, "vout settles {settling:e} s after the edge");
}

#[test]
fn s3_pgood_stays_high() {
    // BS-04 AC 5, SPEC §10 S3: `pgood` stays high from 1.25 ms to 1.85 ms
    let held = LoadLineFamily::waves().pgood().between(LoadLineFamily::LOAD_UP, LoadLineFamily::LOAD_END).holds(1);
    eprintln!("S3: pgood over 1.25 ms to 1.85 ms: {held:?}");
    assert!(held.is_ok(), "pgood departs: {held:?}");
}

#[test]
fn s3_no_current_limit() {
    // BS-04 AC 6, SPEC §10 S3: `buck.oc` stays low from 1.25 ms to 1.85 ms
    let held = LoadLineFamily::waves().over_current().between(LoadLineFamily::LOAD_UP, LoadLineFamily::LOAD_END).holds(0);
    eprintln!("S3: buck.oc over 1.25 ms to 1.85 ms: {held:?}");
    assert!(held.is_ok(), "the current limit trips: {held:?}");
}

#[test]
fn s4_line_step_up_stays_within_165_mv() {
    // BS-04 AC 7, SPEC §10 S4: deviation at most 165 mV after 12 V to 16 V
    let deviation = LoadLineFamily::waves().vout().between(LoadLineFamily::LINE_UP, LoadLineFamily::LINE_DOWN).deviation_from(Timing::NOMINAL);
    eprintln!("S4 up: deviation {:.2} mV at {:e} s (limit 165 mV)", deviation.value * 1e3, deviation.time);
    assert!(deviation.value <= LoadLineFamily::DEVIATION, "vout is {} V from 3.3 V at {:e} s", deviation.value, deviation.time);
}

#[test]
fn s4_line_step_down_stays_within_165_mv() {
    // BS-04 AC 8, SPEC §10 S4: deviation at most 165 mV after 16 V to 12 V
    let deviation = LoadLineFamily::waves().vout().between(LoadLineFamily::LINE_DOWN, LoadLineFamily::LINE_END).deviation_from(Timing::NOMINAL);
    eprintln!("S4 down: deviation {:.2} mV at {:e} s (limit 165 mV)", deviation.value * 1e3, deviation.time);
    assert!(deviation.value <= LoadLineFamily::DEVIATION, "vout is {} V from 3.3 V at {:e} s", deviation.value, deviation.time);
}

#[test]
fn s4_line_step_up_recovers_within_200_us() {
    // BS-04 AC 9, SPEC §10 S4: back within 1 % no later than 200 us after the step's start
    let settling = LoadLineFamily::waves().settling_after(LoadLineFamily::LINE_UP, LoadLineFamily::LINE_DOWN, Timing::NOMINAL);
    eprintln!("S4 up: vout settles into 3.3 V +- 1 % {:.1} us after the step starts (limit 200 us)", settling * 1e6);
    assert!(settling <= LoadLineFamily::RECOVERY, "vout settles {settling:e} s after the step starts");
}

#[test]
fn s4_line_step_down_recovers_within_200_us() {
    // BS-04 AC 10, SPEC §10 S4: back within 1 % no later than 200 us after the step's start
    let settling = LoadLineFamily::waves().settling_after(LoadLineFamily::LINE_DOWN, LoadLineFamily::LINE_END, Timing::NOMINAL);
    eprintln!("S4 down: vout settles into 3.3 V +- 1 % {:.1} us after the step starts (limit 200 us)", settling * 1e6);
    assert!(settling <= LoadLineFamily::RECOVERY, "vout settles {settling:e} s after the step starts");
}

#[test]
fn s4_no_fault() {
    // BS-04 AC 11, SPEC §10 S4: no fault, so `fault` stays `Clear` and `mode` stays `Regulate`
    let waves = LoadLineFamily::waves();
    let fault = waves.fault().between(LoadLineFamily::LINE_UP, LoadLineFamily::LINE_END).holds(Supervisor::CLEAR);
    let mode = waves.mode().between(LoadLineFamily::LINE_UP, LoadLineFamily::LINE_END).holds(Supervisor::REGULATE);
    eprintln!("S4: fault {fault:?}, mode {mode:?} over 2.05 ms to 2.65 ms");
    assert!(fault.is_ok(), "fault departs from Clear: {fault:?}");
    assert!(mode.is_ok(), "mode departs from Regulate: {mode:?}");
}

#[test]
fn s8_off_at_the_sample_after_en_falls() {
    // BS-06 AC 6, SPEC §10 S8, §10.1: `Off` within 2.02 us of the edge
    let delay = LoadLineFamily::off_entry() - LoadLineFamily::DISABLE;
    eprintln!("S8: mode reads Off {:.3} us after en falls (limit 2.02 us)", delay * 1e6);
    assert!((0.0..=Timing::NEXT_SAMPLE).contains(&delay), "mode reads Off {delay:e} s after en falls");
}

#[test]
fn s8_both_switches_stay_off() {
    // BS-06 AC 7, SPEC §10 S8: both gate commands low from one grid row after the entry on
    let (waves, from) = (LoadLineFamily::waves(), LoadLineFamily::off_entry() + Timing::GRID);
    let (high, low) = (waves.high_side().between(from, LoadLineFamily::END).holds(0), waves.low_side().between(from, LoadLineFamily::END).holds(0));
    eprintln!("S8: hs_cmd {high:?}, ls_cmd {low:?} from {from:e} s to the end");
    assert!(high.is_ok(), "the high side departs: {high:?}");
    assert!(low.is_ok(), "the low side departs: {low:?}");
}

#[test]
fn s8_pgood_low_from_off() {
    // BS-06 AC 8, SPEC §8.6: `pgood` falls at once on leaving `Regulate`
    let (waves, from) = (LoadLineFamily::waves(), LoadLineFamily::off_entry());
    let held = waves.pgood().between(from, LoadLineFamily::END).holds(0);
    eprintln!("S8: pgood from {from:e} s to the end: {held:?}");
    assert!(held.is_ok(), "pgood departs from low: {held:?}");
}

#[test]
fn s8_vout_decays_with_the_load_time_constant() {
    // BS-06 AC 9, SPEC §10.1: from 10 us after the entry vout falls in every row with the time
    // constant (6.6 ohm parallel 40 kohm) * 220 uF = 1.452 ms, within 2 %. A row "falls" to the
    // solver's own voltage resolution: the rows are interpolated between accepted steps, and two
    // of them differ by less than `vntol` (1 uV) in either direction
    let decay = LoadLineFamily::waves().vout().between(LoadLineFamily::off_entry() + 10e-6, LoadLineFamily::END + Timing::GRID);
    let (rise, constant) = (decay.largest_rise(), decay.decay_time_constant());
    eprintln!("S8: the largest rise between two rows is {:.3e} V at {:e} s (solver resolution {:e} V); time constant {:.4} ms (1.452 ms +- 2 %)", rise.value, rise.time, Timing::VOLTAGE_RESOLUTION, constant * 1e3);
    assert!(rise.value <= Timing::VOLTAGE_RESOLUTION, "vout rises by {} V at {:e} s", rise.value, rise.time);
    assert!((constant - 1.452e-3).abs() <= 1.452e-3 * 0.02, "time constant {constant:e} s");
}

#[test]
fn load_line_passes_every_check() {
    // BS-08 AC 4, SPEC §10 S10
    LoadLineFamily::assert_every_check_passed();
}

// ---------------------------------------------------------------------------------------------
// The faults family: S5 then S6 (BS-05)
// ---------------------------------------------------------------------------------------------

/// S5 and S6 on one run: an overload at 1.25 ms that is removed during the retry, then a
/// back-drive at 4.50 ms.
struct FaultsFamily;

impl FaultsFamily {
    /// The load step to 0.3 Ω rises (S5's overload).
    const OVERLOAD: f64 = 1.25e-3;
    /// The overload is removed, during the retry's soft-start.
    const OVERLOAD_REMOVED: f64 = 3.32e-3;
    /// `inject` rises (S6).
    const INJECT: f64 = 4.50e-3;
    /// The overload resistance: 11 A of demand at 3.3 V (§10 S5).
    const OVERLOAD_OHMS: f64 = 0.3;
    /// The inductor current stays at or below 7.5 A plus the reaction bound (§10.1).
    const CURRENT_CEILING: f64 = 7.5 + 0.1;
    /// The over-voltage threshold at 3.3 V, 528 + 66 + 16 codes (§8.4), in volts.
    const OVER_VOLTAGE_LEVEL: f64 = 3.81;

    /// The time `mode` first reads `Hiccup` after the overload, with `fault` `OverCurrent`.
    fn over_current_entry() -> f64 {
        let waves = Self::waves();
        let entry = waves.mode().first_time(Supervisor::HICCUP, Self::OVERLOAD).expect("mode never reads Hiccup after the overload");
        assert_eq!(waves.fault().value_at(entry), Some(Supervisor::OVER_CURRENT), "fault at the entry into Hiccup, {entry:e} s");
        entry
    }

    /// The time `mode` reads `SoftStart` after the overcurrent `Hiccup`: the first retry.
    fn retry() -> f64 {
        Self::waves().mode().first_time(Supervisor::SOFT_START, Self::over_current_entry()).expect("mode never retries after the overcurrent Hiccup")
    }

    /// The time `mode` first reads `Hiccup` after `inject` rises, with `fault` `OverVoltage`.
    fn over_voltage_entry() -> f64 {
        let waves = Self::waves();
        let entry = waves.mode().first_time(Supervisor::HICCUP, Self::INJECT).expect("mode never reads Hiccup after inject");
        assert_eq!(waves.fault().value_at(entry), Some(Supervisor::OVER_VOLTAGE), "fault at the entry into Hiccup, {entry:e} s");
        entry
    }

    /// The first time `vout` is at or above 3.81 V after `inject` rises.
    fn over_voltage_level_reached() -> f64 {
        Self::waves().vout().first_crossing(Self::OVER_VOLTAGE_LEVEL, Crossing::Rising, Self::INJECT).expect("vout never reaches 3.81 V after inject")
    }
}

impl Family for FaultsFamily {
    const RUN: &'static str = "faults";

    fn stimulus() -> Stimulus {
        let mut host = BuckHost::at_rest();
        host.step_resistance(Self::OVERLOAD_OHMS, 0.0).enable(1e-6).load_step(true, Self::OVERLOAD);
        host.load_step(false, Self::OVERLOAD_REMOVED).inject(Self::INJECT);
        host.stimulus()
    }
}

#[test]
fn faults_regulates_at_3_3_v_with_pgood_before_its_first_stimulus() {
    FaultsFamily::assert_regulating_before_the_first_stimulus();
}

#[test]
fn s5_every_period_is_cut_until_hiccup() {
    // BS-05 AC 1, SPEC §8.2: from the first rise of `cut` to the entry into Hiccup it stays high
    let cut = FaultsFamily::waves().cut();
    let (first, entry) = (cut.first_time(1, FaultsFamily::OVERLOAD).expect("cut never rises after the overload"), FaultsFamily::over_current_entry());
    let held = cut.between(first, entry).holds(1);
    eprintln!("S5: cut first rises at {first:e} s, Hiccup at {entry:e} s: {held:?}");
    assert!(held.is_ok(), "a period is not cut: {held:?}");
}

#[test]
fn s5_inductor_current_stays_at_or_below_7_6_a() {
    // BS-05 AC 2, SPEC §8.2 and §10.1: 7.5 A plus the reaction bound of 0.1 A
    let (waves, entry) = (FaultsFamily::waves(), FaultsFamily::over_current_entry());
    let peak = waves.inductor_current().between(FaultsFamily::OVERLOAD, entry).maximum();
    eprintln!("S5: the inductor current peaks at {:.4} A at {:e} s (limit 7.6 A)", peak.value, peak.time);
    assert!(peak.value <= FaultsFamily::CURRENT_CEILING, "the inductor current is {} A at {:e} s", peak.value, peak.time);
}

#[test]
fn s5_hiccup_with_over_current_26_5_to_30_5_us_after_the_step() {
    // BS-05 AC 3, SPEC §10 S5 and §10.1: Hiccup with OverCurrent 26.5 to 30.5 us after the edge
    let delay = FaultsFamily::over_current_entry() - FaultsFamily::OVERLOAD;
    eprintln!("S5: Hiccup with OverCurrent {:.3} us after the step (26.5 us to 30.5 us)", delay * 1e6);
    assert!((26.5e-6..=30.5e-6).contains(&delay), "Hiccup {delay:e} s after the step");
    // the mechanism, SPEC §10.1: `cut` rises at the wrap that ends the first cut period, 2 us after
    // its start, and Hiccup is 8 periods and the 500 ns conversion after that start: 16.5 us
    let first_cut_start = FaultsFamily::waves().cut().first_time(1, FaultsFamily::OVERLOAD).expect("cut never rises after the overload") - 2e-6;
    let after_start = FaultsFamily::over_current_entry() - first_cut_start;
    eprintln!("S5: Hiccup {:.4} us after the start of the first cut period (16.5 us +- 20 ns)", after_start * 1e6);
    assert!((after_start - 16.5e-6).abs() <= 20e-9, "Hiccup {after_start:e} s after the first cut period starts");
}

#[test]
fn s5_pgood_falls_within_8_us_of_the_deviation() {
    // BS-05 AC 4, SPEC §10 S5: `pgood` low within 8 us of the ADC code leaving the window
    let waves = FaultsFamily::waves();
    let deviation = waves.first_code_outside_window(FaultsFamily::OVERLOAD).expect("the ADC code never leaves the window");
    let low = waves.pgood().first_time(0, deviation).expect("pgood never falls");
    eprintln!("S5: the code leaves the window at {deviation:e} s, pgood falls {:.3} us later (limit 8 us)", (low - deviation) * 1e6);
    assert!(low - deviation <= 8e-6, "pgood falls {:e} s after the deviation", low - deviation);
}

#[test]
fn s5_both_switches_stay_off_in_hiccup() {
    // BS-05 AC 5, SPEC §8: both gate commands low in every row of the hiccup, from one grid row
    // after the entry as in S6 to S8: the supervisor's commit and the gates' own update take 20 ns
    // (§10.1), and the row of the entry still shows the last period's high-side command
    let (waves, entry, retry) = (FaultsFamily::waves(), FaultsFamily::over_current_entry(), FaultsFamily::retry());
    let from = entry + Timing::GRID;
    let (high, low) = (waves.high_side().between(from, retry).holds(0), waves.low_side().between(from, retry).holds(0));
    eprintln!("S5: hs_cmd {high:?}, ls_cmd {low:?} over {from:e} s to {retry:e} s");
    assert!(high.is_ok(), "the high side departs: {high:?}");
    assert!(low.is_ok(), "the low side departs: {low:?}");
}

#[test]
fn s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on() {
    // BS-05 AC 6, SPEC §8.5: 1000 samples, so SoftStart 2.000 ms after the entry, within a grid
    // row, while the overload is still on
    let (entry, retry) = (FaultsFamily::over_current_entry(), FaultsFamily::retry());
    eprintln!("S5: SoftStart {:.4} ms after the entry into Hiccup (2.000 ms, one grid row); the overload ends at 3.32 ms, the retry is at {retry:e} s", (retry - entry) * 1e3);
    assert!((retry - entry - 2e-3).abs() <= Timing::GRID + 1e-12, "SoftStart {:e} s after the entry", retry - entry);
    assert!(retry < FaultsFamily::OVERLOAD_REMOVED, "the overload is already removed at the retry, {retry:e} s");
}

#[test]
fn s5_retry_without_the_overload_regulates_and_clears_the_fault() {
    // BS-05 AC 7, SPEC §8.5: the overload goes during the retry's soft-start, `Regulate` follows
    // and `fault` reads `Clear` from that row on, until `inject` begins S6
    let (waves, retry) = (FaultsFamily::waves(), FaultsFamily::retry());
    let regulate = waves.mode().first_time(Supervisor::REGULATE, retry).expect("mode never reaches Regulate after the retry");
    let held = waves.fault().between(regulate, FaultsFamily::INJECT).holds(Supervisor::CLEAR);
    eprintln!("S5: Regulate at {regulate:e} s; fault from there to inject: {held:?}");
    assert!(regulate < FaultsFamily::INJECT, "Regulate at {regulate:e} s, after inject");
    assert!(held.is_ok(), "fault departs from Clear: {held:?}");
}

#[test]
fn s6_sink_limit_trips() {
    // BS-05 AC 8, SPEC §8.3: the back-drive trips the sink limit before the over-voltage Hiccup
    let (waves, entry) = (FaultsFamily::waves(), FaultsFamily::over_voltage_entry());
    let first = waves.sink_limit().between(FaultsFamily::INJECT, entry).first_time(1, FaultsFamily::INJECT);
    eprintln!("S6: buck.sink_oc first high at {first:?} (inject at 4.50 ms, Hiccup at {entry:e} s)");
    assert!(first.is_some(), "buck.sink_oc never rises between inject and the Hiccup at {entry:e} s");
}

#[test]
fn s6_inductor_current_is_held_at_minus_3_a() {
    // BS-05 AC 9, SPEC §8.3 and §10 S6: the sink limit holds the current at -3 A, within the
    // reaction bound of 0.1 A (§10.1)
    let (waves, entry) = (FaultsFamily::waves(), FaultsFamily::over_voltage_entry());
    let lowest = waves.inductor_current().between(FaultsFamily::INJECT, entry).minimum();
    eprintln!("S6: the inductor current reaches {:.4} A at {:e} s (-3.0 A +- 0.1 A)", lowest.value, lowest.time);
    assert!((lowest.value + 3.0).abs() <= 0.1, "the lowest inductor current is {} A at {:e} s", lowest.value, lowest.time);
}

#[test]
fn s6_over_voltage_within_two_samples_of_3_81_v() {
    // BS-05 AC 10, SPEC §8.4 and §10.1: Hiccup with OverVoltage no earlier than the first row of
    // vout above 3.81 V and within 4.52 us of it
    let (level, entry) = (FaultsFamily::over_voltage_level_reached(), FaultsFamily::over_voltage_entry());
    eprintln!("S6: vout reaches 3.81 V at {level:e} s, Hiccup with OverVoltage {:.3} us later (0 to 4.52 us)", (entry - level) * 1e6);
    assert!((0.0..=Timing::TWO_SAMPLES).contains(&(entry - level)), "Hiccup {:e} s after vout reaches 3.81 V", entry - level);
}

#[test]
fn s6_reports_its_timing_and_rise_rate() {
    // BS-05 AC 11, SPEC §10 S6 and §10.1: the time from inject to the Hiccup (about 80 to 120 us)
    // and the mean rise of vout from 3.4 V to 3.8 V (about 7 mV/us) are estimates, reported only
    let (waves, entry) = (FaultsFamily::waves(), FaultsFamily::over_voltage_entry());
    let vout = waves.vout();
    let (low, high) = (vout.first_crossing(3.4, Crossing::Rising, FaultsFamily::INJECT), vout.first_crossing(3.8, Crossing::Rising, FaultsFamily::INJECT));
    let (low, high) = (low.expect("vout never reaches 3.4 V after inject"), high.expect("vout never reaches 3.8 V after inject"));
    let rate = (3.8 - 3.4) / (high - low) * 1e-3;
    eprintln!("S6: {:.2} us from inject to the over-voltage Hiccup (SPEC estimates 80 to 120 us); vout rises {:.3} mV/us from 3.4 V to 3.8 V (SPEC estimates 7 mV/us)", (entry - FaultsFamily::INJECT) * 1e6, rate);
    assert!(entry > FaultsFamily::INJECT && rate.is_finite() && rate > 0.0, "time {:e} s, rise rate {rate} mV/us", entry - FaultsFamily::INJECT);
}

#[test]
fn s6_both_switches_stay_off_after_over_voltage() {
    // BS-05 AC 12, SPEC §8: both gate commands low from one grid row after the entry to the end
    let (waves, from) = (FaultsFamily::waves(), FaultsFamily::over_voltage_entry() + Timing::GRID);
    let end = waves.ran.table.rows.last().map_or(0.0, |row| match row[0] {
        Cell::Real(time) => time + Timing::GRID,
        _ => 0.0,
    });
    let (high, low) = (waves.high_side().between(from, end).holds(0), waves.low_side().between(from, end).holds(0));
    eprintln!("S6: hs_cmd {high:?}, ls_cmd {low:?} from {from:e} s to the end");
    assert!(high.is_ok(), "the high side departs: {high:?}");
    assert!(low.is_ok(), "the low side departs: {low:?}");
}

#[test]
fn faults_passes_every_check() {
    // BS-08 AC 4, SPEC §10 S10
    FaultsFamily::assert_every_check_passed();
}

// ---------------------------------------------------------------------------------------------
// The brown-out family: S7 (BS-06 AC 1 to AC 5)
// ---------------------------------------------------------------------------------------------

/// S7 on one run: Vin falls from 12 V to 7.5 V at 50 mV/µs, holds, and rises back.
struct BrownoutFamily;

impl BrownoutFamily {
    /// Vin starts to fall.
    const FALL: f64 = 1.25e-3;
    /// Vin starts to rise again, after 60 µs at 7.5 V (§10.1).
    const RISE: f64 = 1.40e-3;
    /// The 90 µs a 4.5 V ramp takes at 50 mV/µs (§10.1).
    const RAMP: f64 = 90e-6;
    /// The UVLO falling threshold (§8.1).
    const UVLO_FALLING: f64 = 8.0;
    /// The UVLO rising threshold (§8.1).
    const UVLO_RISING: f64 = 8.5;

    /// The first time `buck.vin` is at or below 8.0 V.
    fn falls_through_8_v() -> f64 {
        Self::waves().vin().first_crossing(Self::UVLO_FALLING, Crossing::Falling, Self::FALL).expect("buck.vin never falls through 8.0 V")
    }

    /// The first time `buck.vin` is at or above 8.5 V after it fell through 8.0 V.
    fn rises_through_8_5_v() -> f64 {
        Self::waves().vin().first_crossing(Self::UVLO_RISING, Crossing::Rising, Self::falls_through_8_v()).expect("buck.vin never rises through 8.5 V")
    }

    /// The time `mode` first reads `Off` after Vin starts to fall.
    fn off_entry() -> f64 {
        Self::waves().mode().first_time(Supervisor::OFF, Self::FALL).expect("mode never reads Off in the brown-out")
    }

    /// The time `mode` first reads `SoftStart` after the entry into `Off`.
    fn soft_start_entry() -> f64 {
        Self::waves().mode().first_time(Supervisor::SOFT_START, Self::off_entry()).expect("mode never reads SoftStart after the brown-out")
    }
}

impl Family for BrownoutFamily {
    const RUN: &'static str = "brownout";

    fn stimulus() -> Stimulus {
        let mut host = BuckHost::at_rest();
        host.enable(1e-6).line(12.0, 7.5, Self::FALL, Self::RAMP).line(7.5, 12.0, Self::RISE, Self::RAMP);
        host.stimulus()
    }
}

#[test]
fn brownout_regulates_at_3_3_v_with_pgood_before_its_first_stimulus() {
    BrownoutFamily::assert_regulating_before_the_first_stimulus();
}

#[test]
fn s7_off_at_the_sample_after_vin_falls_through_8_v() {
    // BS-06 AC 1, SPEC §8.1 and §10.1: `Off` within 2.02 us after the crossing, and not before
    let (crossing, off) = (BrownoutFamily::falls_through_8_v(), BrownoutFamily::off_entry());
    eprintln!("S7: buck.vin falls through 8.0 V at {crossing:e} s, mode reads Off {:.3} us later (0 to 2.02 us)", (off - crossing) * 1e6);
    assert!((0.0..=Timing::NEXT_SAMPLE).contains(&(off - crossing)), "mode reads Off {:e} s after the crossing", off - crossing);
}

#[test]
fn s7_both_switches_off_while_off() {
    // BS-06 AC 2, SPEC §8: both gate commands low in every `Off` row from one grid row after the entry
    let (waves, from, to) = (BrownoutFamily::waves(), BrownoutFamily::off_entry() + Timing::GRID, BrownoutFamily::soft_start_entry());
    let (high, low) = (waves.high_side().between(from, to).holds(0), waves.low_side().between(from, to).holds(0));
    eprintln!("S7: hs_cmd {high:?}, ls_cmd {low:?} over {from:e} s to {to:e} s");
    assert!(high.is_ok(), "the high side departs: {high:?}");
    assert!(low.is_ok(), "the low side departs: {low:?}");
}

#[test]
fn s7_pgood_low_while_off() {
    // BS-06 AC 3, SPEC §8.6: `pgood` low in every `Off` row
    let (waves, from, to) = (BrownoutFamily::waves(), BrownoutFamily::off_entry(), BrownoutFamily::soft_start_entry());
    let held = waves.pgood().between(from, to).holds(0);
    eprintln!("S7: pgood over {from:e} s to {to:e} s: {held:?}");
    assert!(held.is_ok(), "pgood departs from low: {held:?}");
}

#[test]
fn s7_fault_unchanged_while_off() {
    // BS-06 AC 4, SPEC §8.1: UVLO is not a fault, so `fault` keeps its value (`Clear`)
    let (waves, from, to) = (BrownoutFamily::waves(), BrownoutFamily::off_entry(), BrownoutFamily::soft_start_entry());
    let held = waves.fault().between(from, to).holds(Supervisor::CLEAR);
    eprintln!("S7: fault over {from:e} s to {to:e} s: {held:?}");
    assert!(held.is_ok(), "fault departs from Clear: {held:?}");
}

#[test]
fn s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v() {
    // BS-06 AC 5, SPEC §8.1 and §10.1: `SoftStart` within 2.02 us after the crossing, `Off` until it
    let (crossing, off, soft) = (BrownoutFamily::rises_through_8_5_v(), BrownoutFamily::off_entry(), BrownoutFamily::soft_start_entry());
    let held = BrownoutFamily::waves().mode().between(off, crossing).holds(Supervisor::OFF);
    eprintln!("S7: buck.vin rises through 8.5 V at {crossing:e} s, SoftStart {:.3} us later (0 to 2.02 us); mode over the gap: {held:?}", (soft - crossing) * 1e6);
    assert!(held.is_ok(), "mode leaves Off before the crossing: {held:?}");
    assert!((0.0..=Timing::NEXT_SAMPLE).contains(&(soft - crossing)), "mode reads SoftStart {:e} s after the crossing", soft - crossing);
}

#[test]
fn brownout_passes_every_check() {
    // BS-08 AC 4, SPEC §10 S10
    BrownoutFamily::assert_every_check_passed();
}

// ---------------------------------------------------------------------------------------------
// The steady-state family: S9 at nine corners (BS-07)
// ---------------------------------------------------------------------------------------------

/// One corner of S9: a supply, a setpoint and the window over which the output is measured.
#[derive(Debug, Clone, Copy)]
struct Corner {
    /// The corner's place in the run, 1 to 9.
    number: usize,
    /// The supply, in volts.
    supply: f64,
    /// The setpoint, in volts (§7.1).
    setpoint: f64,
    /// The DC error SPEC §4.3 allows at this setpoint, in volts.
    bound: f64,
    /// The first time of the window.
    from: f64,
    /// The end of the window (excluded).
    to: f64,
}

/// What one corner's window shows.
struct Measured {
    /// The mean of `vout`.
    mean: f64,
    /// The peak-to-peak of `vout`.
    ripple: f64,
    /// The peak-to-peak of the ADC code.
    code_swing: i128,
    /// The difference between the means of the window's two halves.
    drift: f64,
}

impl Corner {
    /// The one-code bound on an unsettled window: 6.25 mV (§4.1, BS-07 AC 4).
    const ONE_CODE: f64 = 6.25e-3;

    /// What the window shows in `waves`.
    fn measure(&self, waves: &Waveforms<'_>) -> Measured {
        let vout = waves.vout().between(self.from, self.to);
        let middle = (self.from + self.to) / 2.0;
        let (first, second) = (waves.vout().between(self.from, middle).mean(), waves.vout().between(middle, self.to).mean());
        Measured { mean: vout.mean(), ripple: vout.peak_to_peak(), code_swing: waves.code().between(self.from, self.to).peak_to_peak(), drift: (first - second).abs() }
    }

    /// The line a corner prints, failing or not.
    fn describe(&self, measured: &Measured) -> String {
        format!(
            "corner {} (Vin {} V, {} V): mean error {:.2} mV (bound {:.1} mV), ripple {:.2} mV (10 mV), code swing {} (1), half-window drift {:.2} mV (6.25 mV), window {:e} s to {:e} s",
            self.number,
            self.supply,
            self.setpoint,
            (measured.mean - self.setpoint) * 1e3,
            self.bound * 1e3,
            measured.ripple * 1e3,
            measured.code_swing,
            measured.drift * 1e3,
            self.from,
            self.to
        )
    }
}

/// The schedule of S9 and the corners it measures, built together so a window and the stimulus
/// that precedes it can never disagree.
struct SteadyStatePlan {
    /// The stimulus built so far.
    host: BuckHost,
    /// The corners to measure, in plan order.
    corners: Vec<Corner>,
    /// The end of the last window or step: where the next action starts.
    cursor: f64,
    /// The supply voltage the plan has reached.
    supply: f64,
    /// The reference code the plan has written.
    setpoint_code: i64,
}

impl SteadyStatePlan {
    /// Time after a window or a step before the next window starts: 450 µs, about 4 time constants
    /// of the loop's slowest closed-loop pair; the worst ramp's 9.2-code tracking lag (corner 8)
    /// needs 3.6 of them to fall under a quarter code (§10.1).
    const SETTLE: f64 = 450e-6;
    /// How long a supply ramp takes. SPEC §10.1's 10 µs line step is S4's, at 2 A; at full load and
    /// 5.0 V a 9 V to 12 V step in 10 µs sends the inductor current to the 7.5 A limit for eight
    /// periods and trips `OverCurrent` (§8.2), so S9 reaches a corner by a ramp the loop follows.
    const LINE: f64 = 200e-6;
    /// The length of a window: 32 periods (§4.2).
    const WINDOW: f64 = 64e-6;
    /// `en` rises, after the first write.
    const ENABLE: f64 = 2e-6;
    /// The first write's edge.
    const FIRST_WRITE: f64 = 1e-6;
    /// The time from `en` to the first ramp step: the supervisor leaves `Off` at the next sample
    /// and the reference takes its first step at the one after, plus a sample to settle on it.
    const STARTUP_LATENCY: f64 = 3.0 * Timing::SAMPLE;
    /// ADC codes per setpoint code: 25 mV over 6.25 mV (§7.1).
    const CODES_PER_SETPOINT: i64 = 4;
    /// The setpoint, in volts, of a code (§7.1).
    const VOLTS_PER_CODE: f64 = 0.025;
    /// The full-load resistance at a setpoint of `volts`: 5 A (§10 S9).
    const FULL_LOAD_CURRENT: f64 = 5.0;

    /// The plan: start-up at 0.8 V and 9 V, then the nine corners in the order of BS-07.
    fn new() -> SteadyStatePlan {
        let mut plan = SteadyStatePlan { host: BuckHost::at_rest(), corners: Vec::new(), cursor: 0.0, supply: 9.0, setpoint_code: 0 };
        plan.start_up();
        for supply in [12.0, 16.0] {
            plan.measure(6.5e-3);
            plan.step_line(supply);
        }
        plan.measure(6.5e-3);
        plan.move_setpoint(100);
        for supply in [12.0, 9.0] {
            plan.measure(14.7e-3);
            plan.step_line(supply);
        }
        plan.measure(14.7e-3);
        plan.move_setpoint(168);
        for supply in [12.0, 16.0] {
            plan.measure(20e-3);
            plan.step_line(supply);
        }
        plan.measure(20e-3);
        plan
    }

    /// Full-load resistance at the setpoint code `code`.
    fn full_load(code: i64) -> f64 {
        (0.8 + SteadyStatePlan::VOLTS_PER_CODE * code as f64) / SteadyStatePlan::FULL_LOAD_CURRENT
    }

    /// Writes 0 (0.8 V) at 9 V and full load, enables, and waits out the 128-code start-up ramp.
    fn start_up(&mut self) {
        self.host.supply(9.0, 0.0).load(SteadyStatePlan::full_load(0), 0.0);
        self.host.write_setpoint(0, SteadyStatePlan::FIRST_WRITE).enable(SteadyStatePlan::ENABLE);
        let ramp = 128.0 * Timing::SAMPLE;
        self.cursor = SteadyStatePlan::ENABLE + SteadyStatePlan::STARTUP_LATENCY + ramp;
    }

    /// Records the corner whose window starts `SETTLE` after the cursor.
    fn measure(&mut self, bound: f64) {
        let from = self.cursor + SteadyStatePlan::SETTLE;
        let setpoint = 0.8 + SteadyStatePlan::VOLTS_PER_CODE * self.setpoint_code as f64;
        self.corners.push(Corner { number: self.corners.len() + 1, supply: self.supply, setpoint, bound, from, to: from + SteadyStatePlan::WINDOW });
        self.cursor = from + SteadyStatePlan::WINDOW;
    }

    /// Ramps the supply to `volts` from the cursor.
    fn step_line(&mut self, volts: f64) {
        self.host.line(self.supply, volts, self.cursor, SteadyStatePlan::LINE);
        self.supply = volts;
        self.cursor += SteadyStatePlan::LINE;
    }

    /// Writes `code` at the cursor, moves the load to its full value there, and waits out the
    /// ramp (one ADC code per sample, §7.2) and one sample more.
    fn move_setpoint(&mut self, code: i64) {
        self.host.write_setpoint(code, self.cursor).load(SteadyStatePlan::full_load(code), self.cursor);
        let codes = (code - self.setpoint_code).abs() * SteadyStatePlan::CODES_PER_SETPOINT;
        self.cursor += codes as f64 * Timing::SAMPLE + Timing::SAMPLE;
        self.setpoint_code = code;
    }
}

/// S9 on one run: nine corners of supply and setpoint at full load.
struct SteadyStateFamily;

impl SteadyStateFamily {
    /// The nine corners.
    fn corners() -> Vec<Corner> {
        SteadyStatePlan::new().corners
    }

    /// What every corner's window shows, printed one line per corner.
    fn measured() -> Vec<(Corner, Measured)> {
        let waves = Self::waves();
        let measured: Vec<(Corner, Measured)> = Self::corners().into_iter().map(|corner| (corner, corner.measure(&waves))).collect();
        measured.iter().for_each(|(corner, measured)| eprintln!("S9 {}", corner.describe(measured)));
        measured
    }

    /// Fails naming each corner that `broken` describes; every corner is printed first.
    fn assert_every_corner(rule: &str, broken: impl Fn(&Corner, &Measured) -> Option<String>) {
        let failures: Vec<String> = Self::measured().iter().filter_map(|(corner, measured)| broken(corner, measured)).collect();
        assert!(failures.is_empty(), "{rule}: {}", failures.join("; "));
    }

    /// The corners whose halves differ by more than one code: not settled (BS-07 AC 4).
    fn unsettled(corner: &Corner, measured: &Measured) -> Option<String> {
        (measured.drift > Corner::ONE_CODE).then(|| format!("corner {} is not settled: its half-window means differ by {:.2} mV", corner.number, measured.drift * 1e3))
    }
}

impl Family for SteadyStateFamily {
    const RUN: &'static str = "steady_state";

    fn stimulus() -> Stimulus {
        SteadyStatePlan::new().host.stimulus()
    }
}

#[test]
fn s9_every_corner_is_settled() {
    // BS-07 AC 4: the means of a window's two halves differ by at most one code (6.25 mV)
    SteadyStateFamily::assert_every_corner("a corner is not settled", SteadyStateFamily::unsettled);
}

#[test]
fn s9_dc_error_within_section_4_3_at_every_corner() {
    // BS-07 AC 1, SPEC §4.3: the mean of vout is within the total of its setpoint; a corner that
    // is not settled is named as such before its error is judged (BS-07 AC 4)
    SteadyStateFamily::assert_every_corner("a corner misses its DC error", |corner, measured| {
        SteadyStateFamily::unsettled(corner, measured).or_else(|| ((measured.mean - corner.setpoint).abs() > corner.bound).then(|| format!("corner {} is {:.2} mV from {} V (bound {:.1} mV)", corner.number, (measured.mean - corner.setpoint) * 1e3, corner.setpoint, corner.bound * 1e3)))
    });
}

#[test]
fn s9_ripple_at_most_10_mv_at_every_corner() {
    // BS-07 AC 2, SPEC §2 and §10 S9: the peak-to-peak of vout is at most 10 mV
    SteadyStateFamily::assert_every_corner("a corner rings", |corner, measured| (measured.ripple > 10e-3).then(|| format!("corner {} has {:.2} mV peak to peak", corner.number, measured.ripple * 1e3)));
}

#[test]
fn s9_no_idle_tone_above_1_code_at_every_corner() {
    // BS-07 AC 3, SPEC §6.5 and §10 S9: the ADC code moves by at most one code
    SteadyStateFamily::assert_every_corner("a corner has an idle tone", |corner, measured| (measured.code_swing > 1).then(|| format!("corner {} has a code swing of {}", corner.number, measured.code_swing)));
}

#[test]
fn steady_state_passes_every_check() {
    // BS-08 AC 4, SPEC §10 S10
    SteadyStateFamily::assert_every_check_passed();
}
