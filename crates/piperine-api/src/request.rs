//! `RunSpec`: one validated run request, whether a `[run.<name>]` table of `Piperine.toml` or
//! the flags of the command line spelled it (CLI-03, CLI-09 AC 3 and 5). Every fault is a
//! `P0010`: at the span of the key, the value or the run's name when the manifest gave it, with
//! no span when a flag did.

use std::path::{Path, PathBuf};

use piperine_lang::msl::{Diagnostic, Span};
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

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
}

impl Analysis {
    /// The four names a user writes, in the order they are listed.
    pub const NAMES: [&'static str; 4] = ["op", "dc", "ac", "transient"];

    /// The name a user writes.
    pub fn name(&self) -> &'static str {
        match self {
            Analysis::Op => "op",
            Analysis::Dc { .. } => "dc",
            Analysis::Ac { .. } => "ac",
            Analysis::Transient { .. } => "transient",
        }
    }
}

/// The frequency grid of an `ac` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grid {
    /// Logarithmic, `n` points per decade (ngspice `dec`).
    PerDecade(u32),
    /// Linear, `n` points in all.
    Points(u32),
}

impl Grid {
    /// The number of frequencies from `from` to `to`: `round(n * log10(to / from)) + 1` per
    /// decade (CLI-09 AC 4), `n` in all (AC 6).
    pub fn steps(&self, from: f64, to: f64) -> usize {
        match self {
            Grid::PerDecade(n) => (f64::from(*n) * (to / from).log10()).round() as usize + 1,
            Grid::Points(n) => *n as usize,
        }
    }
}

/// The session options of a run, read for every analysis (HII-01): ngspice's `.options` names,
/// lowercase. `None` leaves the solver's own default, so a run that gives none runs as it did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    /// The relative tolerance of a Newton iterate and of a step's error.
    pub reltol: Option<f64>,
    /// The absolute tolerance of a flow, amperes.
    pub abstol: Option<f64>,
    /// The absolute tolerance of a potential, volts.
    pub vntol: Option<f64>,
    /// The factor on the local truncation error a transient accepts.
    pub trtol: Option<f64>,
    /// The temperature of the run in degrees Celsius.
    pub temp: Option<f64>,
    /// The session's tick length in seconds.
    pub resolution: Option<f64>,
    /// The bound on the rounds of an operating point.
    pub maxrounds: Option<u32>,
    /// The bound on the microsteps of one instant.
    pub maxmicro: Option<u32>,
    /// The budget of attempts a transient may make before it fails; the solver's 2,000,000 when
    /// not given.
    pub max_steps: Option<u32>,
    /// The node values that seed the first Newton iterate of an operating point: a path and a
    /// value each, in the order given.
    pub nodeset: Vec<(String, f64)>,
}

/// The settings the command line gives, one per key of a run table; `None` (or empty) leaves
/// the table's value. `params` are added after the table's, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub bench: Option<PathBuf>,
    pub top: Option<String>,
    pub analysis: Option<String>,
    pub stop: Option<f64>,
    pub max_step: Option<f64>,
    pub print_step: Option<f64>,
    pub sweep: Option<String>,
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub step: Option<f64>,
    pub per_decade: Option<u32>,
    pub points: Option<u32>,
    pub print: Vec<String>,
    pub streams: Vec<String>,
    pub params: Vec<(String, f64)>,
    pub backend: Option<String>,
    pub csv: Option<PathBuf>,
    pub records: Option<PathBuf>,
    pub plot: Option<PathBuf>,
    /// `--all-checks`: print the passing `checks` records too.
    pub all_checks: Option<bool>,
    pub reltol: Option<f64>,
    pub abstol: Option<f64>,
    pub vntol: Option<f64>,
    pub trtol: Option<f64>,
    pub temp: Option<f64>,
    pub resolution: Option<f64>,
    pub maxrounds: Option<u32>,
    pub maxmicro: Option<u32>,
    pub max_steps: Option<u32>,
    /// `--nodeset <path>=<value>`: an entry replaces the table's entry of the same path.
    pub nodeset: Vec<(String, f64)>,
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
    nodeset: Vec<(String, f64)>,
}

/// The lowest temperature, degrees Celsius: `temp` must be above it.
const ABSOLUTE_ZERO: f64 = -273.15;

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

