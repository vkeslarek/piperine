//! The noise request (analog-operators AO-05): the keys of `analysis = "noise"`, their faults
//! (`P0010`) and the flags that win over them. The run itself is covered by the targets that
//! follow in this file's later sections.

use piperine_api::request::{Analysis, Overrides, RunSpec};

/// A manifest with the one run `[run.n]`, a noise run.
struct NoiseManifest;

impl NoiseManifest {
    /// The run `[run.n]` of a manifest holding `keys`, or the faults of reading it.
    fn spec(keys: &str) -> Result<RunSpec, String> {
        let text = format!("[run.n]\nbench = \"b.msl\"\nanalysis = \"noise\"\n{keys}");
        let runs = RunSpec::read(&text, std::path::Path::new("Piperine.toml")).map_err(|failures| failures.iter().map(|failure| format!("{failure:?}")).collect::<Vec<_>>().join("\n"))?;
        Ok(runs.into_iter().next().expect("one run"))
    }
}

#[test]
fn a_noise_run_without_output_or_sweep_is_p0010() {
    // AO-05 AC 4 and 6: `output` and the sweep (`from`, `to`, one grid) are required, and the
    // report names the one that is missing.
    let cases = [
        ("from = 1.0\nto = 1e3\nper_decade = 5\n", "needs `output`"),
        ("output = \"o\"\nto = 1e3\nper_decade = 5\n", "needs `from`"),
        ("output = \"o\"\nfrom = 1.0\nper_decade = 5\n", "needs `to`"),
        ("output = \"o\"\nfrom = 1.0\nto = 1e3\n", "exactly one of `per_decade` and `points`"),
    ];
    for (keys, naming) in cases {
        let message = NoiseManifest::spec(keys).expect_err(keys);
        assert!(message.contains("P0010"), "{keys}: {message}");
        assert!(message.contains(naming), "{keys}: the report names the missing key ({naming}): {message}");
    }
    let ok = NoiseManifest::spec("output = \"o\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n").expect("a complete noise run");
    assert!(matches!(ok.analysis, Analysis::Noise { ref output, input: None, .. } if output == "o"));
}

#[test]
fn a_key_of_another_analysis_is_p0010_in_a_noise_run() {
    // AO-05 AC 6: `stop` belongs to `transient`, `sweep` to `dc`, `wrt` to `sens`: none is read by `noise`.
    for key in ["stop = 1e-3\n", "sweep = \"r.r\"\n", "wrt = [\"r.r\"]\n", "period = 1e-3\n"] {
        let keys = format!("output = \"o\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n{key}");
        let message = NoiseManifest::spec(&keys).expect_err(key);
        assert!(message.contains("P0010"), "{key}: {message}");
        assert!(message.contains("not read by a `noise` run"), "{key}: {message}");
    }
}

#[test]
fn the_output_and_input_flags_win_over_the_keys() {
    // AO-05 AC 3: `--output` and `--input` replace the table's `output` and `input`.
    let table = NoiseManifest::spec("output = \"a\"\ninput = \"vin\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n").expect("run");
    let flags = Overrides { output: Some("b".into()), input: Some("v2".into()), ..Overrides::default() };
    let merged = table.merged(&flags).expect("merge");
    assert!(matches!(merged.analysis, Analysis::Noise { ref output, input: Some(ref input), .. } if output == "b" && input == "v2"));
}

mod resolution {
    //! The output and input of a noise run, resolved when the session opens (before anything
    //! compiles), against the AC RC twin.
    use std::path::{Path, PathBuf};

    use piperine_api::{Failure, Overrides, Project, Session};
    use piperine_transpile::Settings;

    /// The AC RC twin, opened for noise runs.
    struct Twin;

