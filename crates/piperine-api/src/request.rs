//! `RunSpec`: one validated run request, whether a `[run.<name>]` table of `Piperine.toml` or
//! the flags of the command line spelled it (CLI-03, CLI-09 AC 3 and 5). Every fault is a
//! `P0010`: at the span of the key, the value or the run's name when the manifest gave it, with
//! no span when a flag did.

use std::path::{Path, PathBuf};

use piperine_lang::msl::Span;
use toml::de::DeTable;

mod grid;
mod options;
mod overrides;
mod rules;
mod run_table;

pub use grid::Grid;
pub use options::Options;
pub use overrides::Overrides;

use crate::failure::Failure;

/// The analysis of a run, with the numbers it reads.
#[derive(Debug, Clone, PartialEq)]
pub enum Analysis {
    /// The operating point.
    Op,
    /// A DC sweep: the param `sweep` at `from + k*step` while not past `to`.
    Dc { sweep: String, from: f64, to: f64, step: f64 },
    /// A small-signal sweep from `from` to `to` hertz.
    Ac { from: f64, to: f64, grid: Grid },
    /// A transient to `stop`; `from` is the first time the result keeps (ngspice's `tstart`).
    Transient { stop: f64, max_step: Option<f64>, print_step: Option<f64>, from: Option<f64> },
    /// The DC transfer function (ngspice `.tf`) from the voltage source `input` to the node `output`.
    Tf { input: String, output: String },
    /// The poles and zeros (ngspice `.pz`) of the transfer from the voltage source `input` to the node `output`.
    Pz { input: String, output: String },
    /// The DC sensitivity (ngspice `.sens`) of the printed signals with respect to each param of `wrt`.
    Sens { wrt: Vec<String> },
    /// The noise analysis: the density of `output` from `from` to `to` hertz; with `input`, also
    /// referred to that source instance (AO-05).
    Noise { output: String, input: Option<String>, from: f64, to: f64, grid: Grid },
    /// The periodic steady state of a circuit driven with period `period`; the orbit starts at
    /// `tstab` (a transient of that length comes first). `max_step` bounds the steps of a shot.
    Pss { period: f64, tstab: f64, max_step: Option<f64>, print_step: Option<f64> },
}

impl Analysis {
    /// The nine names a user writes, in the order they are listed.
    pub const NAMES: [&'static str; 9] = ["op", "dc", "ac", "transient", "tf", "pz", "sens", "pss", "noise"];

    /// The names as a fault lists them: each in backquotes, comma separated.
    fn listed() -> String {
        Analysis::NAMES.map(|name| format!("`{name}`")).join(", ")
    }

    /// The name a user writes.
    pub fn name(&self) -> &'static str {
        match self {
            Analysis::Op => "op",
            Analysis::Dc { .. } => "dc",
            Analysis::Ac { .. } => "ac",
            Analysis::Transient { .. } => "transient",
            Analysis::Tf { .. } => "tf",
            Analysis::Pz { .. } => "pz",
            Analysis::Sens { .. } => "sens",
            Analysis::Pss { .. } => "pss",
            Analysis::Noise { .. } => "noise",
        }
    }
}

/// One run request, validated.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSpec {
    pub name: String,
    pub bench: Option<PathBuf>,
    pub top: Option<String>,
    pub analysis: Analysis,
    pub print: Vec<String>,
    pub streams: Vec<String>,
    pub params: Vec<(String, f64)>,
    /// The params given as text, read by their type when the session opens (`Overrides::param_texts`).
    pub param_texts: Vec<(String, String)>,
    pub backend: Option<String>,
    pub csv: Option<PathBuf>,
    pub records: Option<PathBuf>,
    pub plot: Option<PathBuf>,
    /// Whether the terminal shows the passing `checks` records too (CLI-06 AC 7).
    pub all_checks: bool,
    /// The session options (HII-01), read for every analysis.
    pub options: Options,
    /// What spelled the request, kept so that flags can be merged and validated again.
    keys: Keys,
    /// The manifest the keys were read from; `None` for a request made from flags.
    source: Option<Source>,
}

/// The manifest a request was read from: where a `P0010` points.
#[derive(Debug, Clone, PartialEq)]
struct Source {
    file: PathBuf,
    text: String,
}

/// A value and where the manifest wrote it (`None` when a flag gave it).
#[derive(Debug, Clone, PartialEq)]
struct Given<T> {
    value: T,
    at: Option<Span>,
}

impl<T> Given<T> {
    /// A value a flag gave: no span.
    fn flag(value: T) -> Given<T> {
        Given { value, at: None }
    }
}

