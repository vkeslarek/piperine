//! The rules of CLI-03 and CLI-09 over the keys of a run: which keys an analysis reads, the bounds
//! of the options, and one reader per analysis that types its keys into an [`Analysis`].

use piperine_lang::msl::Span;

use super::{ABSOLUTE_ZERO, ANALYSIS_KEYS, Analysis, Given, Grid, Keys, OPTION_NAMES, Options, RunSpec, Source};
use crate::failure::Failure;

/// The rules of one run: its keys, its name, and the manifest a fault points into.
struct Rules<'k> {
    keys: &'k Keys,
    name: &'k str,
    source: &'k Option<Source>,
}

impl Keys {
    /// The span of a key, if the manifest wrote it.
    pub(super) fn at(&self, key: &str) -> Option<Span> {
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
            "input" => self.input.as_ref().and_then(|g| g.at),
            "output" => self.output.as_ref().and_then(|g| g.at),
            "wrt" => self.wrt.as_ref().and_then(|g| g.at),
            "period" => self.period.as_ref().and_then(|g| g.at),
            "tstab" => self.tstab.as_ref().and_then(|g| g.at),
            _ => None,
        }
    }

    /// Whether the key is given.
    pub(super) fn has(&self, key: &str) -> bool {
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
            "input" => self.input.is_some(),
            "output" => self.output.is_some(),
            "wrt" => self.wrt.is_some(),
            "period" => self.period.is_some(),
            "tstab" => self.tstab.is_some(),
            _ => false,
        }
    }


    /// The rules of CLI-03 and CLI-09 over these keys; the request when they hold.
    pub(super) fn spec(self, name: String, source: Option<Source>) -> Result<RunSpec, Failure> {
        let rules = Rules { keys: &self, name: &name, source: &source };
        let analysis = rules.analysis()?;
        let reads = rules.reads(analysis)?;
        rules.refuse_foreign_keys(analysis, reads)?;
        rules.check_options()?;
        let parsed = match analysis.value.as_str() {
            "op" => Analysis::Op,
            "dc" => rules.sweep()?,
            "ac" | "noise" => rules.frequency(analysis)?,
            "tf" | "pz" => rules.transfer(analysis)?,
            "sens" => rules.sensitivity()?,
            "pss" => rules.periodic()?,
            _ => rules.transient()?,
        };
        let options = rules.options();
        Ok(RunSpec {
            name,
            bench: self.bench.as_ref().map(|g| g.value.clone()),
            top: self.top.as_ref().map(|g| g.value.clone()),
            analysis: parsed,
            print: self.print.clone(),
            streams: self.streams.clone(),
            params: self.params.clone(),
            param_texts: self.param_texts.clone(),
            backend: self.backend.as_ref().map(|g| g.value.clone()),
            csv: self.csv.as_ref().map(|g| g.value.clone()),
            records: self.records.as_ref().map(|g| g.value.clone()),
            plot: self.plot.as_ref().map(|g| g.value.clone()),
            all_checks: self.all_checks.as_ref().is_some_and(|g| g.value),
            options,
            keys: self,
            source,
        })
    }
}

