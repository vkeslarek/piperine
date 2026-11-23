//! `NoisePrinter`: the noise sources of a body (analog-operators, design mechanism 3).
//!
//! A source is the `NOISE` input `noise:<k>`. Every read of its result is a read of the input's
//! `VALUE` (`ValuePrinter` prints the variable `noise<k>_input_`; a reaction or a guard prints
//! `0.0`, the value at every accepted solution). The function `noise:<k>` writes the input's
//! `PSD` from the source's density, a block over the cone the density reads (it may read
//! `freq()`: the assembly runs it only in a noise sweep, so `k / freq().powf(e)` is never
//! evaluated at `freq() == 0`, §14.8.3). A density that is negative or not finite is the fault
//! 454 naming the source and the frequency it was read at.

use piperine_plan::{Mode, Source, Target};

use crate::Error;
use crate::print::{AnalogPrinter, Text};

/// Prints the noise sources of one body from its plan.
#[derive(Debug)]
pub struct NoisePrinter<'p, 'a> {
    printer: &'p AnalogPrinter<'a>,
}

impl<'p, 'a> NoisePrinter<'p, 'a> {
    /// The printer of `printer`'s body.
    pub fn new(printer: &'p AnalogPrinter<'a>) -> NoisePrinter<'p, 'a> {
        NoisePrinter { printer }
    }

    /// The variable that holds the input value of source `k`: `noise0_input_`.
    pub fn input_name(k: u32) -> String {
        Text::generated(&format!("noise{k}_input"))
    }

    /// `noise<k>_density_block` and `noise<k>`, for function `k` of the plan, which runs
    /// `Mode::Noise`: the source's density, a block over the params and the frequency run over
    /// `f64`, written to the input's `PSD`, with the fault 454 for a density that is negative or
    /// not finite.
    pub fn density(&self, k: usize) -> Result<String, Error> {
        let printer = self.printer;
        let Some((f, source)) = printer.plan.functions().get(k).and_then(|f| match f.mode {
            Mode::Noise(s) => Some((f, s)),
            _ => None,
        }) else {
            return Err(printer.unsupported(&format!("function {k} as a noise density function")));
        };
        let Some(noise) = printer.body.noise_sources().get(source as usize) else {
            return Err(printer.unsupported(&format!("noise source {source}, which the body does not have")));
        };
        let value = printer.values.expr(noise.density)?;
        let name = format!("noise{source}");
        let reads: Vec<(usize, Source)> = f.reads.iter().copied().enumerate().collect();
        let mut t = Text::new();
        t.comment(&format!("{}.noise {source}: its density", printer.body.name()), &noise.origin);
        t.line(&format!("fn {name}_density_block<R: Real>({}) -> R {{", printer.block_params(&reads)?));
        t.indented(|t| t.line(&value));
        t.line("}");
        t.line("");
        let names: Vec<String> = f.reads.iter().map(|s| printer.read_name(*s)).collect();
        let writes: Vec<String> = f.writes.iter().map(|w| printer.write_name(*w)).collect();
        t.line(&format!("// function {k} `noise:{source}`: reads {}; writes {} @ {}", names.join(", "), writes.join(", "), printer.function_origin(k)));
        t.line(&format!("fn {name}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{"));
        let mut body = printer.bind_reads(&reads)?;
        body.push(format!("let value_ = Self::{name}_density_block::<f64>({});", printer.block_args(&reads)));
        // the frequency the density was read at, when it reads one
        let at = if f.reads.contains(&Source::Freq) { format!(" at {{}} Hz\", value_, {}", printer.read_name(Source::Freq)) } else { "\", value_".to_string() };
        let label = if noise.name.is_empty() { source.to_string() } else { format!("{source} `{}`", noise.name) };
        body.push(format!("if !(value_ >= 0.0 && value_.is_finite()) {{ return Err(Fault::new(454, format!(\"E0454: noise source {label} has the density {{}}{at}))); }}"));
        for (j, w) in f.writes.iter().enumerate() {
            match w {
                Target::NoiseDensity(_) => body.push(format!("cx_.set({j}, value_);")),
                other => return Err(printer.unsupported(&format!("the write {other:?} of the density of noise source {source}"))),
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