    impl Twin {
        /// Opens a noise run of the AC RC twin with `output` and `input`.
        fn open(output: &str, input: Option<&str>) -> Result<Session, Failure> {
            let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/ac_rc.msl");
            let project = Project::file(&path).unwrap_or_else(|f| panic!("{f}"));
            let overrides = Overrides {
                analysis: Some("noise".into()),
                output: Some(output.into()),
                input: input.map(str::to_string),
                from: Some(1.0),
                to: Some(1e6),
                per_decade: Some(5),
                backend: Some("interp".into()),
                ..Overrides::default()
            };
            let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
            let settings = Settings { rustc: None, cache: std::env::temp_dir() };
            Session::open(&project, &spec, &settings)
        }
    }

    #[test]
    fn an_output_or_input_that_names_nothing_is_e0801() {
        // AO-05 AC 5: an output node, a `v(a, b)` leg, an `i(..)` instance and an input that
        // match nothing, and an input with no `ac_stim` (the resistor), are each E0801.
        let probe = Twin::open("out", Some("v1")).expect("the output node and the source");
        assert_eq!(probe.noise_probe().and_then(|p| p.input.as_deref()), Some("AcRc.v1"));
        for (output, input) in [("nowhere", None), ("v(out, nowhere)", None), ("i(nobody)", None), ("out", Some("nobody")), ("out", Some("r1"))] {
            match Twin::open(output, input) {
                Err(Failure::Host { code, .. }) => assert_eq!(code, "E0801", "{output} {input:?}"),
                Err(other) => panic!("{output} {input:?}: {other}"),
                Ok(_) => panic!("{output} {input:?} opened"),
            }
        }
    }
}

mod run {
    //! A noise run through the session, on the resistor and capacitor of the AC RC twin (neither
    //! models a noise source, so every density is 0): the shape of the table, the summary and the
    //! report.
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::sync::OnceLock;

    use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
    use piperine_transpile::Settings;

    /// A sink that keeps nothing: these tests read the table and the report.
    struct Ignore;

    impl RecordSink for Ignore {
        fn record(&mut self, _: &Record) {}
    }

    /// The compile cache shared by the runs of this module.
    struct Cache;

    impl Cache {
        fn settings() -> Settings {
            static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
            let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
            Settings { rustc: None, cache: dir.path().to_path_buf() }
        }
    }

    /// A finished noise run of the AC RC twin.
    struct NoiseRun;

    impl NoiseRun {
        /// The finished session of a noise run of the AC RC twin from 1 kHz to 1 MHz, 1 point per
        /// decade (`round(1 * 3) + 1 = 4` frequencies), and its table.
        fn run(input: Option<&str>) -> (Session, Table) {
            let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/ac_rc.msl");
            let project = Project::file(&path).unwrap_or_else(|f| panic!("{f}"));
            let overrides = Overrides {
                analysis: Some("noise".into()),
                output: Some("out".into()),
                input: input.map(str::to_string),
                from: Some(1e3),
                to: Some(1e6),
                per_decade: Some(1),
                ..Overrides::default()
            };
            let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
            let mut session = Session::open(&project, &spec, &Cache::settings()).unwrap_or_else(|f| panic!("{f}"));
            let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
            (session, table)
        }
    }

    #[test]
    fn a_noise_run_prints_freq_onoise_and_the_total() {
        // AO-05 AC 1: columns `freq`, `onoise`; one row per frequency, 1e3 to 1e6 at one point
        // per decade is 4 rows; the summary is `onoise_total` alone.
        let (session, table) = NoiseRun::run(None);
        assert_eq!(table.columns, ["freq", "onoise"]);
        let frequencies: Vec<f64> = table.rows.iter().map(|row| match row[0] { Cell::Real(f) => f, _ => panic!("a frequency") }).collect();
        assert_eq!(frequencies.len(), 4);
        let want = [1e3, 1e4, 1e5, 1e6];
        for (got, want) in frequencies.iter().zip(want) {
            assert!((got - want).abs() <= 1e-9 * want, "{got} vs {want}");
        }
        let report = session.noise_report().expect("a report");
        assert_eq!(Table::summary(report), [("onoise_total".to_string(), 0.0)]);
    }

