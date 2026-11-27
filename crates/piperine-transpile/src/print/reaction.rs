//! `DigitalPrinter::reaction`: a reaction of a body's shape as one Rust `fn` over `React`
//! (design "Phase B: generated digital code"; spec RS-11, RS-12 AC 4, 5; abi.md §5, §6).
//!
//! The function binds the reads its statements use (a carrier's pin decoded into its `T`, a
//! param, a probed node as a plain `f64`), collects every commit in an
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
//! A `finish` is told to `Commits::finish`, and the reaction returns `Commits::flow`.
//!
//! The reaction of a drive with an `after` (RS-12 AC 5) commits the undelayed value its `.now`
//! pin holds to the carrier after the delay.
//!
//! The arming reaction of an `at(t)` event (msl-digital A3) evaluates `t` and hands it to
//! `Commits::arm` with the instant's `TIME`: one `REPLACE` firing of the timer's pin, `t - now`
//! later, at the next microstep when `t` is now, never when `t` is past.

use piperine_ir::{Env, EventSource};
use piperine_plan::{ReactionMode, ReactionPlan, Source, StatementPlan};

use crate::Error;
use crate::print::{DigitalPrinter, Text, ExpressionPrinter};

/// What printing the body of one reaction gives: the printer that numbered its reads and commits,
/// the statement lines, and where the reaction was written.
struct ReactionLines<'a> {
    /// The printer of the body's values, which holds the reads and commits the lines made.
    values: ExpressionPrinter<'a>,
    /// The statement lines, in order.
    lines: Vec<String>,
    /// The source position of the reaction, the drive or the event.
    origin: piperine_ir::Origin,
}

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
        let ReactionLines { values, lines, origin } = self.reaction_lines(plan)?;
        let committed = self.committed(plan, &values);
        self.check_statements(k, plan, &committed)?;
        let forced = Self::forced_sources(plan);
        let mut binds = self.binds(&values, "cx_", &plan.reads, &forced)?;
        // `freq()` read through a function: no `FREQ` cell in a reaction, the value is 0 (§14.4.4)
        let freq = ExpressionPrinter::env_name(Env::Freq);
        if values.uses(freq) {
            binds.push(format!("let {freq} = 0.0;"));
        }
        let name = self.plan.shape().shape().reactions.get(k).map_or("", |r| r.name.as_str());
        let mut t = Text::new();
        t.line(&self.reaction_header(k, plan, &committed));
        t.line(&format!("fn {}(cx_: &mut React<'_>) -> Result<Flow, Fault> {{", self.reaction_name(plan)));
        t.indented(|t| {
            for line in &binds {
                t.line(line);
            }
            t.comment(&format!("{} of body {}: the commits are made here and flushed after the last statement", name, self.body.name()), &origin);
            // every use of `Commits` but a bare `flush` takes `&mut self`
            let mutable = !committed.is_empty() || values.records().finishes();
            t.line(&format!("let {}commits_ = rt::Commits::new();", if mutable { "mut " } else { "" }));
            for line in &lines {
                t.line(line);
            }
            if values.records().finishes() {
                t.line("let flow_ = commits_.flow();");
            }
            t.line("commits_.flush(cx_)?;");
            t.line(if values.records().finishes() { "Ok(flow_)" } else { "Ok(Flow::Continue)" });
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    /// The printer, the statement lines and the origin of the body of reaction `plan`, by its
    /// mode: a reaction's statements, a delayed drive's one commit or an arming's one `arm`.
    fn reaction_lines(&self, plan: &ReactionPlan) -> Result<ReactionLines<'a>, Error> {
        match plan.mode {
            ReactionMode::Reaction(r) => self.body_lines(r),
            ReactionMode::Delayed(d) => self.delayed_lines(d),
            ReactionMode::Arm(e) => self.arm_lines(e),
        }
    }

    /// The lines of reaction `r` of the body: its statements, printed in order.
    fn body_lines(&self, r: u32) -> Result<ReactionLines<'a>, Error> {
        let Some(reaction) = self.body.reactions().get(r as usize) else {
            return Err(self.reaction_unsupported(&format!("reaction {r}")));
        };
        let values = ExpressionPrinter::of_reaction(self.ir, self.body, r as usize).with_reads(self.reads());
        let mut lines = Vec::with_capacity(reaction.stmts.len());
        for s in &reaction.stmts {
            lines.push(values.stmt(*s)?);
        }
        Ok(ReactionLines { values, lines, origin: reaction.origin.clone() })
    }

    /// The one commit of the reaction of drive `d`: the undelayed value its `.now` pin holds, after
    /// the drive's delay.
    fn delayed_lines(&self, d: u32) -> Result<ReactionLines<'a>, Error> {
        let Some(drive) = self.body.drives().get(d as usize) else {
            return Err(self.reaction_unsupported(&format!("drive {d}")));
        };
        let values = ExpressionPrinter::of_digital(self.ir, self.body).with_reads(self.reads());
        let delay = match drive.after {
            Some(after) => values.expr(after)?,
            None => return Err(self.reaction_unsupported(&format!("drive {d} without an `after`"))),
        };
        // the undelayed value is read from the `.now` pin and committed as it is
        let now = self.names.read_name(Source::Delayed(d));
        Ok(ReactionLines { values, lines: vec![format!("commits_.commit(0, {now}, {delay});")], origin: drive.origin.clone() })
    }

    /// The one `arm` of the reaction of the `at(t)` event `e`.
    fn arm_lines(&self, e: u32) -> Result<ReactionLines<'a>, Error> {
        let Some(event) = self.body.events().get(e as usize) else {
            return Err(self.reaction_unsupported(&format!("event {e}")));
        };
        let EventSource::At(time) = event.source else {
            return Err(self.reaction_unsupported(&format!("event {e} as a timer")));
        };
        let values = ExpressionPrinter::of_digital(self.ir, self.body).with_reads(self.reads());
        let at = values.expr(time)?;
        // the time is compared with the instant's own `TIME` (abi.md §4.2)
        let now = self.names.read_name(Source::Time);
        Ok(ReactionLines { values, lines: vec![format!("commits_.arm(0, {at}, {now});")], origin: event.origin.clone() })
    }

    /// The statements `plan`'s reaction commits through, as the printer numbered them.
    fn committed(&self, plan: &ReactionPlan, values: &ExpressionPrinter<'_>) -> Vec<StatementPlan> {
        match plan.mode {
            ReactionMode::Reaction(_) => values.records().committed(),
            ReactionMode::Delayed(d) => {
                let drive = self.body.drives().get(d as usize);
                vec![StatementPlan::commit(self.ir, drive.map_or(u32::MAX, |x| x.carrier), drive.and_then(|x| x.after))]
            }
            ReactionMode::Arm(e) => vec![StatementPlan::arm(self.body, e)],
        }
    }

    /// The statement number each commit prints with is its place in the plan, so the plan and the
    /// printer must agree on the carrier and the `LATER` bound of every statement: a difference is
    /// a transpiler defect.
    fn check_statements(&self, k: usize, plan: &ReactionPlan, committed: &[StatementPlan]) -> Result<(), Error> {
        if committed == plan.statements {
            return Ok(());
        }
        Err(self.reaction_unsupported(&format!(
            "reaction {k} whose commits {committed:?} are not the plan's statements {:?} (a transpiler defect)",
            plan.statements
        )))
    }

    /// The sources the reaction binds whatever its statements read: a drive's `.now` pin, an
    /// arming's `TIME`.
    fn forced_sources(plan: &ReactionPlan) -> Vec<Source> {
        match plan.mode {
            ReactionMode::Delayed(d) => vec![Source::Delayed(d)],
            ReactionMode::Arm(_) => vec![Source::Time],
            ReactionMode::Reaction(_) => Vec::new(),
        }
    }

    /// The comment line above the `fn`: the reaction's head, reads and commits.
    fn reaction_header(&self, k: usize, plan: &ReactionPlan, committed: &[StatementPlan]) -> String {
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
        format!(
            "// reaction {k} `{name}`: on {}; reads {}; commits {} @ {}",
            on.join(", "),
            reads.join(", "),
            writes.join(", "),
            reaction.map_or("", |r| r.origin.as_str())
        )
    }

    /// The refusal of `form` as a transpiler `Unsupported`, at this body.
    fn reaction_unsupported(&self, form: &str) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {}", self.body.name()), origin: self.body.origin().clone() }
    }
}
