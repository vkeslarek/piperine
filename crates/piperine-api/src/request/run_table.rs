//! The run table of a manifest: its `[run.<name>]` tables read, key by key, into the keys of a run.

use std::path::PathBuf;

use piperine_lang::msl::{Diagnostic, Span};
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use super::{Given, Keys, RUN_KEYS, RunSpec, Source};
use crate::failure::Failure;

pub(super) type Key<'i> = Spanned<DeString<'i>>;
pub(super) type Value<'i> = Spanned<DeValue<'i>>;

impl Source {
    /// A `P0010` at `at` in this manifest.
    pub(super) fn fault(&self, at: Option<Span>, message: String, label: &str) -> Failure {
        Source::fault_in(Some(self), at, message, label)
    }

    /// A `P0010` at `at` in `source`, or with no span when there is no manifest or no span.
    pub(super) fn fault_in(source: Option<&Source>, at: Option<Span>, message: String, label: &str) -> Failure {
        match (source, at) {
            (Some(s), Some(span)) => {
                Failure::Request { diagnostic: Diagnostic::new("P0010", message, span, label), file: s.file.clone(), text: s.text.clone() }
            }
            _ => Failure::Host { code: "P0010", message },
        }
    }

    /// One `[run.<name>]`: its keys typed, then the rules of its analysis.
    pub(super) fn run(&self, name: &Key<'_>, value: &Value<'_>) -> Result<RunSpec, Vec<Failure>> {
        let name_at = Keys::span(name.span());
        let text = name.get_ref().to_string();
        if !Keys::is_identifier(&text) {
            let message = format!("`{text}` is not a run name: a run is named by an identifier");
            return Err(vec![self.fault(Some(name_at), message, "not an identifier")]);
        }
        let Some(table) = value.get_ref().as_table() else {
            return Err(vec![self.fault(Some(Keys::span_of(value)), format!("run `{text}` must be a table"), "expected a table")]);
        };
        let mut keys = Keys { name_at: Some(name_at), ..Keys::default() };
        let mut faults = Vec::new();
        for (key, value) in Keys::entries(table) {
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
        let at = Some(Keys::span_of(value));
        let wrong = |wants: &str| {
            let message = format!("`{name}` must be {wants}, found {}", value.get_ref().type_str());
            self.fault(Some(whole), message, &format!("expected {wants}"))
        };
        let v = value.get_ref();
        let text = || v.as_str().map(|s| Given { value: s.to_string(), at }).ok_or_else(|| wrong("a string"));
        let path = || v.as_str().map(|s| Given { value: PathBuf::from(s), at }).ok_or_else(|| wrong("a string"));
        let flag = || v.as_bool().map(|b| Given { value: b, at }).ok_or_else(|| wrong("a boolean"));
        let real = || Keys::number(v).map(|n| Given { value: n, at }).ok_or_else(|| wrong("a number"));
        let count = || Keys::i64_of(v).and_then(|n| u32::try_from(n).ok()).map(|n| Given { value: n, at }).ok_or_else(|| wrong("a whole number"));
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
            "params" => keys.params.extend(self.numbers(v, "a table of param paths and numbers", &wrong)?),
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
            "input" => keys.input = Some(text()?),
            "output" => keys.output = Some(text()?),
            "wrt" => keys.wrt = Some(Given { value: strings()?, at }),
            "period" => keys.period = Some(real()?),
            "tstab" => keys.tstab = Some(real()?),
            "nodeset" => keys.nodeset.extend(self.numbers(v, "a table of node paths and numbers", &wrong)?),
            "simparam" => keys.simparam.extend(self.simparam_table(v, &wrong)?),
            _ => return Err(self.unknown_key(key)),
        }
        Ok(())
    }

    /// The fault of a key no run takes.
    fn unknown_key(&self, key: &Key<'_>) -> Failure {
        let message = format!("unknown key `{}` in a run: a run takes {}", key.get_ref(), RUN_KEYS.join(", "));
        self.fault(Some(Keys::span(key.span())), message, "not a key of a run")
    }

    /// The `path = number` entries of the table `v`; a fault named by `wants` when it is no such table.
    fn numbers(&self, v: &DeValue<'_>, wants: &str, wrong: &dyn Fn(&str) -> Failure) -> Result<Vec<(String, f64)>, Failure> {
        let table = v.as_table().ok_or_else(|| wrong(wants))?;
        Keys::entries(table).into_iter().map(|(path, n)| Keys::number(n.get_ref()).map(|n| (path.get_ref().to_string(), n)).ok_or_else(|| wrong(wants))).collect()
    }

    /// The `name = number` entries of the `simparam` table `v`; a fault at the first entry that is no number.
    fn simparam_table(&self, v: &DeValue<'_>, wrong: &dyn Fn(&str) -> Failure) -> Result<Vec<(String, f64)>, Failure> {
        let table = v.as_table().ok_or_else(|| wrong("a table of names and numbers"))?;
        let mut entries = Vec::new();
        for (entry, n) in Keys::entries(table) {
            let Some(number) = Keys::number(n.get_ref()) else {
                let message = format!("`simparam` entry `{}` must be a number, found {}", entry.get_ref(), n.get_ref().type_str());
                return Err(self.fault(Some(Keys::span(entry.span())), message, "not a number"));
            };
            entries.push((entry.get_ref().to_string(), number));
        }
        Ok(entries)
    }
}

impl Keys {
    /// The entries of a table in source order (the table itself keeps them sorted by key).
    pub(super) fn entries<'a, 'i>(table: &'a DeTable<'i>) -> Vec<(&'a Key<'i>, &'a Value<'i>)> {
        let mut all: Vec<_> = table.iter().collect();
        all.sort_by_key(|(k, _)| k.span().start);
        all
    }

    /// The span of a byte range.
    pub(super) fn span(range: std::ops::Range<usize>) -> Span {
        Span::new(range.start, range.end)
    }

    /// The span of a spanned value.
    pub(super) fn span_of(value: &Value<'_>) -> Span {
        Keys::span(value.span())
    }

    /// A TOML integer or float as `f64`.
    pub(super) fn number(v: &DeValue<'_>) -> Option<f64> {
        if let Some(f) = v.as_float() {
            return f.as_str().replace('_', "").parse::<f64>().ok();
        }
        Keys::i64_of(v).map(|n| n as f64)
    }

    /// A TOML integer, in any of its radixes.
    pub(super) fn i64_of(v: &DeValue<'_>) -> Option<i64> {
        let i = v.as_integer()?;
        let digits = i.as_str().replace('_', "");
        i64::from_str_radix(&digits, i.radix()).ok()
    }

    /// `[A-Za-z_][A-Za-z0-9_]*`.
    pub(super) fn is_identifier(text: &str) -> bool {
        let mut chars = text.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}
