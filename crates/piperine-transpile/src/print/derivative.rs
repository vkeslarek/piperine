//! `DerivativePrinter`: the derivative sites of a body (analog-structure, design mechanism 3).
//!
//! A site's state is the argument of a core module, evaluated at every accepted solution whatever
//! control flow surrounds the `ddt` (§14 rule 9). So `terms_block` binds the state of every site
//! first, as `d<k>_state_`, before the block and outside every condition. The site's two terms
//! write the pin `d<k>.rate` (its own value `u`, read as `d<k>_rate_`) and `-d<k>_state_`; every
//! `rate(D<k>)` in the code prints as the rate pin's variable, wherever it stands: `terms`, a
//! `cross` watch or a reaction. A site with an `abstol` has the function `d<k>.abstol`, a block
//! over the params that writes its pin.

use std::collections::BTreeSet;

use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::{AnalogPrinter, Text};

/// The state bindings of a body's derivative sites: the lines, and the reads they use.
#[derive(Debug, Default)]
pub struct States {
    /// One `let d<k>_state_ = …;` per site, in site order.
    pub lines: Vec<String>,
    /// The reads the states use.
    pub sources: BTreeSet<Source>,
}

/// Prints the derivative sites of one body from its plan.
#[derive(Debug)]
pub struct DerivativePrinter<'p, 'a> {
    printer: &'p AnalogPrinter<'a>,
}

impl<'p, 'a> DerivativePrinter<'p, 'a> {
    /// The printer of `printer`'s body.
    pub fn new(printer: &'p AnalogPrinter<'a>) -> DerivativePrinter<'p, 'a> {
        DerivativePrinter { printer }
    }

    /// The variable that holds the state of site `k`: `d0_state_`.
    pub fn state_name(k: u32) -> String {
        Text::generated(&format!("d{k}_state"))
    }

    /// The `let` of each site's state, in site order, and what they read. A state reads no local:
    /// the elaborator inlines every block-local `let` it reads (design mechanism 3).
    pub fn states(&self) -> Result<States, Error> {
        let body = self.printer.body;
        let mut states = States::default();
        for (k, site) in body.derivatives().iter().enumerate() {
            let mut locals = BTreeSet::new();
            self.printer.reads(site.state, Mode::Terms, &mut locals, &mut states.sources);
            if !locals.is_empty() {
                return Err(self.printer.unsupported(&format!("the state of derivative site {k}, which reads a local")));
            }
            let value = self.printer.values.expr(site.state)?;
            states.lines.push(format!("let {} = {value};", Self::state_name(k as u32)));
        }
        Ok(states)
    }

    /// `derivative<k>_abstol_block` and `derivative<k>_abstol`, for function `k` of the plan, which
    /// runs `Mode::DerivativeAbstol`: the site's `abstol`, a block generic over `Real` run over
    /// `f64`, written to the site's `abstol` pin.
    pub fn abstol(&self, k: usize) -> Result<String, Error> {
        let printer = self.printer;
        let Some((f, site)) = printer.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::DerivativeAbstol(s) => Some((f, s)),
            _ => None,
        }) else {
            return Err(printer.unsupported(&format!("function {k} as a derivative abstol function")));
        };
        let derivative = printer.body.derivatives().get(site as usize);
        let Some((derivative, value)) = derivative.and_then(|d| d.abstol.map(|a| (d, a))) else {
            return Err(printer.unsupported(&format!("derivative site {site}, which has no abstol")));
        };
        let name = format!("derivative{site}_abstol");
        let reads: Vec<(usize, Source)> = f.reads.iter().copied().enumerate().collect();
        let mut t = Text::new();
        t.comment(&format!("{}.derivative {site}: its abstol", printer.body.name()), &derivative.origin);
        t.line(&format!("fn {name}_block<R: Real>({}) -> R {{", printer.block_params(&reads)));
        let expression = printer.values.expr(value)?;
        t.indented(|t| t.line(&expression));
        t.line("}");
        t.line("");
        let names: Vec<String> = f.reads.iter().map(|s| printer.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| printer.write_name(*w)).collect();
        t.line(&format!(
            "// function {k} `d{site}.abstol`: reads {}; writes {} @ {}",
            names.join(", "),
            writes.join(", "),
            printer.function_origin(k)
        ));
        t.line(&format!("fn {name}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = printer.bind_reads(&reads);
        body.push(format!("let value_ = Self::{name}_block::<f64>({});", printer.block_args(&reads)));
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::DerivativeAbstol(_) => body.push(format!("cx_.set({j}, value_);")),
                other => return Err(printer.unsupported(&format!("the write {other:?} of the abstol of derivative site {site}"))),
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