/// Every key of a run as given, before the rules of an analysis are applied.
#[derive(Debug, Clone, Default, PartialEq)]
struct Keys {
    name_at: Option<Span>,
    bench: Option<Given<PathBuf>>,
    top: Option<Given<String>>,
    analysis: Option<Given<String>>,
    stop: Option<Given<f64>>,
    max_step: Option<Given<f64>>,
    print_step: Option<Given<f64>>,
    sweep: Option<Given<String>>,
    from: Option<Given<f64>>,
    to: Option<Given<f64>>,
    step: Option<Given<f64>>,
    per_decade: Option<Given<u32>>,
    points: Option<Given<u32>>,
    print: Vec<String>,
    streams: Vec<String>,
    params: Vec<(String, f64)>,
    param_texts: Vec<(String, String)>,
    backend: Option<Given<String>>,
    csv: Option<Given<PathBuf>>,
    records: Option<Given<PathBuf>>,
    plot: Option<Given<PathBuf>>,
    all_checks: Option<Given<bool>>,
    reltol: Option<Given<f64>>,
    abstol: Option<Given<f64>>,
    vntol: Option<Given<f64>>,
    trtol: Option<Given<f64>>,
    temp: Option<Given<f64>>,
    resolution: Option<Given<f64>>,
    maxrounds: Option<Given<u32>>,
    maxmicro: Option<Given<u32>>,
    max_steps: Option<Given<u32>>,
    input: Option<Given<String>>,
    output: Option<Given<String>>,
    wrt: Option<Given<Vec<String>>>,
    period: Option<Given<f64>>,
    tstab: Option<Given<f64>>,
    nodeset: Vec<(String, f64)>,
    simparam: Vec<(String, f64)>,
}

/// The names `simparam` may not set: each has its own key (a solver option or the temperatures).
const OPTION_NAMES: [&str; 5] = ["reltol", "abstol", "vntol", "trtol", "temp"];

/// The lowest temperature, degrees Celsius: `temp` must be above it.
const ABSOLUTE_ZERO: f64 = -273.15;

/// The keys of a run table, in the order CLI-03 AC 1 lists them, then the session options of
/// HII-01.
const RUN_KEYS: [&str; 36] = [
    "bench", "top", "analysis", "stop", "max_step", "print_step", "sweep", "from", "to", "step", "per_decade", "points", "print", "streams",
    "params", "backend", "csv", "records", "plot", "all_checks", "reltol", "abstol", "vntol", "trtol", "temp", "resolution", "maxrounds",
    "maxmicro", "nodeset", "max_steps", "input", "output", "wrt", "period", "tstab", "simparam",
];

/// The keys whose meaning belongs to an analysis; each analysis reads the ones `reads` lists.
const ANALYSIS_KEYS: [&str; 15] = ["stop", "max_step", "print_step", "sweep", "from", "to", "step", "per_decade", "points", "plot", "input", "output", "wrt", "period", "tstab"];

impl RunSpec {
    /// Every `[run.<name>]` of the manifest `text` (the content of `file`), in source order. A
    /// manifest with no `run` table has no run. Every fault of every run is returned.
    pub fn read(text: &str, file: &Path) -> Result<Vec<RunSpec>, Vec<Failure>> {
        let source = Source { file: file.to_path_buf(), text: text.to_string() };
        let table = match DeTable::parse(text) {
            Ok(table) => table,
            Err(e) => {
                let range = e.span().unwrap_or(0..0);
                let span = Span::new(range.start, range.end.max(range.start + 1).min(text.len().max(range.start)));
                return Err(vec![source.fault(Some(span), format!("the manifest is not valid TOML: {}", e.message()), "not valid TOML")]);
            }
        };
        let Some(run) = table.get_ref().get("run") else { return Ok(Vec::new()) };
        let Some(runs) = run.get_ref().as_table() else {
            return Err(vec![source.fault(Some(Keys::span_of(run)), "`run` must be a table of runs, `[run.<name>]`".into(), "expected a table")]);
        };
        let (mut specs, mut faults) = (Vec::new(), Vec::new());
        for (name, value) in Keys::entries(runs) {
            match source.run(name, value) {
                Ok(spec) => specs.push(spec),
                Err(mut more) => faults.append(&mut more),
            }
        }
        if faults.is_empty() { Ok(specs) } else { Err(faults) }
    }

    /// A request made from flags alone: a lone file's run, named `name`.
    pub fn from_flags(name: &str, overrides: &Overrides) -> Result<RunSpec, Failure> {
        let keys = Keys::default().with(overrides);
        keys.spec(name.to_string(), None)
    }

    /// This request with `overrides` applied (a flag over the key it shares), validated again.
    pub fn merged(&self, overrides: &Overrides) -> Result<RunSpec, Failure> {
        let keys = self.keys.clone().with(overrides);
        keys.spec(self.name.clone(), self.source.clone())
    }

