//! `noise`, `white_noise` and `flicker_noise` run from MSL source on the Rust backend through the
//! host library (analog-operators AO-03 and AO-04, Part IV §14.8): each bench is written in
//! `tests/operators/noise.msl`, opened as a session and swept by the noise analysis from 1 kHz to
//! 1 MHz at one point per decade (1e3, 1e4, 1e5, 1e6). Every expected density is a closed form
//! of the circuit, written beside its assertion.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_solver::protocol::ObservationKind;
use piperine_transpile::Settings;

#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

/// A sink that keeps nothing: these tests read the report.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// The benches of `tests/operators/noise.msl` and the noise runs the tests make of them.
struct Hiss;

impl Hiss {
    /// The frequencies of every run.
    const FREQUENCIES: [f64; 4] = [1e3, 1e4, 1e5, 1e6];

    /// The compile cache the runs share.
    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("a temp dir"));
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    fn path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/noise.msl")
    }

    /// The noise run of `top` measuring `output`: its session and table, or the failure.
    fn run(top: &str, output: &str) -> Result<(Session, Table), Failure> {
        let project = Project::file(&Self::path()).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some(top.into()), analysis: Some("noise".into()), output: Some(output.into()), from: Some(1e3), to: Some(1e6), per_decade: Some(1), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Self::settings())?;
        let table = session.run(Rc::new(RefCell::new(Ignore)))?;
        Ok((session, table))
    }

    /// The table of the run `overrides` of `top`, printing `signal`.
    fn table(top: &str, signal: &str, overrides: Overrides) -> Table {
        let project = Project::file(&Self::path()).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some(top.into()), print: vec![signal.into()], ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Self::settings()).unwrap_or_else(|f| panic!("{f}"));
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"))
    }

    /// The AC run of `top` printing `signal` at 1 kHz: its table.
    fn ac_run(top: &str, signal: &str) -> Table {
        Self::table(top, signal, Overrides { analysis: Some("ac".into()), from: Some(1e3), to: Some(1e3), points: Some(1), ..Overrides::default() })
    }

    /// The output density at each frequency of the noise run of `top` measuring `output`.
    fn density(top: &str, output: &str) -> Vec<f64> {
        let (session, _) = Self::run(top, output).unwrap_or_else(|f| panic!("{top}: {f}"));
        session.noise_report().expect("a report").output.clone()
    }

    /// Asserts each density equals its closed form to `relative`.
    fn assert_densities(what: &str, got: &[f64], wanted: impl Fn(f64) -> f64, relative: f64) {
        assert_eq!(got.len(), Self::FREQUENCIES.len(), "{what}");
        for (got, f) in got.iter().zip(Self::FREQUENCIES) {
            let want = wanted(f);
            assert!((got - want).abs() <= relative * want.abs().max(f64::MIN_POSITIVE), "{what} at {f} Hz: {got:e}, closed form {want:e}");
        }
    }
}

#[test]
fn white_noise_is_a_source_of_its_density() {
    // AO-04 AC 1: a resistor's `4 k T / r` current noise into itself gives `v = r i`: the output
    // density is `sqrt(4 k T r)` at every frequency, T = 300.15 K (the session's 27 C).
    let (k, t, r) = (1.380649e-23_f64, 300.15_f64, 1e3_f64);
    Hiss::assert_densities("white", &Hiss::density("ThermalBench", "o"), |_| (4.0 * k * t * r).sqrt(), 1e-9);
}

#[test]
fn flicker_noise_falls_as_one_over_f_to_the_exponent() {
    // AO-04 AC 2: `flicker_noise(1e-18, 1.0)` is a current density `1e-18 / f`: the output density
    // is `r * sqrt(1e-18 / f)`.
    Hiss::assert_densities("flicker", &Hiss::density("FlickerBench", "o"), |f| 1e3 * (1e-18 / f).sqrt(), 1e-9);
}

#[test]
fn two_sources_add_their_powers() {
    // AO-03 AC 3: two instances, 1e-18 and 3e-18 A^2/Hz, into 1 kohm: `r * sqrt(1e-18 + 3e-18)`.
    Hiss::assert_densities("two sources", &Hiss::density("TwoSourcesBench", "o"), |_| 1e3 * (4e-18_f64).sqrt(), 1e-9);
}

