//! `DelayPrinter`: the delay sites of a body (analog-operators, design mechanism 2).
//!
//! A site's input, delay time and bound are the arguments of a core module, evaluated at every
//! accepted solution whatever control flow surrounds the call (§14 rule 9). So `terms_block`
//! binds them first, as `delay<k>_input_`, `delay<k>_time_` and `delay<k>_max_`, before the
//! block and outside every condition. The site's two terms write the output pin `delay<k>.value`
//! (its own value `u`) and `-delay<k>_input_`; the `TIME` of the history term is
//! `min(td, max)` (§14.6.4, D5). A negative `td` is the fault 453 of `terms`; a bound that is not
//! positive (or, with no `max`, a negative `td`) is the fault 453 of the function `delay<k>.max`,
//! a block over the params that writes the bound pin (`td` itself when the site gives no `max`,
//! D4).

use std::collections::BTreeSet;

use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::{AnalogPrinter, Text};

/// The bindings of a body's delay sites: the lines, and the reads they use.
#[derive(Debug, Default)]
pub struct DelayStates {
    /// The `let`s of every site, in site order: input, time, then bound.
    pub lines: Vec<String>,
    /// The reads the bindings use.
    pub sources: BTreeSet<Source>,
}

/// Prints the delay sites of one body from its plan.
#[derive(Debug)]
pub struct DelayPrinter<'p, 'a> {
    printer: &'p AnalogPrinter<'a>,
}

impl<'p, 'a> DelayPrinter<'p, 'a> {
    /// The printer of `printer`'s body.
    pub fn new(printer: &'p AnalogPrinter<'a>) -> DelayPrinter<'p, 'a> {
        DelayPrinter { printer }
    }

    /// The variable that holds the input of site `k`: `delay0_input_`.
    pub fn input_name(k: u32) -> String {
        Text::generated(&format!("delay{k}_input"))
    }

    /// The variable that holds the delay time of site `k`: `delay0_time_`.
    pub fn time_name(k: u32) -> String {
        Text::generated(&format!("delay{k}_time"))
    }

    /// The variable that holds the bound of site `k`: `delay0_max_`.
    pub fn maximum_name(k: u32) -> String {
        Text::generated(&format!("delay{k}_max"))
    }

    /// The value of the `TIME` write of site `k`: the delay time held to the bound.
    pub fn time_value(k: u32) -> String {
        format!("{}.min({})", Self::time_name(k), Self::maximum_name(k))
    }

    /// The `let`s of each site's input, time and bound, in site order, and what they read. None
    /// reads a local: the elaborator inlines every block-local `let` they read.
    pub fn states(&self) -> Result<DelayStates, Error> {
        let body = self.printer.body;
        let mut states = DelayStates::default();
        for (k, site) in body.delays().iter().enumerate() {
            let k = k as u32;
            let mut locals = BTreeSet::new();
            let bound = site.maximum.unwrap_or(site.time);
            for expr in [site.input, site.time, bound] {
                self.printer.reads(expr, Mode::Terms, &mut locals, &mut states.sources);
            }
            if !locals.is_empty() {
                return Err(self.printer.unsupported(&format!("the arguments of delay site {k}, which read a local")));
            }
            for (name, expr) in [(Self::input_name(k), site.input), (Self::time_name(k), site.time), (Self::maximum_name(k), bound)] {
                let value = self.printer.values.expr(expr)?;
                states.lines.push(format!("let {name} = {value};"));
            }
        }
        Ok(states)
    }

    /// The fault 453 line of `terms` for the `TIME` write `w` of site `k`: the written time is
    /// `min(td, max)`, so it is negative exactly when `td` is (`max` is positive).
    pub fn time_fault(&self, w: usize, k: u32) -> String {
        format!("if at_iterate_[{w}].v < 0.0 {{ return Err(Fault::new(453, \"E0453: delay site {k} reads a negative td\")); }}")
    }

    /// `delay<k>_maximum_block` and `delay<k>_maximum`, for function `k` of the plan, which runs
    /// `Mode::DelayMaximum`: the site's bound, a block over the params run over `f64`, written to
    /// the bound pin, with the fault 453 for a bound that is not positive.
    pub fn maximum(&self, k: usize) -> Result<String, Error> {
        let printer = self.printer;
        let Some((f, site)) = printer.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::DelayMaximum(s) => Some((f, s)),
            _ => None,
        }) else {
            return Err(printer.unsupported(&format!("function {k} as a delay maximum function")));
        };
        let Some(delay) = printer.body.delays().get(site as usize) else {
            return Err(printer.unsupported(&format!("delay site {site}, which the body does not have")));
        };
        let explicit = delay.maximum.is_some();
        let value = printer.values.expr(delay.maximum.unwrap_or(delay.time))?;
        let name = format!("delay{site}_maximum");
        let reads: Vec<(usize, Source)> = f.reads.iter().copied().enumerate().collect();
        let mut t = Text::new();
        t.comment(&format!("{}.delay {site}: its maximum", printer.body.name()), &delay.origin);
        t.line(&format!("fn {name}_block<R: Real>({}) -> R {{", printer.block_params(&reads)?));
        t.indented(|t| t.line(&value));
        t.line("}");
        t.line("");
        let names: Vec<String> = f.reads.iter().map(|s| printer.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| printer.write_name(*w)).collect();
        t.line(&format!("// function {k} `delay{site}.max`: reads {}; writes {} @ {}", names.join(", "), writes.join(", "), printer.function_origin(k)));
        t.line(&format!("fn {name}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = printer.bind_reads(&reads)?;
        body.push(format!("let value_ = Self::{name}_block::<f64>({});", printer.block_args(&reads)));
        // an explicit bound must be positive (D5); without one the bound is `td`, which may be 0
        body.push(if explicit {
            format!("if !(value_ > 0.0) {{ return Err(Fault::new(453, \"E0453: delay site {site} has a max that is not positive\")); }}")
        } else {
            format!("if value_ < 0.0 {{ return Err(Fault::new(453, \"E0453: delay site {site} reads a negative td\")); }}")
        });
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::DelayMaximum(_) => body.push(format!("cx_.set({j}, value_);")),
                other => return Err(printer.unsupported(&format!("the write {other:?} of the maximum of delay site {site}"))),
            }
        }
        body.push("Ok(())".to_string());
        t.indented(|t| {
            for line in &body {
                t.line(line);
            }
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }
}