impl Rules<'_> {
    /// A `P0010` at `at` in the manifest, or with no span when a flag gave the key.
    fn fault(&self, at: Option<Span>, message: String, label: &str) -> Failure {
        Source::fault_in(self.source.as_ref(), at, message, label)
    }

    /// The fault of a key the `analysis` run needs and does not have.
    fn missing(&self, key: &str, analysis: &str) -> Failure {
        self.fault(self.keys.name_at, format!("run `{}`: a `{analysis}` run needs `{key}`", self.name), &format!("no `{key}`"))
    }

    /// `key`, when given, must be greater than 0.
    fn positive(&self, key: &str, given: &Option<Given<f64>>) -> Result<(), Failure> {
        match given {
            Some(Given { value, at }) if !(*value > 0.0) => {
                Err(self.fault(*at, format!("run `{}`: `{key}` must be greater than 0, found {value}", self.name), "not greater than 0"))
            }
            _ => Ok(()),
        }
    }

    /// The `analysis` key of the run, which every run needs.
    fn analysis(&self) -> Result<&Given<String>, Failure> {
        match &self.keys.analysis {
            Some(analysis) => Ok(analysis),
            None => {
                let message = format!("run `{}` has no `analysis`: it is one of {}", self.name, Analysis::listed());
                Err(self.fault(self.keys.name_at, message, "no `analysis`"))
            }
        }
    }

    /// The keys whose meaning belongs to `analysis`; an analysis that is none is refused.
    fn reads(&self, analysis: &Given<String>) -> Result<&'static [&'static str], Failure> {
        Ok(match analysis.value.as_str() {
            "op" => &[],
            "dc" => &["sweep", "from", "to", "step", "plot"],
            "ac" => &["from", "to", "per_decade", "points", "plot"],
            "noise" => &["output", "input", "from", "to", "per_decade", "points"],
            "transient" => &["stop", "max_step", "print_step", "from", "plot"],
            "tf" | "pz" => &["input", "output"],
            "sens" => &["wrt"],
            "pss" => &["period", "tstab", "max_step", "print_step", "plot"],
            other => {
                let message = format!("unknown analysis `{other}`: the analyses are {}", Analysis::listed());
                return Err(self.fault(analysis.at, message, "not an analysis"));
            }
        })
    }

    /// A key of another analysis than `analysis` is refused.
    fn refuse_foreign_keys(&self, analysis: &Given<String>, reads: &[&str]) -> Result<(), Failure> {
        if let Some(key) = ANALYSIS_KEYS.iter().find(|k| self.keys.has(k) && !reads.contains(k)) {
            let nothing = if *key == "plot" && analysis.value == "op" { ": an operating point has nothing to plot" } else { "" };
            let message = format!("run `{}`: `{key}` is not read by a `{}` run{nothing}", self.name, analysis.value);
            return Err(self.fault(self.keys.at(key), message, &format!("not a key of `{}`", analysis.value)));
        }
        Ok(())
    }

    /// The bounds of the session options: tolerances above 0, a temperature above absolute zero,
    /// whole bounds of at least 1, and no `simparam` that shadows an option.
    fn check_options(&self) -> Result<(), Failure> {
        let keys = self.keys;
        for (key, given) in [("reltol", &keys.reltol), ("abstol", &keys.abstol), ("vntol", &keys.vntol), ("trtol", &keys.trtol), ("resolution", &keys.resolution)] {
            self.positive(key, given)?;
        }
        if let Some(Given { value, at }) = &keys.temp {
            if !(*value > ABSOLUTE_ZERO) {
                let message = format!("run `{}`: `temp` must be greater than {ABSOLUTE_ZERO} (degrees Celsius), found {value}", self.name);
                return Err(self.fault(*at, message, "not above absolute zero"));
            }
        }
        for (key, given) in [("maxrounds", &keys.maxrounds), ("maxmicro", &keys.maxmicro), ("max_steps", &keys.max_steps)] {
            if let Some(Given { value: 0, at }) = given {
                return Err(self.fault(*at, format!("run `{}`: `{key}` must be a whole number of at least 1, found 0", self.name), "less than 1"));
            }
        }
        if let Some((entry, _)) = keys.simparam.iter().find(|(entry, _)| OPTION_NAMES.contains(&entry.as_str())) {
            let message = format!("run `{}`: `simparam` may not set `{entry}`: it has its own key and flag", self.name);
            return Err(self.fault(keys.name_at, message, "an option's name"));
        }
        Ok(())
    }

    /// The session options the run gives.
    fn options(&self) -> Options {
        let keys = self.keys;
        Options {
            reltol: keys.reltol.as_ref().map(|g| g.value),
            abstol: keys.abstol.as_ref().map(|g| g.value),
            vntol: keys.vntol.as_ref().map(|g| g.value),
            trtol: keys.trtol.as_ref().map(|g| g.value),
            temp: keys.temp.as_ref().map(|g| g.value),
            resolution: keys.resolution.as_ref().map(|g| g.value),
            maxrounds: keys.maxrounds.as_ref().map(|g| g.value),
            maxmicro: keys.maxmicro.as_ref().map(|g| g.value),
            max_steps: keys.max_steps.as_ref().map(|g| g.value),
            nodeset: keys.nodeset.clone(),
            session_parameters: keys.simparam.clone(),
            time_point_rule: None,
        }
    }

    /// The `dc` sweep: its four keys, with a step that is not 0 and points toward `to`.
    fn sweep(&self) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let sweep = keys.sweep.as_ref().ok_or_else(|| self.missing("sweep", "dc"))?;
        let from = keys.from.as_ref().ok_or_else(|| self.missing("from", "dc"))?;
        let to = keys.to.as_ref().ok_or_else(|| self.missing("to", "dc"))?;
        let step = keys.step.as_ref().ok_or_else(|| self.missing("step", "dc"))?;
        if step.value == 0.0 || !step.value.is_finite() {
            return Err(self.fault(step.at, format!("run `{}`: `step` must not be 0", self.name), "a step of 0"));
        }
        if (to.value - from.value) * step.value < 0.0 {
            let message = format!("run `{}`: `step` {} points away from `to` ({} to {})", self.name, step.value, from.value, to.value);
            return Err(self.fault(step.at, message, "points away from `to`"));
        }
        Ok(Analysis::Dc { sweep: sweep.value.clone(), from: from.value, to: to.value, step: step.value })
    }

    /// The frequency grid of an `ac` or a `noise` run: exactly one of `per_decade` and `points`.
    fn grid(&self, analysis: &Given<String>) -> Result<Grid, Failure> {
        let keys = self.keys;
        // The article the analysis name takes in a message: "an `ac` run", "a `noise` run".
        let article = if analysis.value == "ac" { "an" } else { "a" };
        match (&keys.per_decade, &keys.points) {
            (Some(n), None) if n.value > 0 => Ok(Grid::PerDecade(n.value)),
            (None, Some(n)) if n.value > 0 => Ok(Grid::Points(n.value)),
            (Some(n), None) | (None, Some(n)) => {
                Err(self.fault(n.at, format!("run `{}`: `per_decade` or `points` must be greater than 0", self.name), "not greater than 0"))
            }
            (Some(_), Some(p)) => {
                let message = format!("run `{}`: {article} `{}` run gives exactly one of `per_decade` and `points`, not both", self.name, analysis.value);
                Err(self.fault(p.at.or(keys.name_at), message, "both grids"))
            }
            (None, None) => {
                let message = format!("run `{}`: {article} `{}` run gives exactly one of `per_decade` and `points`", self.name, analysis.value);
                Err(self.fault(keys.name_at, message, "no grid"))
            }
        }
    }

    /// An `ac` or a `noise` run: a grid, and a range from above 0 up.
    fn frequency(&self, analysis: &Given<String>) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let grid = self.grid(analysis)?;
        let from = keys.from.as_ref().ok_or_else(|| self.missing("from", &analysis.value))?;
        let to = keys.to.as_ref().ok_or_else(|| self.missing("to", &analysis.value))?;
        if !(from.value > 0.0) {
            return Err(self.fault(from.at, format!("run `{}`: `from` must be greater than 0, found {}", self.name, from.value), "not greater than 0"));
        }
        if to.value < from.value {
            let message = format!("run `{}`: `to` ({}) is less than `from` ({})", self.name, to.value, from.value);
            return Err(self.fault(to.at, message, "less than `from`"));
        }
        if analysis.value == "ac" {
            return Ok(Analysis::Ac { from: from.value, to: to.value, grid });
        }
        let output = keys.output.as_ref().ok_or_else(|| self.missing("output", "noise"))?;
        Ok(Analysis::Noise { output: output.value.clone(), input: keys.input.as_ref().map(|g| g.value.clone()), from: from.value, to: to.value, grid })
    }

    /// A `tf` or a `pz` run: the source and the node.
    fn transfer(&self, analysis: &Given<String>) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let input = keys.input.as_ref().ok_or_else(|| self.missing("input", &analysis.value))?;
        let output = keys.output.as_ref().ok_or_else(|| self.missing("output", &analysis.value))?;
        let (input, output) = (input.value.clone(), output.value.clone());
        Ok(if analysis.value == "tf" { Analysis::Tf { input, output } } else { Analysis::Pz { input, output } })
    }

    /// A `sens` run: at least one param to differentiate by.
    fn sensitivity(&self) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let wrt = keys.wrt.as_ref().ok_or_else(|| self.missing("wrt", "sens"))?;
        if wrt.value.is_empty() {
            return Err(self.fault(wrt.at.or(keys.name_at), format!("run `{}`: `wrt` must name at least one param", self.name), "empty"));
        }
        Ok(Analysis::Sens { wrt: wrt.value.clone() })
    }

    /// A `pss` run: a positive period and steps, and a `tstab` of at least 0.
    fn periodic(&self) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let period = keys.period.as_ref().ok_or_else(|| self.missing("period", "pss"))?;
        self.positive("period", &keys.period)?;
        self.positive("max_step", &keys.max_step)?;
        self.positive("print_step", &keys.print_step)?;
        if let Some(Given { value, at }) = &keys.tstab {
            if !(*value >= 0.0) {
                return Err(self.fault(*at, format!("run `{}`: `tstab` must be at least 0, found {value}", self.name), "negative"));
            }
        }
        Ok(Analysis::Pss {
            period: period.value,
            tstab: keys.tstab.as_ref().map_or(0.0, |g| g.value),
            max_step: keys.max_step.as_ref().map(|g| g.value),
            print_step: keys.print_step.as_ref().map(|g| g.value),
        })
    }

    /// A `transient` run: a positive `stop`, and a `from` inside `[0, stop)`.
    fn transient(&self) -> Result<Analysis, Failure> {
        let keys = self.keys;
        let stop = keys.stop.as_ref().ok_or_else(|| self.missing("stop", "transient"))?;
        self.positive("stop", &keys.stop)?;
        self.positive("max_step", &keys.max_step)?;
        self.positive("print_step", &keys.print_step)?;
        if let Some(from) = &keys.from {
            if !(from.value >= 0.0 && from.value < stop.value) {
                let message = format!("run `{}`: `from` must be at least 0 and less than `stop` ({}), found {}", self.name, stop.value, from.value);
                return Err(self.fault(from.at, message, "outside [0, stop)"));
            }
        }
        Ok(Analysis::Transient {
            stop: stop.value,
            max_step: keys.max_step.as_ref().map(|g| g.value),
            print_step: keys.print_step.as_ref().map(|g| g.value),
            from: keys.from.as_ref().map(|g| g.value),
        })
    }
}
