//! `DigitalPrinter::reaction`: a reaction of a body's shape as one Rust `fn` over `React`
//! (design "Phase B: generated digital code"; spec RS-11, RS-12 AC 4, 5; abi.md §5, §6).
//!
//! The function binds the reads its statements use (a carrier's pin decoded into an
//! `rt::Flagged<T>`, a param, a probed node as a plain `f64`), collects every commit in an
//! `rt::Commits` and hands them to the lens only after the last statement has run. So a fault
//! halfway makes no commit, and the commits go out in the order their statements were
//! made.
//!
//! **Statement order.** The shape's statement `i` is the `i`-th commit of a depth-first walk of
//! the reaction (operands in order, a block's statements before its value, `then` before
//! `else`, `piperine-plan`'s `Commits`); the printer numbers each commit by the order it prints
//! them, which is the same walk, and refuses the reaction when the carriers it numbered are
//! not the plan's (a transpiler defect, never a commit through the wrong statement).
//!
//! **Unknown control.** An `if` on a condition that may be unknown is `c.decide("<origin>")?`:
//! a `Fault` naming the `if` when the condition is unknown (§3.3.3 is out of scope here, RS-11
//! AC 5), returned before anything is flushed. A commit's delay that is unknown commits
//! `Unknown` at the next microstep (`Commits::commit`, RS-11 AC 6).
//!
//! The reaction of a drive with an `after` (RS-12 AC 5) commits the undelayed value its `.now`
//! pin holds to the carrier after the delay.

use piperine_ir::ValueType;
use piperine_plan::{ReactionMode, ReactionPlan, Source, StatementPlan};

use crate::Error;
use crate::print::{DigitalPrinter, Text, ValuePrinter};

impl<'a> DigitalPrinter<'a> {
    /// The name of the Rust function that runs reaction `plan`: `reaction<k>`, or
    /// `<carrier>_drive_after` for the reaction of a delayed drive.
    pub fn reaction_name(&self, plan: &ReactionPlan) -> String {
        match plan.mode {
            ReactionMode::Reaction(k) => format!("reaction{k}"),
            ReactionMode::Delayed(k) => {
                let c = self.body.drives().get(k as usize).map_or(u32::MAX, |d| d.carrier);
                let name = self.body.carriers().get(c as usize).map_or_else(|| format!("drive{k}"), |x| Text::ident(&x.name));
                format!("{name}_drive_after")
            }
            ReactionMode::Arm(k) => format!("at{k}_arm"),
        }
    }

    /// Reaction `k` of the plan as one Rust `fn` over `React`.
    pub fn reaction(&self, k: usize) -> Result<String, Error> {
        let Some(plan) = self.plan.reactions().get(k) else {
            return Err(self.reaction_unsupported(&format!("reaction {k}")));
        };
        let (values, lines, origin) = match plan.mode {
            ReactionMode::Reaction(r) => {
                let Some(reaction) = self.body.reactions().get(r as usize) else {
                    return Err(self.reaction_unsupported(&format!("reaction {r}")));
                };
                let values = ValuePrinter::of_reaction(self.ir, self.body, r as usize).with_reads(self.reads());
                let mut lines = Vec::with_capacity(reaction.stmts.len());
                for s in &reaction.stmts {
                    lines.push(values.stmt(*s)?);
                }
                (values, lines, reaction.origin.clone())
            }
            ReactionMode::Delayed(d) => {
                let Some(drive) = self.body.drives().get(d as usize) else {
                    return Err(self.reaction_unsupported(&format!("drive {d}")));
                };
                let values = ValuePrinter::of_digital(self.ir, self.body).with_reads(self.reads());
                let delay = match drive.after {
                    Some(after) => values.expr_as(after, &ValueType::Flagged(Box::new(ValueType::F64)))?,
                    None => return Err(self.reaction_unsupported(&format!("drive {d} without an `after`"))),
                };
                // the undelayed value is read from the `.now` pin and committed as it is
                let now = self.names.read_name(Source::Delayed(d));
                (values, vec![format!("commits_.commit(0, {now}, {delay});")], drive.origin.clone())
            }
            ReactionMode::Arm(e) => return Err(self.reaction_unsupported(&format!("the arming reaction of at(t) event {e}"))),
        };
        // the statement number each commit prints with is its place in the plan, so the plan and
        // the printer must agree on the carrier and the `LATER` bound of every statement
        let committed: Vec<StatementPlan> = match plan.mode {
            ReactionMode::Reaction(_) => values.committed(),
            ReactionMode::Delayed(d) => {
                let drive = self.body.drives().get(d as usize);
                vec![StatementPlan::commit(self.ir, drive.map_or(u32::MAX, |x| x.carrier), drive.and_then(|x| x.after))]
            }
            ReactionMode::Arm(e) => vec![StatementPlan::arm(self.body, e)],
        };
        if committed != plan.statements {
            return Err(self.reaction_unsupported(&format!(
                "reaction {k} whose commits {committed:?} are not the plan's statements {:?} (a transpiler defect)",
                plan.statements
            )));
        }
        let forced: Vec<Source> = match plan.mode {
            ReactionMode::Delayed(d) => vec![Source::Delayed(d)],
            ReactionMode::Reaction(_) | ReactionMode::Arm(_) => Vec::new(),
        };
        let binds = self.binds(&values, "cx_", &plan.reads, &forced)?;

        let shape = self.plan.shape().shape();
        let reaction = shape.reactions.get(k);
        let name = reaction.map_or("", |r| r.name.as_str());
        let on: Vec<String> = plan
            .on
            .iter()
            .map(|p| shape.pins.get(*p as usize).map_or_else(|| format!("pin {p}"), |x| x.name.as_str().to_string()))
            .collect();
        let reads: Vec<String> = plan.reads.iter().map(|s| self.names.read_name(*s)).collect();
        let writes: Vec<String> = committed.iter().map(|s| self.names.write_name(s.target)).collect();
        let mut t = Text::new();
        t.line(&format!(
            "// reaction {k} `{name}`: on {}; reads {}; commits {} @ {}",
            on.join(", "),
            reads.join(", "),
            writes.join(", "),
            reaction.map_or("", |r| r.origin.as_str())
        ));
        t.line(&format!("fn {}(cx_: &mut React<'_>) -> Result<Flow, Fault> {{", self.reaction_name(plan)));
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            t.comment(&format!("{} of body {}: the commits are made here and flushed after the last statement", name, self.body.name()), &origin);
            t.line(&format!("let {}commits_ = rt::Commits::new();", if committed.is_empty() { "" } else { "mut " }));
            for line in &lines {
                t.line(line);
            }
            t.line("commits_.flush(cx_)?;");
            t.line("Ok(Flow::Continue)");
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    fn reaction_unsupported(&self, form: &str) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {}", self.body.name()), origin: self.body.origin().clone() }
    }
}
