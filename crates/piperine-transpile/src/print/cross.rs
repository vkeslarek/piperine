//! `DigitalPrinter::cross`: the function of a `cross` event's `CROSS` watch (design "From an IR
//! function to a Rust function", `cross:<event>`; spec RS-12 AC 6; abi.md §11.5).
//!
//! The function is two Rust functions, as `terms` is. `cross<e>_block<R: Real>` is the crossed
//! expression over the reads it uses, in the plan's order; `cross<e>` binds those reads, runs
//! the block over `Dual<N>` at the iterate (`N` the function's differentiable reads, the
//! continuous pins), writes the watch's value, and writes each partial the plan declares from
//! the dual's tangent. A param is a constant of the dual and a carrier read is passed as it is
//! (an `rt::Flagged<T>`, not differentiable, like every read of `terms` that is no pin).

use piperine_ir::EventSource;
use piperine_plan::Mode;

use crate::Error;
use crate::print::{DigitalPrinter, Text, ValuePrinter};

impl<'a> DigitalPrinter<'a> {
    /// Function `k` of the plan, which runs `Mode::Cross`: the watch's value and its partials.
    pub fn cross(&self, k: usize) -> Result<String, Error> {
        let unsupported = |form: String| Error::Unsupported {
            form,
            path: format!("body {}", self.body.name()),
            origin: self.body.origin().clone(),
        };
        let Some(f) = self.plan.functions().get(k) else {
            return Err(unsupported(format!("function {k} as a cross watch")));
        };
        let Mode::Cross(e) = f.mode else {
            return Err(unsupported(format!("function {k} as a cross watch")));
        };
        let Some(event) = self.body.events().get(e as usize) else {
            return Err(unsupported(format!("event {e}")));
        };
        let EventSource::Cross { e: crossed, .. } = event.source else {
            return Err(unsupported(format!("event {e} as a cross watch")));
        };
        // the expression is code over reals: generic, like an `analog` block
        let values = ValuePrinter::of_body(self.ir, self.body).with_reads(self.reads());
        let expr = values.expr(crossed)?;
        let reads: Vec<(usize, _)> =
            f.reads.iter().copied().enumerate().filter(|(_, s)| values.uses(&self.names.read_name(*s))).collect();

        let mut params = Vec::with_capacity(reads.len());
        let mut binds = Vec::with_capacity(reads.len());
        for (i, s) in &reads {
            params.push(format!("{}: {}", self.names.read_name(*s), self.names.block_type(*s)?));
            binds.push(self.names.decode("cx_", *i, *s)?);
        }
        let names: Vec<String> = f.reads.iter().map(|s| self.names.read_name(*s)).collect();
        let write = self.names.write_name(f.writes.first().copied().unwrap_or(piperine_plan::Target::Watch(e)));
        let name = self.function_name(f);
        let args: Vec<String> = reads
            .iter()
            .map(|(i, s)| {
                if *i < f.slots {
                    format!("Dual::<{}>::seeded({}, {i})", f.slots, names[*i])
                } else if self.names.is_plain(*s) {
                    names[*i].clone()
                } else {
                    format!("Dual::<{}>::constant({})", f.slots, names[*i])
                }
            })
            .collect();

        let mut t = Text::new();
        t.comment(&format!("{}.cross {e}: the value the watch crosses zero on", self.body.name()), &event.origin);
        t.line(&format!("fn {name}_block<R: Real>({}) -> R {{", params.join(", ")));
        t.indented(|t| t.line(&expr));
        t.line("}");
        t.line("");
        self.header(&mut t, k, f);
        t.line(&format!("fn {name}(cx_: &mut Eval<'_>) -> Result<(), Fault> {{"));
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            t.line("// the evaluation at the iterate");
            t.line(&format!("let at_iterate_ = Self::{name}_block({});", args.join(", ")));
            t.line(&format!("// the watch `{write}`: its value"));
            t.line("cx_.set(0, at_iterate_.v);");
            for (p, (_, slot)) in f.partials.iter().enumerate() {
                t.line(&format!("// partial {p}: d {write} / d {}", names[*slot]));
                t.line(&format!("if cx_.wanted({p}) {{"));
                t.indented(|t| t.line(&format!("cx_.jac()[{p}] = at_iterate_.d[{slot}];")));
                t.line("}");
            }
            t.line("Ok(())");
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use piperine_plan::{BodyPlan, Mode};

    use super::*;
    use crate::alone::Alone;
    use crate::digital::Buck;
    use crate::print::AnalogPrinter;

    #[test]
    fn a_cross_function_prints_its_watch_and_partials() {
        let (ir, body) = Alone::design(Buck::comparator);
        let plan = BodyPlan::new(ir.body(body));
        let printer = DigitalPrinter::new(ir.body(body), &plan);
        let print = |name: &str| {
            let k = plan.shape().shape().functions.iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no `{name}`"));
            assert!(matches!(plan.functions()[k].mode, Mode::Cross { .. }));
            printer.cross(k).unwrap_or_else(|e| panic!("{e}"))
        };
        // convert.msl:11 `on cross(v(a) - vth - hyst / 2.0)`: abi.md §11.5's comparator function
        // writes the watch `W0` and one partial against each continuous read: here the one node
        // `a` (slot 0, a `Dual` seeded with its value), the params constants of the dual; the
        // watch takes the value at the iterate, the partial the tangent of slot 0
        let up = "\
// A2D.cross 0: the value the watch crosses zero on @ convert.msl:11:8
fn cross0_block<R: Real>(a: R, vth: R, hyst: R) -> R {
    a - vth - hyst / R::constant(2.0)
}

// function 0 `cross:0`: reads a, vth, hyst; writes cross:0 @ convert.msl:11:8
fn cross0(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let a = cx_.f64(0);
    let vth = cx_.f64(1);
    let hyst = cx_.f64(2);
    // the evaluation at the iterate
    let at_iterate_ = Self::cross0_block(Dual::<1>::seeded(a, 0), Dual::<1>::constant(vth), Dual::<1>::constant(hyst));
    // the watch `cross:0`: its value
    cx_.set(0, at_iterate_.v);
    // partial 0: d cross:0 / d a
    if cx_.wanted(0) {
        cx_.jac()[0] = at_iterate_.d[0];
    }
    Ok(())
}";
        assert_eq!(print("cross:0"), up);
        // convert.msl:12 `on cross(vth - hyst / 2.0 - v(a))`: the second watch, its own function
        // with its own value (`W1` of §11.5 is `-e`)
        let down = "\
// A2D.cross 1: the value the watch crosses zero on @ convert.msl:12:8
fn cross1_block<R: Real>(a: R, vth: R, hyst: R) -> R {
    vth - hyst / R::constant(2.0) - a
}

// function 1 `cross:1`: reads a, vth, hyst; writes cross:1 @ convert.msl:12:8
fn cross1(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let a = cx_.f64(0);
    let vth = cx_.f64(1);
    let hyst = cx_.f64(2);
    // the evaluation at the iterate
    let at_iterate_ = Self::cross1_block(Dual::<1>::seeded(a, 0), Dual::<1>::constant(vth), Dual::<1>::constant(hyst));
    // the watch `cross:1`: its value
    cx_.set(0, at_iterate_.v);
    // partial 0: d cross:1 / d a
    if cx_.wanted(0) {
        cx_.jac()[0] = at_iterate_.d[0];
    }
    Ok(())
}";
        assert_eq!(print("cross:1"), down);
    }

    #[test]
    fn terms_reads_a_carrier_as_a_plain_value() {
        // switch.msl:5 `i(p, n) +> v(p, n) * if g.high() { 1.0 / ron } else { 1.0 / roff }`: the
        // carrier `g` is decoded once from its pin, as the value that may be unknown it is, and
        // passed to `terms_block` as it is: no `Dual`, no partial (RS-12 AC 7); `g.high()` is
        // the known-and-true read of §3.3.4. Both partials are the node pins', and the term
        // stays affine in them (a carrier is constant in the node potentials)
        let (ir, body) = Alone::design(Buck::switch);
        let plan = BodyPlan::new(ir.body(body));
        let k = plan.functions().iter().position(|f| f.mode == Mode::Terms).expect("a terms function");
        let text = AnalogPrinter::new(ir.body(body), &plan).terms(k).unwrap_or_else(|e| panic!("{e}"));
        let expected = "\
// Switch.analog @ switch.msl:1:1
fn terms_block<R: Real>(p: R, n: R, ron: R, roff: R, g: rt::Flagged<bool>) -> [R; 1] {
    // contribution to i(p, n), branch b0 @ switch.msl:5:21
    let b0_flow_ = (p - n) * (if g.high() {
        R::constant(1.0) / ron
    } else {
        R::constant(1.0) / roff
    });
    [b0_flow_]
}

// function 0 `terms`: reads p, n, ron, roff, g; writes b0.flow @ switch.msl:1:1
fn terms(cx_: &mut Eval<'_>) -> Result<(), Fault> {
    let p = cx_.f64(0);
    let n = cx_.f64(1);
    let ron = cx_.f64(2);
    let roff = cx_.f64(3);
    let g = rt::Flagged::<bool>::decode(cx_.bytes(4));
    // the evaluation with every continuous pin read as 0 (the affine rule)
    let at_zero_ = Self::terms_block(Dual::<2>::seeded(0.0, 0), Dual::<2>::seeded(0.0, 1), Dual::<2>::constant(ron), Dual::<2>::constant(roff), g);
    // term 0 `b0.flow`: affine, c + Σ g·x from the evaluation at 0, folded from 0.0
    cx_.set(0, at_zero_[0].v + (0.0 + at_zero_[0].d[0] * p + at_zero_[0].d[1] * n));
    // partial 0: d b0.flow / d p
    if cx_.wanted(0) {
        cx_.jac()[0] = at_zero_[0].d[0];
    }
    // partial 1: d b0.flow / d n
    if cx_.wanted(1) {
        cx_.jac()[1] = at_zero_[0].d[1];
    }
    Ok(())
}";
        assert_eq!(text, expected);
        assert_eq!(plan.functions()[k].slots, 2, "only the two node pins are differentiable reads");
    }
}