    /// Where the manifest wrote the `bench` value; `None` when a flag gave it or there is none.
    pub(crate) fn bench_at(&self) -> Option<Span> {
        self.keys.bench.as_ref().and_then(|b| b.at)
    }

    /// A `P0010` at `at` in the manifest this request was read from, or with no span when the
    /// request has no manifest or `at` is `None`.
    pub(crate) fn fault(&self, at: Option<Span>, message: String, label: &str) -> Failure {
        Source::fault_in(self.source.as_ref(), at, message, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "Piperine.toml";

    /// Associated test helpers of `request`, grouped on one fixture type.
    struct RequestFixture;

    impl RequestFixture {
        /// The manifest text: a `[package]` and then `runs`.
        fn manifest(runs: &str) -> String {
            format!("[package]\nname = \"rc\"\nversion = \"0.1.0\"\n\n{runs}")
        }

        fn read(runs: &str) -> Result<Vec<RunSpec>, Vec<Failure>> {
            RunSpec::read(&RequestFixture::manifest(runs), Path::new(FILE))
        }

        fn one(runs: &str) -> RunSpec {
            let mut all = RequestFixture::read(runs).unwrap_or_else(|f| panic!("{f:?}"));
            assert_eq!(all.len(), 1, "one run");
            all.remove(0)
        }

        /// The single fault of a manifest that has exactly one.
        fn fault(runs: &str) -> Failure {
            let mut faults = RequestFixture::read(runs).expect_err("the run is refused");
            assert_eq!(faults.len(), 1, "one fault: {faults:?}");
            faults.remove(0)
        }

        /// The code, the message and the spanned text of a fault the manifest located.
        fn located(f: &Failure) -> (&str, &str, &str) {
            match f {
                Failure::Request { diagnostic, text, .. } => {
                    let (span, _) = &diagnostic.primary;
                    (&diagnostic.code, &diagnostic.message, &text[span.start..span.end])
                }
                other => panic!("a located P0010, found {other:?}"),
            }
        }

        fn options() -> Options {
            Options {
                reltol: Some(1e-4),
                abstol: Some(1e-13),
                vntol: Some(2e-6),
                trtol: Some(5.0),
                temp: Some(75.0),
                resolution: Some(1e-12),
                maxrounds: Some(30),
                maxmicro: Some(500),
                max_steps: Some(4_000_000),
                nodeset: vec![("Top.a".into(), 1.5), ("Top.b".into(), 0.0)],
                session_parameters: vec![],
                time_point_rule: None,
            }
        }
    }

    #[test]
    fn a_run_table_reads_its_nineteen_keys() {
        // CLI-03 AC 1: each key read where the design's run table puts it. The analysis-bound
        // numbers are spread over three runs, since each analysis reads its own (CLI-03 AC 7).
        let runs = RequestFixture::read(
            r#"
[run.step]
bench = "benches/step.msl"
top = "Step"
analysis = "transient"
stop = 10e-6
max_step = 10e-9
print_step = 1e-6
from = 2e-6
print = ["out", "i(v1)"]
streams = ["trip", "checks"]
params = { "r1.r" = 2000.0, "c1.c" = 1 }
backend = "rust"
csv = "out/step.csv"
records = "out/step.jsonl"
plot = "out/step.svg"
all_checks = true

[run.iv]
analysis = "dc"
sweep = "v1.dc"
from = -1.0
to = 0.8
step = 0.05

[run.bode]
analysis = "ac"
from = 1e3
to = 1e7
per_decade = 10

[run.lin]
analysis = "ac"
from = 1e3
to = 5e3
points = 5
"#,
        )
        .unwrap_or_else(|f| panic!("{f:?}"));
        let names: Vec<&str> = runs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["step", "iv", "bode", "lin"], "source order");
        let step = &runs[0];
        assert_eq!(step.bench.as_deref(), Some(Path::new("benches/step.msl")));
        assert_eq!(step.top.as_deref(), Some("Step"));
        assert_eq!(step.analysis, Analysis::Transient { stop: 10e-6, max_step: Some(10e-9), print_step: Some(1e-6), from: Some(2e-6) });
        assert_eq!(step.print, ["out", "i(v1)"]);
        assert_eq!(step.streams, ["trip", "checks"]);
        assert_eq!(step.params, [("r1.r".to_string(), 2000.0), ("c1.c".to_string(), 1.0)], "an integer is a number");
        assert_eq!(step.backend.as_deref(), Some("rust"));
        assert_eq!(step.csv.as_deref(), Some(Path::new("out/step.csv")));
        assert_eq!(step.records.as_deref(), Some(Path::new("out/step.jsonl")));
        assert_eq!(step.plot.as_deref(), Some(Path::new("out/step.svg")));
        assert!(step.all_checks && !runs[1].all_checks, "`all_checks` is off unless a run turns it on");
        assert_eq!(runs[1].analysis, Analysis::Dc { sweep: "v1.dc".into(), from: -1.0, to: 0.8, step: 0.05 });
        assert_eq!(runs[2].analysis, Analysis::Ac { from: 1e3, to: 1e7, grid: Grid::PerDecade(10) });
        assert_eq!(runs[3].analysis, Analysis::Ac { from: 1e3, to: 5e3, grid: Grid::Points(5) });
        // the nineteen keys of CLI-03 AC 1, `all_checks` (CLI-06 AC 7) and the nine session options
        // of HII-01 and `max_steps` (HII-01 amendment), no more
        assert_eq!(RUN_KEYS.len(), 36);
        // a manifest with no run table has no run
        assert_eq!(RequestFixture::read("").map(|r| r.len()).ok(), Some(0));
    }

    #[test]
    fn an_unknown_key_or_a_wrong_type_is_p0010_at_its_span() {
        // CLI-03 AC 4: the span covers the key
        let f = RequestFixture::fault("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nstopp = 1\n");
        let (code, message, at) = RequestFixture::located(&f);
        assert_eq!((code, at), ("P0010", "stopp"));
        assert!(message.contains("`stopp`"), "{message}");
        let f = RequestFixture::fault("[run.a]\nanalysis = \"transient\"\nstop = \"x\"\n");
        let (code, message, at) = RequestFixture::located(&f);
        assert_eq!(code, "P0010");
        assert!(at.starts_with("stop") && at.contains("\"x\""), "the span covers `stop` and its value: {at:?}");
        assert!(message.contains("`stop`") && message.contains("number"), "{message}");
        // a list that holds a non-string is the wrong type too
        let f = RequestFixture::fault("[run.a]\nanalysis = \"op\"\nprint = [1]\n");
        let (_, _, at) = RequestFixture::located(&f);
        assert!(at.starts_with("print"), "{at:?}");
        // every fault of a manifest is returned, in source order
        let faults = RequestFixture::read("[run.a]\nanalysis = \"op\"\nfoo = 1\nbar = 2\n").expect_err("two unknown keys");
        let spans: Vec<&str> = faults.iter().map(|f| RequestFixture::located(f).2).collect();
        assert_eq!(spans, ["foo", "bar"]);
    }

    #[test]
    fn analysis_is_one_of_four() {
        // CLI-03 AC 5: absent, the fault is at the run's name; unknown, at the value
        for (runs, at) in [("[run.a]\nstop = 1e-3\n", "a"), ("[run.a]\nanalysis = \"tran\"\nstop = 1e-3\n", "\"tran\"")] {
            let f = RequestFixture::fault(runs);
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", at), "{runs}");
            for name in ["`op`", "`dc`", "`ac`", "`transient`"] {
                assert!(message.contains(name), "{message} names {name}");
            }
        }
    }

    #[test]
    fn a_transient_needs_a_positive_stop() {
        // CLI-03 AC 6: each fault names its key
        let cases = [
            ("analysis = \"transient\"\n", "stop", "a"),
            ("analysis = \"transient\"\nstop = 0\n", "stop", "0"),
            ("analysis = \"transient\"\nstop = 1e-3\nmax_step = -1\n", "max_step", "-1"),
            ("analysis = \"transient\"\nstop = 1e-3\nprint_step = 0\n", "print_step", "0"),
        ];
        for (keys, key, at) in cases {
            let f = RequestFixture::fault(&format!("[run.a]\n{keys}"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", at), "{keys}");
            assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
        }
        // contract D10: `from` of a transient is the first time kept, `0 <= from < stop`
        let f = RequestFixture::fault("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nfrom = 1e-3\n");
        let (_, message, spanned) = RequestFixture::located(&f);
        assert_eq!(spanned, "1e-3");
        assert!(message.contains("`from`"), "{message}");
        assert!(RequestFixture::read("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nfrom = 0\n").is_ok());
    }

    #[test]
    fn a_key_outside_its_analysis_is_p0010() {
        // CLI-03 AC 7: the fault names the key and the analysis
        for (keys, key, analysis, at) in [
            ("analysis = \"op\"\nstop = 1e-3\n", "stop", "op", "1e-3"),
            ("analysis = \"transient\"\nstop = 1e-3\nsweep = \"v1.dc\"\n", "sweep", "transient", "\"v1.dc\""),
            ("analysis = \"op\"\nplot = \"a.svg\"\n", "plot", "op", "\"a.svg\""),
        ] {
            let f = RequestFixture::fault(&format!("[run.a]\n{keys}"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!(code, "P0010");
            assert!(message.contains(&format!("`{key}`")) && message.contains(&format!("`{analysis}`")), "{message}");
            assert_eq!(spanned, at, "the span is the key's value");
        }
    }

    #[test]
    fn a_run_name_is_an_identifier() {
        // spec edge 1: `[run."a b"]` is P0010 at the name
        let f = RequestFixture::fault("[run.\"a b\"]\nanalysis = \"op\"\n");
        let (code, message, spanned) = RequestFixture::located(&f);
        assert_eq!(code, "P0010");
        assert!(spanned.contains("a b"), "{spanned:?}");
        assert!(message.contains("`a b`"), "{message}");
        assert_eq!(RequestFixture::one("[run.a_1]\nanalysis = \"op\"\n").name, "a_1");
    }

    #[test]
    fn a_dc_sweep_needs_its_four_keys_and_a_step_toward_to() {
        // CLI-09 AC 3
        let whole = |without: &str| {
            ["sweep = \"v1.dc\"", "from = 0", "to = 1", "step = 0.25"].iter().filter(|k| !k.starts_with(without)).cloned().collect::<Vec<_>>().join("\n")
        };
        for key in ["sweep", "from", "to", "step"] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"dc\"\n{}\n", whole(key)));
            let (code, message, _) = RequestFixture::located(&f);
            assert_eq!(code, "P0010");
            assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
        }
        for (step, at) in [("0", "0"), ("-0.1", "-0.1")] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 0\nto = 1\nstep = {step}\n"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", at));
            assert!(message.contains("`step`"), "{message}");
        }
        // a step toward `to`, down
        let down = RequestFixture::one("[run.a]\nanalysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 1\nto = 0\nstep = -0.25\n");
        assert_eq!(down.analysis, Analysis::Dc { sweep: "v1.dc".into(), from: 1.0, to: 0.0, step: -0.25 });
    }

    #[test]
    fn an_ac_grid_is_exactly_one_of_two_and_its_range_is_ordered() {
        // CLI-09 AC 5: each fault names the keys
        let cases: [(&str, &[&str]); 4] = [
            ("from = 1e3\nto = 1e7\nper_decade = 10\npoints = 5", &["per_decade", "points"]),
            ("from = 1e3\nto = 1e7", &["per_decade", "points"]),
            ("from = 0\nto = 1e7\nper_decade = 10", &["from"]),
            ("from = 1e3\nto = 1e2\nper_decade = 10", &["to", "from"]),
        ];
        for (keys, named) in cases {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"ac\"\n{keys}\n"));
            let (code, message, _) = RequestFixture::located(&f);
            assert_eq!(code, "P0010", "{keys}");
            for key in named {
                assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
            }
        }
        // `to` equal to `from` is ordered
        assert!(RequestFixture::read("[run.a]\nanalysis = \"ac\"\nfrom = 1e3\nto = 1e3\npoints = 1\n").is_ok());
    }

    #[test]
    fn per_decade_gives_round_n_log10_plus_one_points() {
        // CLI-09 AC 4: 1e3 to 1e7 is 4 decades, so 10 per decade is 4 * 10 + 1 = 41 (the 41
        // rows of ac_rc's ngspice export); 1 to 1e3 at 3 per decade is 3 * 3 + 1 = 10
        assert_eq!(Grid::PerDecade(10).steps(1e3, 1e7), 41);
        assert_eq!(Grid::PerDecade(3).steps(1.0, 1e3), 10);
        // not a whole number of decades: 10 * log10(5) = 6.99, which rounds to 7, so 8 points
        assert_eq!(Grid::PerDecade(10).steps(1.0, 5.0), 8);
        // `from` equal to `to` is one point
        assert_eq!(Grid::PerDecade(10).steps(1e3, 1e3), 1);
        // AC 6: `points = n` is n points in all
        assert_eq!(Grid::Points(5).steps(1e3, 5e3), 5);
    }

    #[test]
    fn a_flag_overrides_a_key() {
        // CLI-03 AC 3: the flag wins; a key no flag names keeps the table's value
        let table = RequestFixture::one("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nprint_step = 1e-4\nprint = [\"out\"]\nparams = { \"r1.r\" = 2000.0 }\n");
        let flags = Overrides { stop: Some(2e-3), print: vec!["vin".into()], params: vec![("r1.r".into(), 1000.0)], ..Overrides::default() };
        let merged = table.merged(&flags).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(merged.analysis, Analysis::Transient { stop: 2e-3, max_step: None, print_step: Some(1e-4), from: None });
        assert_eq!(merged.print, ["vin"], "a list flag replaces the key");
        // CLI-10 AC 1: the table's params first, then the flags'
        assert_eq!(merged.params, [("r1.r".to_string(), 2000.0), ("r1.r".to_string(), 1000.0)]);
        assert_eq!(merged.name, "a");
        // no flag: the table's request unchanged
        assert_eq!(table.merged(&Overrides::default()).ok().as_ref(), Some(&table));
        // the merged request is validated again: a flag that breaks a rule is a P0010 naming
        // the key, with no span since no manifest wrote it
        let bad = table.merged(&Overrides { stop: Some(0.0), ..Overrides::default() }).expect_err("stop 0");
        assert!(matches!(&bad, Failure::Host { code: "P0010", message } if message.contains("`stop`")), "{bad:?}");
        let other = table.merged(&Overrides { analysis: Some("op".into()), ..Overrides::default() }).expect_err("op reads no stop");
        assert!(other.to_string().contains("`stop`") && other.to_string().contains("`op`"), "{other}");
        // a request from flags alone, as a lone file's
        let lone = RunSpec::from_flags("divider", &Overrides { analysis: Some("op".into()), ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!((lone.name.as_str(), &lone.analysis), ("divider", &Analysis::Op));
        assert!(RunSpec::from_flags("divider", &Overrides::default()).is_err(), "no analysis");
    }

    /// The nine option keys of HII-01 AC 1, as a run table gives them.
    const ALL_OPTIONS: &str = "reltol = 1e-4\nabstol = 1e-13\nvntol = 2e-6\ntrtol = 5\ntemp = 75\nresolution = 1e-12\nmaxrounds = 30\nmaxmicro = 500\nmax_steps = 4000000\nnodeset = { \"Top.a\" = 1.5, \"Top.b\" = 0 }\n";

    #[test]
    fn option_keys_are_read_for_every_analysis() {
        // HII-01 AC 1: the nine keys are read by each of the eight analyses; a run that gives none
        // has no option (HII-02 AC 1)
        let analyses = [
            "analysis = \"op\"\n",
            "analysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 0\nto = 1\nstep = 0.5\n",
            "analysis = \"ac\"\nfrom = 1\nto = 10\npoints = 3\n",
            "analysis = \"transient\"\nstop = 1e-3\n",
            "analysis = \"tf\"\ninput = \"v1\"\noutput = \"out\"\n",
            "analysis = \"pz\"\ninput = \"v1\"\noutput = \"out\"\n",
            "analysis = \"sens\"\nwrt = [\"r1.r\"]\n",
            "analysis = \"pss\"\nperiod = 1e-3\n",
        ];
        for analysis in analyses {
            let spec = RequestFixture::one(&format!("[run.a]\n{analysis}{ALL_OPTIONS}"));
            assert_eq!(spec.options, RequestFixture::options(), "{analysis}");
            assert_eq!(RequestFixture::one(&format!("[run.a]\n{analysis}")).options, Options::default(), "{analysis}");
        }
        assert_eq!(RUN_KEYS[20..30], ["reltol", "abstol", "vntol", "trtol", "temp", "resolution", "maxrounds", "maxmicro", "nodeset", "max_steps"]);
    }

    #[test]
    fn tolerance_keys_must_be_positive() {
        // HII-01 AC 3: not a number greater than 0 is a P0010 at the value, naming the key
        for key in ["reltol", "abstol", "vntol", "trtol", "resolution"] {
            for value in ["0", "-1e-3"] {
                let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"op\"\n{key} = {value}\n"));
                let (code, message, at) = RequestFixture::located(&f);
                assert_eq!((code, at), ("P0010", value), "{key} = {value}");
                assert!(message.contains(&format!("`{key}`")) && message.contains("greater than 0"), "{message}");
            }
            RequestFixture::one(&format!("[run.a]\nanalysis = \"op\"\n{key} = 1e-9\n"));
        }
    }

    #[test]
    fn temp_must_be_above_absolute_zero() {
        // HII-01 AC 4: -273.15 itself is refused, a value above it (negative included) is read
        for value in ["-273.15", "-300"] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"op\"\ntemp = {value}\n"));
            let (code, message, at) = RequestFixture::located(&f);
            assert_eq!((code, at), ("P0010", value));
            assert!(message.contains("`temp`") && message.contains("-273.15"), "{message}");
        }
        assert_eq!(RequestFixture::one("[run.a]\nanalysis = \"op\"\ntemp = -273.0\n").options.temp, Some(-273.0));
        assert_eq!(RequestFixture::one("[run.a]\nanalysis = \"op\"\ntemp = 0\n").options.temp, Some(0.0), "an integer is a number");
    }

    #[test]
    fn bounds_must_be_whole_numbers_of_at_least_one() {
        // HII-01 AC 5: 0, a fraction and a negative number are each a P0010 naming the key
        for key in ["maxrounds", "maxmicro", "max_steps"] {
            for value in ["0", "1.5", "-2"] {
                let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"op\"\n{key} = {value}\n"));
                let (code, message, _) = RequestFixture::located(&f);
                assert_eq!(code, "P0010", "{key} = {value}");
                assert!(message.contains(&format!("`{key}`")), "{message}");
            }
        }
        let spec = RequestFixture::one("[run.a]\nanalysis = \"op\"\nmaxrounds = 1\nmaxmicro = 1\n");
        assert_eq!((spec.options.maxrounds, spec.options.maxmicro), (Some(1), Some(1)));
    }

    #[test]
    fn a_flag_wins_over_its_option_key() {
        // HII-01 AC 2: a flag replaces the key it shares; a nodeset flag replaces the entry of its
        // path and leaves the others; a bad flag value is a P0010 with no span
        let table = RequestFixture::one(&format!("[run.a]\nanalysis = \"op\"\n{ALL_OPTIONS}"));
        let flags = Overrides {
            reltol: Some(1e-6),
            temp: Some(-40.0),
            maxrounds: Some(3),
            nodeset: vec![("Top.a".into(), 9.0), ("Top.c".into(), 2.0)],
            ..Overrides::default()
        };
        let merged = table.merged(&flags).unwrap_or_else(|f| panic!("{f}"));
        let want = Options {
            reltol: Some(1e-6),
            temp: Some(-40.0),
            maxrounds: Some(3),
            nodeset: vec![("Top.a".into(), 9.0), ("Top.b".into(), 0.0), ("Top.c".into(), 2.0)],
            ..RequestFixture::options()
        };
        assert_eq!(merged.options, want);
        let alone = RunSpec::from_flags("a", &Overrides { analysis: Some("op".into()), vntol: Some(1e-7), ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(alone.options, Options { vntol: Some(1e-7), ..Options::default() });
        let bad = table.merged(&Overrides { maxmicro: Some(0), ..Overrides::default() }).expect_err("maxmicro 0");
        assert!(matches!(&bad, Failure::Host { code: "P0010", message } if message.contains("`maxmicro`")), "{bad:?}");
    }

    #[test]
    fn tf_and_pz_need_input_and_output() {
        // HII-04 AC 3 and HII-05 AC 4: a run lacking either key is a P0010 naming it
        for analysis in ["tf", "pz"] {
            let both = RequestFixture::one(&format!("[run.a]\nanalysis = \"{analysis}\"\ninput = \"v1\"\noutput = \"out\"\n"));
            let want = if analysis == "tf" { Analysis::Tf { input: "v1".into(), output: "out".into() } } else { Analysis::Pz { input: "v1".into(), output: "out".into() } };
            assert_eq!(both.analysis, want);
            for (keys, missing) in [("output = \"out\"\n", "input"), ("input = \"v1\"\n", "output"), ("", "input")] {
                let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"{analysis}\"\n{keys}"));
                let (code, message, spanned) = RequestFixture::located(&f);
                assert_eq!((code, spanned), ("P0010", "a"), "{analysis} {keys}");
                assert!(message.contains(&format!("`{missing}`")) && message.contains(&format!("`{analysis}`")), "{message}");
            }
        }
        // a flag gives the key as a table does
        let flags = Overrides { analysis: Some("tf".into()), input: Some("v1".into()), output: Some("out".into()), ..Overrides::default() };
        let spec = RunSpec::from_flags("d", &flags).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(spec.analysis, Analysis::Tf { input: "v1".into(), output: "out".into() });
        // `input` belongs to `tf` and `pz` alone
        let f = RequestFixture::fault("[run.a]\nanalysis = \"op\"\ninput = \"v1\"\n");
        let (_, message, spanned) = RequestFixture::located(&f);
        assert!(message.contains("`input`") && message.contains("`op`"), "{message}");
        assert_eq!(spanned, "\"v1\"");
    }

    #[test]
    fn pss_needs_a_positive_period_and_a_non_negative_tstab() {
        // HII-07 AC 5: a `pss` run lacking `period`, with a `period` not above 0 or a negative
        // `tstab` is a P0010 naming the key
        let ok = RequestFixture::one("[run.a]\nanalysis = \"pss\"\nperiod = 1e-3\ntstab = 5e-3\nprint_step = 1e-5\n");
        assert_eq!(ok.analysis, Analysis::Pss { period: 1e-3, tstab: 5e-3, max_step: None, print_step: Some(1e-5) });
        // `tstab` defaults to 0, and 0 itself is allowed
        for keys in ["", "tstab = 0\n"] {
            let spec = RequestFixture::one(&format!("[run.a]\nanalysis = \"pss\"\nperiod = 2e-3\n{keys}"));
            assert_eq!(spec.analysis, Analysis::Pss { period: 2e-3, tstab: 0.0, max_step: None, print_step: None }, "{keys:?}");
        }
        for (keys, key, at) in [("", "period", "a"), ("period = 0\n", "period", "0"), ("period = -1e-3\n", "period", "-1e-3"), ("period = 1e-3\ntstab = -1e-3\n", "tstab", "-1e-3")] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"pss\"\n{keys}"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", at), "{keys:?}");
            assert!(message.contains(&format!("`{key}`")), "{message}");
        }
        // a flag gives the keys as a table does, and wins over the table
        let table = RequestFixture::one("[run.a]\nanalysis = \"pss\"\nperiod = 1e-3\n");
        let merged = table.merged(&Overrides { period: Some(4e-3), tstab: Some(1e-3), ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(merged.analysis, Analysis::Pss { period: 4e-3, tstab: 1e-3, max_step: None, print_step: None });
        // `period` and `tstab` belong to `pss` alone, and `plot` is allowed on it
        let f = RequestFixture::fault("[run.a]\nanalysis = \"op\"\nperiod = 1e-3\n");
        assert!(RequestFixture::located(&f).1.contains("`period`"), "{f:?}");
        RequestFixture::one("[run.a]\nanalysis = \"pss\"\nperiod = 1e-3\nplot = \"p.svg\"\n");
    }

    #[test]
    fn sens_needs_a_non_empty_wrt() {
        // HII-06 AC 3: no `wrt`, or an empty one, is a P0010 naming the key
        let ok = RequestFixture::one("[run.a]\nanalysis = \"sens\"\nwrt = [\"r1.r\", \"r2.r\"]\n");
        assert_eq!(ok.analysis, Analysis::Sens { wrt: vec!["r1.r".into(), "r2.r".into()] });
        for (keys, at) in [("", "a"), ("wrt = []\n", "[]")] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"sens\"\n{keys}"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", at), "{keys:?}");
            assert!(message.contains("`wrt`"), "{message}");
        }
        // the flag list replaces the key, in the order given
        let table = RequestFixture::one("[run.a]\nanalysis = \"sens\"\nwrt = [\"r1.r\"]\n");
        let merged = table.merged(&Overrides { wrt: vec!["r2.r".into(), "v1.dc".into()], ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(merged.analysis, Analysis::Sens { wrt: vec!["r2.r".into(), "v1.dc".into()] });
        // `wrt` belongs to `sens` alone
        let f = RequestFixture::fault("[run.a]\nanalysis = \"tf\"\ninput = \"v1\"\noutput = \"out\"\nwrt = [\"r1.r\"]\n");
        assert!(RequestFixture::located(&f).1.contains("`wrt`"), "{f:?}");
    }

    #[test]
    fn the_eight_analysis_names() {
        // HII-08 AC 3: eight names, in the order listed, each written in an unknown-analysis fault
        assert_eq!(Analysis::NAMES, ["op", "dc", "ac", "transient", "tf", "pz", "sens", "pss", "noise"]);
        let f = RequestFixture::fault("[run.a]\nanalysis = \"tran\"\nstop = 1e-3\n");
        let (_, message, _) = RequestFixture::located(&f);
        for name in Analysis::NAMES {
            assert!(message.contains(&format!("`{name}`")), "{message} names {name}");
        }
        for (analysis, keys) in [("tf", "input = \"v1\"\noutput = \"o\"\n"), ("pz", "input = \"v1\"\noutput = \"o\"\n"), ("sens", "wrt = [\"r.r\"]\n"), ("pss", "period = 1e-3\n")] {
            assert_eq!(RequestFixture::one(&format!("[run.a]\nanalysis = \"{analysis}\"\n{keys}")).analysis.name(), analysis);
        }
    }

    #[test]
    fn plot_is_refused_for_tf_pz_and_sens() {
        // HII-08 AC 3: a `plot` key on a run of the three is a P0010 at the key's value, naming both
        for (analysis, keys) in [("tf", "input = \"v1\"\noutput = \"o\"\n"), ("pz", "input = \"v1\"\noutput = \"o\"\n"), ("sens", "wrt = [\"r.r\"]\n")] {
            let f = RequestFixture::fault(&format!("[run.a]\nanalysis = \"{analysis}\"\n{keys}plot = \"p.svg\"\n"));
            let (code, message, spanned) = RequestFixture::located(&f);
            assert_eq!((code, spanned), ("P0010", "\"p.svg\""), "{analysis}");
            assert!(message.contains("`plot`") && message.contains(&format!("`{analysis}`")), "{message}");
        }
    }
}