/// The keys of a run table, in the order CLI-03 AC 1 lists them, then the session options of
/// HII-01.
const RUN_KEYS: [&str; 30] = [
    "bench", "top", "analysis", "stop", "max_step", "print_step", "sweep", "from", "to", "step", "per_decade", "points", "print", "streams",
    "params", "backend", "csv", "records", "plot", "all_checks", "reltol", "abstol", "vntol", "trtol", "temp", "resolution", "maxrounds",
    "maxmicro", "nodeset", "max_steps",
];

/// The keys whose meaning belongs to an analysis; each analysis reads the ones `reads` lists.
const ANALYSIS_KEYS: [&str; 10] = ["stop", "max_step", "print_step", "sweep", "from", "to", "step", "per_decade", "points", "plot"];

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
            return Err(vec![source.fault(Some(span_of(run)), "`run` must be a table of runs, `[run.<name>]`".into(), "expected a table")]);
        };
        let (mut specs, mut faults) = (Vec::new(), Vec::new());
        for (name, value) in entries(runs) {
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

impl Source {
    fn fault(&self, at: Option<Span>, message: String, label: &str) -> Failure {
        Source::fault_in(Some(self), at, message, label)
    }

    fn fault_in(source: Option<&Source>, at: Option<Span>, message: String, label: &str) -> Failure {
        match (source, at) {
            (Some(s), Some(span)) => {
                Failure::Request { diagnostic: Diagnostic::new("P0010", message, span, label), file: s.file.clone(), text: s.text.clone() }
            }
            _ => Failure::Host { code: "P0010", message },
        }
    }

    /// One `[run.<name>]`: its keys typed, then the rules of its analysis.
    fn run(&self, name: &Key<'_>, value: &Value<'_>) -> Result<RunSpec, Vec<Failure>> {
        let name_at = span(name.span());
        let text = name.get_ref().to_string();
        if !is_identifier(&text) {
            let message = format!("`{text}` is not a run name: a run is named by an identifier");
            return Err(vec![self.fault(Some(name_at), message, "not an identifier")]);
        }
        let Some(table) = value.get_ref().as_table() else {
            return Err(vec![self.fault(Some(span_of(value)), format!("run `{text}` must be a table"), "expected a table")]);
        };
        let mut keys = Keys { name_at: Some(name_at), ..Keys::default() };
        let mut faults = Vec::new();
        for (key, value) in entries(table) {
            if let Err(f) = self.key(&mut keys, key, value) {
                faults.push(f);
            }
        }
        if !faults.is_empty() {
            return Err(faults);
        }
        keys.spec(text, Some(self.clone())).map_err(|f| vec![f])
    }

    /// Types one key of a run into `keys`.
    fn key(&self, keys: &mut Keys, key: &Key<'_>, value: &Value<'_>) -> Result<(), Failure> {
        let name = key.get_ref().as_ref();
        let whole = Span::new(key.span().start, value.span().end);
        let at = Some(span_of(value));
        let wrong = |wants: &str| {
            let message = format!("`{name}` must be {wants}, found {}", value.get_ref().type_str());
            self.fault(Some(whole), message, &format!("expected {wants}"))
        };
        let v = value.get_ref();
        let text = || v.as_str().map(|s| Given { value: s.to_string(), at }).ok_or_else(|| wrong("a string"));
        let path = || v.as_str().map(|s| Given { value: PathBuf::from(s), at }).ok_or_else(|| wrong("a string"));
        let flag = || v.as_bool().map(|b| Given { value: b, at }).ok_or_else(|| wrong("a boolean"));
        let real = || number(v).map(|n| Given { value: n, at }).ok_or_else(|| wrong("a number"));
        let count = || i64_of(v).and_then(|n| u32::try_from(n).ok()).map(|n| Given { value: n, at }).ok_or_else(|| wrong("a whole number"));
        let strings = || {
            let array = v.as_array().ok_or_else(|| wrong("an array of strings"))?;
            array.iter().map(|e| e.get_ref().as_str().map(str::to_string)).collect::<Option<Vec<_>>>().ok_or_else(|| wrong("an array of strings"))
        };
        match name {
            "bench" => keys.bench = Some(path()?),
            "top" => keys.top = Some(text()?),
            "analysis" => keys.analysis = Some(text()?),
            "stop" => keys.stop = Some(real()?),
            "max_step" => keys.max_step = Some(real()?),
            "print_step" => keys.print_step = Some(real()?),
            "sweep" => keys.sweep = Some(text()?),
            "from" => keys.from = Some(real()?),
            "to" => keys.to = Some(real()?),
            "step" => keys.step = Some(real()?),
            "per_decade" => keys.per_decade = Some(count()?),
            "points" => keys.points = Some(count()?),
            "print" => keys.print = strings()?,
            "streams" => keys.streams = strings()?,
            "params" => {
                let table = v.as_table().ok_or_else(|| wrong("a table of param paths and numbers"))?;
                for (path, n) in entries(table) {
                    let Some(n) = number(n.get_ref()) else { return Err(wrong("a table of param paths and numbers")) };
                    keys.params.push((path.get_ref().to_string(), n));
                }
            }
            "backend" => keys.backend = Some(text()?),
            "csv" => keys.csv = Some(path()?),
            "records" => keys.records = Some(path()?),
            "plot" => keys.plot = Some(path()?),
            "all_checks" => keys.all_checks = Some(flag()?),
            "reltol" => keys.reltol = Some(real()?),
            "abstol" => keys.abstol = Some(real()?),
            "vntol" => keys.vntol = Some(real()?),
            "trtol" => keys.trtol = Some(real()?),
            "temp" => keys.temp = Some(real()?),
            "resolution" => keys.resolution = Some(real()?),
            "maxrounds" => keys.maxrounds = Some(count()?),
            "maxmicro" => keys.maxmicro = Some(count()?),
            "max_steps" => keys.max_steps = Some(count()?),
            "nodeset" => {
                let table = v.as_table().ok_or_else(|| wrong("a table of node paths and numbers"))?;
                for (path, n) in entries(table) {
                    let Some(n) = number(n.get_ref()) else { return Err(wrong("a table of node paths and numbers")) };
                    keys.nodeset.push((path.get_ref().to_string(), n));
                }
            }
            _ => {
                let message = format!("unknown key `{name}` in a run: a run takes {}", RUN_KEYS.join(", "));
                return Err(self.fault(Some(span(key.span())), message, "not a key of a run"));
            }
        }
        Ok(())
    }
}

impl Keys {
    /// These keys with each flag of `o` over the key it shares; `params` appended.
    fn with(mut self, o: &Overrides) -> Keys {
        fn over<T: Clone>(key: &mut Option<Given<T>>, flag: &Option<T>) {
            if let Some(v) = flag {
                *key = Some(Given::flag(v.clone()));
            }
        }
        over(&mut self.bench, &o.bench);
        over(&mut self.top, &o.top);
        over(&mut self.analysis, &o.analysis);
        over(&mut self.stop, &o.stop);
        over(&mut self.max_step, &o.max_step);
        over(&mut self.print_step, &o.print_step);
        over(&mut self.sweep, &o.sweep);
        over(&mut self.from, &o.from);
        over(&mut self.to, &o.to);
        over(&mut self.step, &o.step);
        over(&mut self.per_decade, &o.per_decade);
        over(&mut self.points, &o.points);
        over(&mut self.backend, &o.backend);
        over(&mut self.csv, &o.csv);
        over(&mut self.records, &o.records);
        over(&mut self.plot, &o.plot);
        over(&mut self.all_checks, &o.all_checks);
        over(&mut self.reltol, &o.reltol);
        over(&mut self.abstol, &o.abstol);
        over(&mut self.vntol, &o.vntol);
        over(&mut self.trtol, &o.trtol);
        over(&mut self.temp, &o.temp);
        over(&mut self.resolution, &o.resolution);
        over(&mut self.maxrounds, &o.maxrounds);
        over(&mut self.maxmicro, &o.maxmicro);
        over(&mut self.max_steps, &o.max_steps);
        for (path, value) in &o.nodeset {
            match self.nodeset.iter_mut().find(|(p, _)| p == path) {
                Some(entry) => entry.1 = *value,
                None => self.nodeset.push((path.clone(), *value)),
            }
        }
        if !o.print.is_empty() {
            self.print = o.print.clone();
        }
        if !o.streams.is_empty() {
            self.streams = o.streams.clone();
        }
        self.params.extend(o.params.iter().cloned());
        self
    }

    /// The span of a key, if the manifest wrote it.
    fn at(&self, key: &str) -> Option<Span> {
        match key {
            "stop" => self.stop.as_ref().and_then(|g| g.at),
            "max_step" => self.max_step.as_ref().and_then(|g| g.at),
            "print_step" => self.print_step.as_ref().and_then(|g| g.at),
            "sweep" => self.sweep.as_ref().and_then(|g| g.at),
            "from" => self.from.as_ref().and_then(|g| g.at),
            "to" => self.to.as_ref().and_then(|g| g.at),
            "step" => self.step.as_ref().and_then(|g| g.at),
            "per_decade" => self.per_decade.as_ref().and_then(|g| g.at),
            "points" => self.points.as_ref().and_then(|g| g.at),
            "plot" => self.plot.as_ref().and_then(|g| g.at),
            _ => None,
        }
    }

    /// Whether the key is given.
    fn has(&self, key: &str) -> bool {
        match key {
            "stop" => self.stop.is_some(),
            "max_step" => self.max_step.is_some(),
            "print_step" => self.print_step.is_some(),
            "sweep" => self.sweep.is_some(),
            "from" => self.from.is_some(),
            "to" => self.to.is_some(),
            "step" => self.step.is_some(),
            "per_decade" => self.per_decade.is_some(),
            "points" => self.points.is_some(),
            "plot" => self.plot.is_some(),
            _ => false,
        }
    }

    /// The rules of CLI-03 and CLI-09 over these keys; the request when they hold.
    fn spec(self, name: String, source: Option<Source>) -> Result<RunSpec, Failure> {
        let fault = |at: Option<Span>, message: String, label: &str| Source::fault_in(source.as_ref(), at, message, label);
        let missing = |key: &str, analysis: &str| {
            fault(self.name_at, format!("run `{name}`: a `{analysis}` run needs `{key}`"), &format!("no `{key}`"))
        };
        let Some(analysis) = &self.analysis else {
            let message = format!("run `{name}` has no `analysis`: it is one of `op`, `dc`, `ac`, `transient`");
            return Err(fault(self.name_at, message, "no `analysis`"));
        };
        let reads: &[&str] = match analysis.value.as_str() {
            "op" => &[],
            "dc" => &["sweep", "from", "to", "step", "plot"],
            "ac" => &["from", "to", "per_decade", "points", "plot"],
            "transient" => &["stop", "max_step", "print_step", "from", "plot"],
            other => {
                let message = format!("unknown analysis `{other}`: the analyses are `op`, `dc`, `ac`, `transient`");
                return Err(fault(analysis.at, message, "not an analysis"));
            }
        };
        if let Some(key) = ANALYSIS_KEYS.iter().find(|k| self.has(k) && !reads.contains(k)) {
            let nothing = if *key == "plot" && analysis.value == "op" { ": an operating point has nothing to plot" } else { "" };
            let message = format!("run `{name}`: `{key}` is not read by a `{}` run{nothing}", analysis.value);
            return Err(fault(self.at(key), message, &format!("not a key of `{}`", analysis.value)));
        }
        let positive = |key: &str, g: &Option<Given<f64>>| match g {
            Some(Given { value, at }) if !(*value > 0.0) => {
                Err(fault(*at, format!("run `{name}`: `{key}` must be greater than 0, found {value}"), "not greater than 0"))
            }
            _ => Ok(()),
        };
        for (key, given) in [("reltol", &self.reltol), ("abstol", &self.abstol), ("vntol", &self.vntol), ("trtol", &self.trtol), ("resolution", &self.resolution)] {
            positive(key, given)?;
        }
        if let Some(Given { value, at }) = &self.temp {
            if !(*value > ABSOLUTE_ZERO) {
                let message = format!("run `{name}`: `temp` must be greater than {ABSOLUTE_ZERO} (degrees Celsius), found {value}");
                return Err(fault(*at, message, "not above absolute zero"));
            }
        }
        for (key, given) in [("maxrounds", &self.maxrounds), ("maxmicro", &self.maxmicro), ("max_steps", &self.max_steps)] {
            if let Some(Given { value: 0, at }) = given {
                return Err(fault(*at, format!("run `{name}`: `{key}` must be a whole number of at least 1, found 0"), "less than 1"));
            }
        }
        let parsed = match analysis.value.as_str() {
            "op" => Analysis::Op,
            "dc" => {
                let sweep = self.sweep.as_ref().ok_or_else(|| missing("sweep", "dc"))?;
                let from = self.from.as_ref().ok_or_else(|| missing("from", "dc"))?;
                let to = self.to.as_ref().ok_or_else(|| missing("to", "dc"))?;
                let step = self.step.as_ref().ok_or_else(|| missing("step", "dc"))?;
                if step.value == 0.0 || !step.value.is_finite() {
                    return Err(fault(step.at, format!("run `{name}`: `step` must not be 0"), "a step of 0"));
                }
                if (to.value - from.value) * step.value < 0.0 {
                    let message = format!("run `{name}`: `step` {} points away from `to` ({} to {})", step.value, from.value, to.value);
                    return Err(fault(step.at, message, "points away from `to`"));
                }
                Analysis::Dc { sweep: sweep.value.clone(), from: from.value, to: to.value, step: step.value }
            }
            "ac" => {
                let grid = match (&self.per_decade, &self.points) {
                    (Some(n), None) if n.value > 0 => Grid::PerDecade(n.value),
                    (None, Some(n)) if n.value > 0 => Grid::Points(n.value),
                    (Some(n), None) | (None, Some(n)) => {
                        return Err(fault(n.at, format!("run `{name}`: `per_decade` or `points` must be greater than 0"), "not greater than 0"));
                    }
                    (Some(_), Some(p)) => {
                        let message = format!("run `{name}`: an `ac` run gives exactly one of `per_decade` and `points`, not both");
                        return Err(fault(p.at.or(self.name_at), message, "both grids"));
                    }
                    (None, None) => {
                        let message = format!("run `{name}`: an `ac` run gives exactly one of `per_decade` and `points`");
                        return Err(fault(self.name_at, message, "no grid"));
                    }
                };
                let from = self.from.as_ref().ok_or_else(|| missing("from", "ac"))?;
                let to = self.to.as_ref().ok_or_else(|| missing("to", "ac"))?;
                if !(from.value > 0.0) {
                    return Err(fault(from.at, format!("run `{name}`: `from` must be greater than 0, found {}", from.value), "not greater than 0"));
                }
                if to.value < from.value {
                    let message = format!("run `{name}`: `to` ({}) is less than `from` ({})", to.value, from.value);
                    return Err(fault(to.at, message, "less than `from`"));
                }
                Analysis::Ac { from: from.value, to: to.value, grid }
            }
            _ => {
                let stop = self.stop.as_ref().ok_or_else(|| missing("stop", "transient"))?;
                positive("stop", &self.stop)?;
                positive("max_step", &self.max_step)?;
                positive("print_step", &self.print_step)?;
                if let Some(from) = &self.from {
                    if !(from.value >= 0.0 && from.value < stop.value) {
                        let message = format!("run `{name}`: `from` must be at least 0 and less than `stop` ({}), found {}", stop.value, from.value);
                        return Err(fault(from.at, message, "outside [0, stop)"));
                    }
                }
                Analysis::Transient {
                    stop: stop.value,
                    max_step: self.max_step.as_ref().map(|g| g.value),
                    print_step: self.print_step.as_ref().map(|g| g.value),
                    from: self.from.as_ref().map(|g| g.value),
                }
            }
        };
        Ok(RunSpec {
            name,
            bench: self.bench.as_ref().map(|g| g.value.clone()),
            top: self.top.as_ref().map(|g| g.value.clone()),
            analysis: parsed,
            print: self.print.clone(),
            streams: self.streams.clone(),
            params: self.params.clone(),
            backend: self.backend.as_ref().map(|g| g.value.clone()),
            csv: self.csv.as_ref().map(|g| g.value.clone()),
            records: self.records.as_ref().map(|g| g.value.clone()),
            plot: self.plot.as_ref().map(|g| g.value.clone()),
            all_checks: self.all_checks.as_ref().is_some_and(|g| g.value),
            options: Options {
                reltol: self.reltol.as_ref().map(|g| g.value),
                abstol: self.abstol.as_ref().map(|g| g.value),
                vntol: self.vntol.as_ref().map(|g| g.value),
                trtol: self.trtol.as_ref().map(|g| g.value),
                temp: self.temp.as_ref().map(|g| g.value),
                resolution: self.resolution.as_ref().map(|g| g.value),
                maxrounds: self.maxrounds.as_ref().map(|g| g.value),
                maxmicro: self.maxmicro.as_ref().map(|g| g.value),
                max_steps: self.max_steps.as_ref().map(|g| g.value),
                nodeset: self.nodeset.clone(),
            },
            keys: self,
            source,
        })
    }
}

/// The entries of a table in source order (the table itself keeps them sorted by key).
fn entries<'a, 'i>(table: &'a DeTable<'i>) -> Vec<(&'a Key<'i>, &'a Value<'i>)> {
    let mut all: Vec<_> = table.iter().collect();
    all.sort_by_key(|(k, _)| k.span().start);
    all
}

fn span(range: std::ops::Range<usize>) -> Span {
    Span::new(range.start, range.end)
}

fn span_of(value: &Value<'_>) -> Span {
    span(value.span())
}

/// A TOML integer or float as `f64`.
fn number(v: &DeValue<'_>) -> Option<f64> {
    if let Some(f) = v.as_float() {
        return f.as_str().replace('_', "").parse::<f64>().ok();
    }
    i64_of(v).map(|n| n as f64)
}

/// A TOML integer, in any of its radixes.
fn i64_of(v: &DeValue<'_>) -> Option<i64> {
    let i = v.as_integer()?;
    let digits = i.as_str().replace('_', "");
    i64::from_str_radix(&digits, i.radix()).ok()
}

/// `[A-Za-z_][A-Za-z0-9_]*`.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "Piperine.toml";

    /// The manifest text: a `[package]` and then `runs`.
    fn manifest(runs: &str) -> String {
        format!("[package]\nname = \"rc\"\nversion = \"0.1.0\"\n\n{runs}")
    }

    fn read(runs: &str) -> Result<Vec<RunSpec>, Vec<Failure>> {
        RunSpec::read(&manifest(runs), Path::new(FILE))
    }

    fn one(runs: &str) -> RunSpec {
        let mut all = read(runs).unwrap_or_else(|f| panic!("{f:?}"));
        assert_eq!(all.len(), 1, "one run");
        all.remove(0)
    }

    /// The single fault of a manifest that has exactly one.
    fn fault(runs: &str) -> Failure {
        let mut faults = read(runs).expect_err("the run is refused");
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

    #[test]
    fn a_run_table_reads_its_nineteen_keys() {
        // CLI-03 AC 1: each key read where the design's run table puts it. The analysis-bound
        // numbers are spread over three runs, since each analysis reads its own (CLI-03 AC 7).
        let runs = read(
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
        assert_eq!(RUN_KEYS.len(), 30);
        // a manifest with no run table has no run
        assert_eq!(read("").map(|r| r.len()).ok(), Some(0));
    }

    #[test]
    fn an_unknown_key_or_a_wrong_type_is_p0010_at_its_span() {
        // CLI-03 AC 4: the span covers the key
        let f = fault("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nstopp = 1\n");
        let (code, message, at) = located(&f);
        assert_eq!((code, at), ("P0010", "stopp"));
        assert!(message.contains("`stopp`"), "{message}");
        let f = fault("[run.a]\nanalysis = \"transient\"\nstop = \"x\"\n");
        let (code, message, at) = located(&f);
        assert_eq!(code, "P0010");
        assert!(at.starts_with("stop") && at.contains("\"x\""), "the span covers `stop` and its value: {at:?}");
        assert!(message.contains("`stop`") && message.contains("number"), "{message}");
        // a list that holds a non-string is the wrong type too
        let f = fault("[run.a]\nanalysis = \"op\"\nprint = [1]\n");
        let (_, _, at) = located(&f);
        assert!(at.starts_with("print"), "{at:?}");
        // every fault of a manifest is returned, in source order
        let faults = read("[run.a]\nanalysis = \"op\"\nfoo = 1\nbar = 2\n").expect_err("two unknown keys");
        let spans: Vec<&str> = faults.iter().map(|f| located(f).2).collect();
        assert_eq!(spans, ["foo", "bar"]);
    }

    #[test]
    fn analysis_is_one_of_four() {
        // CLI-03 AC 5: absent, the fault is at the run's name; unknown, at the value
        for (runs, at) in [("[run.a]\nstop = 1e-3\n", "a"), ("[run.a]\nanalysis = \"tran\"\nstop = 1e-3\n", "\"tran\"")] {
            let f = fault(runs);
            let (code, message, spanned) = located(&f);
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
            let f = fault(&format!("[run.a]\n{keys}"));
            let (code, message, spanned) = located(&f);
            assert_eq!((code, spanned), ("P0010", at), "{keys}");
            assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
        }
        // contract D10: `from` of a transient is the first time kept, `0 <= from < stop`
        let f = fault("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nfrom = 1e-3\n");
        let (_, message, spanned) = located(&f);
        assert_eq!(spanned, "1e-3");
        assert!(message.contains("`from`"), "{message}");
        assert!(read("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nfrom = 0\n").is_ok());
    }

    #[test]
    fn a_key_outside_its_analysis_is_p0010() {
        // CLI-03 AC 7: the fault names the key and the analysis
        for (keys, key, analysis, at) in [
            ("analysis = \"op\"\nstop = 1e-3\n", "stop", "op", "1e-3"),
            ("analysis = \"transient\"\nstop = 1e-3\nsweep = \"v1.dc\"\n", "sweep", "transient", "\"v1.dc\""),
            ("analysis = \"op\"\nplot = \"a.svg\"\n", "plot", "op", "\"a.svg\""),
        ] {
            let f = fault(&format!("[run.a]\n{keys}"));
            let (code, message, spanned) = located(&f);
            assert_eq!(code, "P0010");
            assert!(message.contains(&format!("`{key}`")) && message.contains(&format!("`{analysis}`")), "{message}");
            assert_eq!(spanned, at, "the span is the key's value");
        }
    }

    #[test]
    fn a_run_name_is_an_identifier() {
        // spec edge 1: `[run."a b"]` is P0010 at the name
        let f = fault("[run.\"a b\"]\nanalysis = \"op\"\n");
        let (code, message, spanned) = located(&f);
        assert_eq!(code, "P0010");
        assert!(spanned.contains("a b"), "{spanned:?}");
        assert!(message.contains("`a b`"), "{message}");
        assert_eq!(one("[run.a_1]\nanalysis = \"op\"\n").name, "a_1");
    }

    #[test]
    fn a_dc_sweep_needs_its_four_keys_and_a_step_toward_to() {
        // CLI-09 AC 3
        let whole = |without: &str| {
            ["sweep = \"v1.dc\"", "from = 0", "to = 1", "step = 0.25"].iter().filter(|k| !k.starts_with(without)).cloned().collect::<Vec<_>>().join("\n")
        };
        for key in ["sweep", "from", "to", "step"] {
            let f = fault(&format!("[run.a]\nanalysis = \"dc\"\n{}\n", whole(key)));
            let (code, message, _) = located(&f);
            assert_eq!(code, "P0010");
            assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
        }
        for (step, at) in [("0", "0"), ("-0.1", "-0.1")] {
            let f = fault(&format!("[run.a]\nanalysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 0\nto = 1\nstep = {step}\n"));
            let (code, message, spanned) = located(&f);
            assert_eq!((code, spanned), ("P0010", at));
            assert!(message.contains("`step`"), "{message}");
        }
        // a step toward `to`, down
        let down = one("[run.a]\nanalysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 1\nto = 0\nstep = -0.25\n");
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
            let f = fault(&format!("[run.a]\nanalysis = \"ac\"\n{keys}\n"));
            let (code, message, _) = located(&f);
            assert_eq!(code, "P0010", "{keys}");
            for key in named {
                assert!(message.contains(&format!("`{key}`")), "{message} names `{key}`");
            }
        }
        // `to` equal to `from` is ordered
        assert!(read("[run.a]\nanalysis = \"ac\"\nfrom = 1e3\nto = 1e3\npoints = 1\n").is_ok());
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
        let table = one("[run.a]\nanalysis = \"transient\"\nstop = 1e-3\nprint_step = 1e-4\nprint = [\"out\"]\nparams = { \"r1.r\" = 2000.0 }\n");
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
        }
    }

    #[test]
    fn option_keys_are_read_for_every_analysis() {
        // HII-01 AC 1: the nine keys are read by each of the four analyses; a run that gives none
        // has no option (HII-02 AC 1)
        for analysis in ["analysis = \"op\"\n", "analysis = \"dc\"\nsweep = \"v1.dc\"\nfrom = 0\nto = 1\nstep = 0.5\n", "analysis = \"ac\"\nfrom = 1\nto = 10\npoints = 3\n", "analysis = \"transient\"\nstop = 1e-3\n"] {
            let spec = one(&format!("[run.a]\n{analysis}{ALL_OPTIONS}"));
            assert_eq!(spec.options, options(), "{analysis}");
            assert_eq!(one(&format!("[run.a]\n{analysis}")).options, Options::default(), "{analysis}");
        }
        assert_eq!(RUN_KEYS[20..], ["reltol", "abstol", "vntol", "trtol", "temp", "resolution", "maxrounds", "maxmicro", "nodeset", "max_steps"]);
    }

    #[test]
    fn tolerance_keys_must_be_positive() {
        // HII-01 AC 3: not a number greater than 0 is a P0010 at the value, naming the key
        for key in ["reltol", "abstol", "vntol", "trtol", "resolution"] {
            for value in ["0", "-1e-3"] {
                let f = fault(&format!("[run.a]\nanalysis = \"op\"\n{key} = {value}\n"));
                let (code, message, at) = located(&f);
                assert_eq!((code, at), ("P0010", value), "{key} = {value}");
                assert!(message.contains(&format!("`{key}`")) && message.contains("greater than 0"), "{message}");
            }
            one(&format!("[run.a]\nanalysis = \"op\"\n{key} = 1e-9\n"));
        }
    }

    #[test]
    fn temp_must_be_above_absolute_zero() {
        // HII-01 AC 4: -273.15 itself is refused, a value above it (negative included) is read
        for value in ["-273.15", "-300"] {
            let f = fault(&format!("[run.a]\nanalysis = \"op\"\ntemp = {value}\n"));
            let (code, message, at) = located(&f);
            assert_eq!((code, at), ("P0010", value));
            assert!(message.contains("`temp`") && message.contains("-273.15"), "{message}");
        }
        assert_eq!(one("[run.a]\nanalysis = \"op\"\ntemp = -273.0\n").options.temp, Some(-273.0));
        assert_eq!(one("[run.a]\nanalysis = \"op\"\ntemp = 0\n").options.temp, Some(0.0), "an integer is a number");
    }

    #[test]
    fn bounds_must_be_whole_numbers_of_at_least_one() {
        // HII-01 AC 5: 0, a fraction and a negative number are each a P0010 naming the key
        for key in ["maxrounds", "maxmicro", "max_steps"] {
            for value in ["0", "1.5", "-2"] {
                let f = fault(&format!("[run.a]\nanalysis = \"op\"\n{key} = {value}\n"));
                let (code, message, _) = located(&f);
                assert_eq!(code, "P0010", "{key} = {value}");
                assert!(message.contains(&format!("`{key}`")), "{message}");
            }
        }
        let spec = one("[run.a]\nanalysis = \"op\"\nmaxrounds = 1\nmaxmicro = 1\n");
        assert_eq!((spec.options.maxrounds, spec.options.maxmicro), (Some(1), Some(1)));
    }

    #[test]
    fn a_flag_wins_over_its_option_key() {
        // HII-01 AC 2: a flag replaces the key it shares; a nodeset flag replaces the entry of its
        // path and leaves the others; a bad flag value is a P0010 with no span
        let table = one(&format!("[run.a]\nanalysis = \"op\"\n{ALL_OPTIONS}"));
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
            ..options()
        };
        assert_eq!(merged.options, want);
        let alone = RunSpec::from_flags("a", &Overrides { analysis: Some("op".into()), vntol: Some(1e-7), ..Overrides::default() }).unwrap_or_else(|f| panic!("{f}"));
        assert_eq!(alone.options, Options { vntol: Some(1e-7), ..Options::default() });
        let bad = table.merged(&Overrides { maxmicro: Some(0), ..Overrides::default() }).expect_err("maxmicro 0");
        assert!(matches!(&bad, Failure::Host { code: "P0010", message } if message.contains("`maxmicro`")), "{bad:?}");
    }
}