#[test]
fn two_sources_sharing_a_name_stay_independent() {
    // AO-03 AC 8: two sources named `hiss` add as powers, not as amplitudes (`sqrt(1e-18 + 3e-18)`,
    // not `sqrt(1e-18) + sqrt(3e-18)`), and the report holds them as the one group `hiss`.
    let (session, _) = Hiss::run("SameNameBench", "o").unwrap_or_else(|f| panic!("{f}"));
    let report = session.noise_report().expect("a report");
    Hiss::assert_densities("same name", &report.output, |_| 1e3 * (4e-18_f64).sqrt(), 1e-9);
    let names: Vec<&str> = report.groups.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["hiss"], "one group by name");
    Hiss::assert_densities("the group", &report.groups[0].1, |_| 1e3 * (4e-18_f64).sqrt(), 1e-9);
}

#[test]
fn one_source_read_twice_is_fully_correlated() {
    // AO-03 AC 2: one `noise` read by two contributions feeds two nodes with the same value: their
    // difference is 0 (a correlated source), each alone is `r * sqrt(psd)`.
    let differential = Hiss::density("CorrelatedBench", "v(a, b)");
    assert!(differential.iter().all(|d| d.abs() <= 1e-18), "fully correlated: {differential:?}");
    Hiss::assert_densities("a alone", &Hiss::density("CorrelatedBench", "a"), |_| 1e3 * (1e-18_f64).sqrt(), 1e-9);
}

#[test]
fn a_density_reading_freq_is_evaluated_at_each_sweep_frequency() {
    // AO-03 AC 4: `noise(1e-20 * freq())` is read at each sweep frequency, never at 0 (the
    // operating point runs without reading it): the density is `r * sqrt(1e-20 f)`.
    Hiss::assert_densities("freq density", &Hiss::density("FrequencyDensityBench", "o"), |f| 1e3 * (1e-20 * f).sqrt(), 1e-9);
}

#[test]
fn noise_through_ddt_and_delay_takes_their_response() {
    // AO-03 AC 5: `i +> ddt(1n * n)` gives the response `j w c`: the density is `r w c sqrt(1e-12)`;
    // `delay(n, 1 us)` the response `exp(-j w td)`, whose magnitude is 1: `r sqrt(1e-12)`.
    let two_pi = std::f64::consts::TAU;
    Hiss::assert_densities("ddt", &Hiss::density("ThroughDdtBench", "o"), |f| 1e3 * two_pi * f * 1e-9 * 1e-6, 1e-9);
    Hiss::assert_densities("delay", &Hiss::density("ThroughDelayBench", "o"), |_| 1e3 * 1e-6, 1e-9);
}

#[test]
fn a_negative_density_fails_with_e0454_naming_the_frequency() {
    // AO-03 AC 6: `noise(-1.0)` is read by the sweep alone: the run fails with `E0454` naming the
    // source and the frequency, the first of the sweep.
    let Err(failure) = Hiss::run("NegativeDensityBench", "o") else { panic!("a negative density ran") };
    let message = failure.to_string();
    assert!(message.contains("E0454"), "{message}");
    assert!(message.contains("`bad`"), "names the source: {message}");
    assert!(message.contains("1000 Hz"), "names the frequency: {message}");
}

#[test]
fn a_density_that_is_not_finite_fails_with_e0454_naming_the_frequency() {
    // AO-03 AC 6: a density that is not finite (`1.0 / 0.0` or NaN) at a frequency of the sweep fails like a
    // negative one: `E0454` naming the source and the first frequency.
    for bench in ["InfiniteDensityBench", "NanDensityBench"] {
        let Err(failure) = Hiss::run(bench, "o") else { panic!("{bench}: a density that is not finite ran") };
        let message = failure.to_string();
        assert!(message.contains("E0454"), "{bench}: {message}");
        assert!(message.contains("`unbounded`"), "{bench}, names the source: {message}");
        assert!(message.contains("1000 Hz"), "{bench}, names the frequency: {message}");
    }
}