    #[test]
    fn an_input_adds_inoise_and_its_total() {
        // AO-05 AC 2: with `input = v1` the table gains `inoise`, the summary `inoise_total`.
        // A noiseless circuit refers 0 to the input: 0 / |H|^2 = 0.
        let (session, table) = NoiseRun::run(Some("v1"));
        assert_eq!(table.columns, ["freq", "onoise", "inoise"]);
        let report = session.noise_report().expect("a report");
        let names: Vec<String> = Table::summary(report).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["onoise_total", "inoise_total"]);
    }

    #[test]
    fn the_library_returns_rows_totals_and_group_densities() {
        // AO-05 AC 8: the report carries the rows, the totals, the unit and the groups (none: no
        // device of the design has a noise source).
        let (session, _) = NoiseRun::run(Some("v1"));
        let report = session.noise_report().expect("a report");
        assert_eq!(report.frequencies.len(), 4);
        assert_eq!(report.output.len(), 4);
        assert_eq!(report.unit, "V/sqrt(Hz)");
        assert!(report.groups.is_empty(), "{:?}", report.groups);
        assert_eq!(report.output_total, 0.0);
    }

    #[test]
    fn a_design_with_no_noise_source_prints_zero_and_succeeds() {
        // AO-05 edge case: no source means a density of 0 at every frequency, not a failure.
        let (_, table) = NoiseRun::run(None);
        assert!(table.rows.iter().all(|row| row[1] == Cell::Real(0.0)), "{:?}", table.rows);
    }
}

mod report {
    //! `NoiseReport::of` on a result built by hand, so every density is nonzero and the grouping,
    //! the roots and the totals can each be wrong.
    use piperine_api::noise::NoiseReport;
    use piperine_solver::prelude::{NoiseAnalysisResult, NoiseContribution, NoiseKind};

    /// The contributions of a hand-built noise result.
    struct Contributions;

    impl Contributions {
        /// One contribution of `element`, named `source` (empty: unnamed), with the power density `psd`.
        fn contribution(element: &str, source: &str, psd: &[f64]) -> NoiseContribution {
            NoiseContribution { element: element.into(), source: source.into(), kind: NoiseKind::Thermal, integrated_sq: 0.0, psd: psd.to_vec() }
        }
    }

    #[test]
    fn the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals() {
        // AO-05 AC 8: densities are the roots of the solver's powers; contributions sharing a
        // source name are one group whose power is the sum; an unnamed one is grouped by its
        // instance path; groups keep their first-seen order; the totals are carried unchanged.
        let result = NoiseAnalysisResult {
            frequencies: vec![1.0, 10.0],
            out_noise_sq: vec![25.0, 36.0],
            integrated_noise: 0.0,
            output_total: 7.5,
            input_noise_sq: Some(vec![4.0, 9.0]),
            input_total: Some(2.5),
            contributions: vec![
                Contributions::contribution("top.r1", "thermal", &[9.0, 16.0]),
                Contributions::contribution("top.m1", "", &[4.0, 4.0]),
                Contributions::contribution("top.r2", "thermal", &[7.0, 9.0]),
            ],
        };
        let report = NoiseReport::of(&result, "V/sqrt(Hz)");
        assert_eq!(report.frequencies, [1.0, 10.0]);
        assert_eq!(report.output, [5.0, 6.0]);
        assert_eq!(report.input, Some(vec![2.0, 3.0]));
        assert_eq!((report.output_total, report.input_total), (7.5, Some(2.5)));
        assert_eq!(report.unit, "V/sqrt(Hz)");
        // thermal: sqrt(9 + 7) = 4 and sqrt(16 + 9) = 5, a sum of powers, not of amplitudes
        assert_eq!(report.groups, [("thermal".to_string(), vec![4.0, 5.0]), ("top.m1".to_string(), vec![2.0, 2.0])]);
    }
}
