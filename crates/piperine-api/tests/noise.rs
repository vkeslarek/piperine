//! The noise request (analog-operators AO-05): the keys of `analysis = "noise"`, their faults
//! (`P0010`) and the flags that win over them. The run itself is covered by the targets that
//! follow in this file's later sections.

use piperine_api::request::{Analysis, Overrides, RunSpec};

/// The runs of a manifest holding `[run.n]` with `keys`.
fn spec(keys: &str) -> Result<RunSpec, String> {
    let text = format!("[run.n]\nbench = \"b.msl\"\nanalysis = \"noise\"\n{keys}");
    let runs = RunSpec::read(&text, std::path::Path::new("Piperine.toml")).map_err(|failures| failures.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>().join("\n"))?;
    Ok(runs.into_iter().next().expect("one run"))
}

#[test]
fn a_noise_run_without_output_or_sweep_is_p0010() {
    // AO-05 AC 4 and 6: `output` and the sweep (`from`, `to`, one grid) are required.
    for keys in ["from = 1.0\nto = 1e3\nper_decade = 5\n", "output = \"o\"\nto = 1e3\nper_decade = 5\n", "output = \"o\"\nfrom = 1.0\nto = 1e3\n"] {
        let message = spec(keys).expect_err(keys);
        assert!(message.contains("P0010"), "{keys}: {message}");
    }
    let ok = spec("output = \"o\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n").expect("a complete noise run");
    assert!(matches!(ok.analysis, Analysis::Noise { ref output, input: None, .. } if output == "o"));
}

#[test]
fn a_key_of_another_analysis_is_p0010_in_a_noise_run() {
    // AO-05 AC 6: `stop` belongs to `transient`, `wrt` to `sens`: neither is read by `noise`.
    for key in ["stop = 1e-3\n", "wrt = [\"r.r\"]\n", "period = 1e-3\n"] {
        let keys = format!("output = \"o\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n{key}");
        let message = spec(&keys).expect_err(key);
        assert!(message.contains("not read by a `noise` run"), "{key}: {message}");
    }
}

#[test]
fn the_output_and_input_flags_win_over_the_keys() {
    // AO-05 AC 3: `--output` and `--input` replace the table's `output` and `input`.
    let table = spec("output = \"a\"\ninput = \"vin\"\nfrom = 1.0\nto = 1e3\nper_decade = 5\n").expect("run");
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

    #[test]
    fn an_output_or_input_that_names_nothing_is_e0801() {
        // AO-05 AC 5: an output node, a `v(a, b)` leg, an `i(..)` instance and an input that
        // match nothing, and an input with no `ac_stim` (the resistor), are each E0801.
        let probe = open("out", Some("v1")).expect("the output node and the source");
        assert_eq!(probe.noise_probe().and_then(|p| p.input.as_deref()), Some("AcRc.v1"));
        for (output, input) in [("nowhere", None), ("v(out, nowhere)", None), ("i(nobody)", None), ("out", Some("nobody")), ("out", Some("r1"))] {
            match open(output, input) {
                Err(Failure::Host { code, .. }) => assert_eq!(code, "E0801", "{output} {input:?}"),
                Err(other) => panic!("{output} {input:?}: {other}"),
                Ok(_) => panic!("{output} {input:?} opened"),
            }
        }
    }
}