#[test]
fn a_let_reading_a_limit_is_a_density_input() {
    // AO-03 (§14 rule 5, §14.7.4): a density that reads a `let` bound to a `limit` is read at the operating
    // point, where the limited iterate is the unlimited one: 1 V on the node, density `1e-18 * 1^2`, so the
    // output is `r * sqrt(1e-18)`.
    Hiss::assert_densities("limit", &Hiss::density("LimitedDensityBench", "o"), |_| 1e3 * (1e-18_f64).sqrt(), 1e-9);
}

#[test]
fn analysis_noise_is_true_and_ac_stim_zero_in_a_noise_run() {
    // AO-05 AC 7: `analysis(Noise)` is true in the sweep, so the density is its 1e-18 branch: the
    // output density is `r * sqrt(1e-18)`, not the 1e-16 of the other. It is true in the operating point
    // too: the source of `OperatingPointBench` is 2 V there (1 V elsewhere), and the density reads it, so
    // the output is `r * sqrt(1e-18 * 2^2)`. `ac_stim` is 0 in the sweep: the density of a bench that
    // drives a stimulus of 1 A into the node stays `r * sqrt(1e-18)` although the density itself reads an
    // `ac_stim` of 3 (`1e-18 * (1 + ac_stim(3))`, so a sweep that kept the stimulus would give 4e-18), while
    // its AC run reads the 1 kV the stimulus gives (so the stimulus is real, and the sweep leaves it out).
    Hiss::assert_densities("analysis(Noise)", &Hiss::density("AnalysisBench", "o"), |_| 1e3 * (1e-18_f64).sqrt(), 1e-9);
    Hiss::assert_densities("operating point", &Hiss::density("OperatingPointBench", "o"), |_| 1e3 * (4e-18_f64).sqrt(), 1e-9);
    Hiss::assert_densities("ac_stim", &Hiss::density("StimulusBench", "o"), |_| 1e3 * (1e-18_f64).sqrt(), 1e-9);
    let ac = Hiss::ac_run("StimulusBench", "o");
    let Cell::Real(magnitude) = ac.rows[0][1] else { panic!("a real magnitude in {:?}", ac.rows[0]) };
    assert!((magnitude - 1e3).abs() <= 1e-6, "the AC run reads the stimulus: {magnitude}");
}

#[test]
fn a_noise_source_is_zero_outside_a_noise_sweep_and_never_reads_its_density() {
    // AO-03 AC 1: `i(gnd, out) +> 1m + noise(-1.0)` into 1 kohm is 1 V at the operating point, in an
    // AC run (no stimulus, so 0 amplitude) and along a transient, as without the source: the negative density is read by no solve but a noise
    // sweep (which fails, the test above).
    let operating_point = Hiss::table("NegativeDensityBench", "o", Overrides { analysis: Some("op".into()), ..Overrides::default() });
    assert!(matches!(operating_point.rows[0][1], Cell::Real(v) if (v - 1.0).abs() <= 1e-9), "v(o) of the operating point: {operating_point:?}");
    let ac = Hiss::ac_run("NegativeDensityBench", "o");
    assert!(ac.rows.iter().all(|row| matches!(row[1], Cell::Real(magnitude) if magnitude == 0.0)), "the AC run has no stimulus and the source adds none: {ac:?}");
    let built = support::build(&piperine_parity::project::Project::new(&Hiss::path()), "NegativeDensityBench");
    let out = piperine_solver::prelude::NodeIdentifier::Anonymous(built.ir.node_at("NegativeDensityBench.o").expect("node o").id().index());
    let (_, result) = built.run_with(1e-6, 0.1e-6);
    for point in result.iter() {
        assert!((point.get_node(&out).expect("v(o)") - 1.0).abs() <= 1e-9, "v(o) at {} s", point.time());
    }
}

#[test]
fn a_reaction_or_an_event_reads_noise_as_zero() {
    // AO-03 AC 7: `on at(1m) { emit!("n", x = n) }` with `n = noise(1.0)`: the record holds 0.
    let seen = support::build(&piperine_parity::project::Project::new(&Hiss::path()), "WatcherBench").run(2e-3, 0.1e-3);
    let records: Vec<Vec<u8>> = seen
        .all()
        .iter()
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { bytes, .. } => Some(bytes.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(records.len(), 1, "one record: {records:?}");
    assert_eq!(f64::from_le_bytes(records[0][..8].try_into().expect("the field x")), 0.0);
}
