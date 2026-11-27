//! The settings the command line gives, and how they lie over the keys of a run table.

use std::path::PathBuf;

use super::{Given, Keys};

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
    /// `--param` values given as text, read by the param's type (PI-01 to PI-03, D24): an integer
    /// literal in a radix, a variant name, `None`. Applied after `params`.
    pub param_texts: Vec<(String, String)>,
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
    /// `--input`: the voltage source a `tf` or `pz` run excites.
    pub input: Option<String>,
    /// `--output`: the node a `tf` or `pz` run measures.
    pub output: Option<String>,
    /// `--wrt`, repeatable: the params a `sens` run differentiates by; a flag list replaces the key.
    pub wrt: Vec<String>,
    /// `--period`: the period of a `pss` run.
    pub period: Option<f64>,
    /// `--tstab`: the length of the transient a `pss` run takes before its first shot.
    pub tstab: Option<f64>,
    /// `--nodeset <path>=<value>`: an entry replaces the table's entry of the same path.
    pub nodeset: Vec<(String, f64)>,
    /// `--simparam <name>=<value>`, repeatable: an entry replaces the table's entry of the same name.
    pub simparam: Vec<(String, f64)>,
}

impl<T: Clone> Given<T> {
    /// `key` replaced by the flag's value when the flag is given.
    fn over(key: &mut Option<Given<T>>, flag: &Option<T>) {
        if let Some(v) = flag {
            *key = Some(Given::flag(v.clone()));
        }
    }
}

impl Keys {
    /// These keys with each flag of `o` over the key it shares; `params` appended.
    pub(super) fn with(mut self, o: &Overrides) -> Keys {
        Given::over(&mut self.bench, &o.bench);
        Given::over(&mut self.top, &o.top);
        Given::over(&mut self.analysis, &o.analysis);
        Given::over(&mut self.stop, &o.stop);
        Given::over(&mut self.max_step, &o.max_step);
        Given::over(&mut self.print_step, &o.print_step);
        Given::over(&mut self.sweep, &o.sweep);
        Given::over(&mut self.from, &o.from);
        Given::over(&mut self.to, &o.to);
        Given::over(&mut self.step, &o.step);
        Given::over(&mut self.per_decade, &o.per_decade);
        Given::over(&mut self.points, &o.points);
        Given::over(&mut self.backend, &o.backend);
        Given::over(&mut self.csv, &o.csv);
        Given::over(&mut self.records, &o.records);
        Given::over(&mut self.plot, &o.plot);
        Given::over(&mut self.all_checks, &o.all_checks);
        Given::over(&mut self.reltol, &o.reltol);
        Given::over(&mut self.abstol, &o.abstol);
        Given::over(&mut self.vntol, &o.vntol);
        Given::over(&mut self.trtol, &o.trtol);
        Given::over(&mut self.temp, &o.temp);
        Given::over(&mut self.resolution, &o.resolution);
        Given::over(&mut self.maxrounds, &o.maxrounds);
        Given::over(&mut self.maxmicro, &o.maxmicro);
        Given::over(&mut self.max_steps, &o.max_steps);
        Given::over(&mut self.input, &o.input);
        Given::over(&mut self.output, &o.output);
        Given::over(&mut self.period, &o.period);
        Given::over(&mut self.tstab, &o.tstab);
        if !o.wrt.is_empty() {
            self.wrt = Some(Given::flag(o.wrt.clone()));
        }
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
        for (entry, value) in &o.simparam {
            match self.simparam.iter_mut().find(|(n, _)| n == entry) {
                Some(found) => found.1 = *value,
                None => self.simparam.push((entry.clone(), *value)),
            }
        }
        self.params.extend(o.params.iter().cloned());
        self.param_texts.extend(o.param_texts.iter().cloned());
        self
    }
}
