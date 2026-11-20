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
//! **Unknown control.** An `if` on a condition that may be unknown is the merge of §3.3.3: when
//! its arms commit or emit it is `commits_.branch(c, |commits_| { A }, |commits_| { B })`, which
//! runs both arms as paths of the reaction's `Commits` and joins what they committed, and a
//! reaction holding one, or running on a guard that may be unknown, builds
//! `rt::Commits::controlled(&CARRIERS_, resolution_)`, `CARRIERS_` being the carrier each
//! statement commits to and `resolution_` the session's tick, so that two delays that round to one
//! tick are one key of the join. A commit's delay that is unknown
//! commits `Unknown` at the next microstep (`Commits::commit`, RS-11 AC 6). A `finish` is told to
//! `Commits::finish`, and the reaction returns `Commits::flow`.
//!
//! The reaction of a drive with an `after` (RS-12 AC 5) commits the undelayed value its `.now`
//! pin holds to the carrier after the delay.
//!
//! The arming reaction of an `at(t)` event (msl-digital A3) evaluates `t` and hands it to
//! `Commits::arm` with the instant's `TIME`: one `REPLACE` firing of the timer's pin, `t - now`
//! later, at the next microstep when `t` is now, never when `t` is unknown or past.

use piperine_ir::{Env, EventSource, ValueType};
use piperine_plan::{ReactionMode, ReactionPlan, Source, StatementPlan, Target};

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
            ReactionMode::Arm(e) => {
                let Some(event) = self.body.events().get(e as usize) else {
                    return Err(self.reaction_unsupported(&format!("event {e}")));
                };
                let EventSource::At(time) = event.source else {
                    return Err(self.reaction_unsupported(&format!("event {e} as a timer")));
                };
                let values = ValuePrinter::of_digital(self.ir, self.body).with_reads(self.reads());
                let at = values.expr_as(time, &ValueType::Flagged(Box::new(ValueType::F64)))?;
                // the time is compared with the instant's own `TIME` (abi.md §4.2)
                let now = self.names.read_name(Source::Time);
                (values, vec![format!("commits_.arm(0, {at}, {now});")], event.origin.clone())
            }
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
            ReactionMode::Arm(_) => vec![Source::Time],
            ReactionMode::Reaction(_) => Vec::new(),
        };
        // a reaction on an event whose guard may be unknown may not have run at all (§13 rule 1):
        // it knows it ran for sure when an event of its head fired with a guard that is known (or
        // none), and otherwise runs its body as a path joined with not running it
        let definite = self.definite(&values, plan)?;
        // a reaction that joins paths, or runs as a maybe, keys its commits by due instant: it
        // reads the session's tick, and the carrier each of its statements commits to, so two
        // commits to one carrier merge exactly (§3.3.3, §13 rule 1)
        let resolution = self.names.read_name(Source::Resolution);
        let carriers = (values.branches() || definite.is_some()).then(|| {
            values.read(&resolution);
            let table: Vec<String> = committed.iter().map(|s| match s.target {
                Target::Carrier(c) => format!("Some({c})"),
                _ => "None".to_string(),
            }).collect();
            format!("const CARRIERS_: &[Option<u32>] = &[{}];", table.join(", "))
        });
        let mut binds = self.binds(&values, "cx_", &plan.reads, &forced)?;
        // `freq()` read through a function: no `FREQ` cell in a reaction, the value is 0 (§14.4.4)
        let freq = ValuePrinter::env_name(Env::Freq);
        if values.uses(freq) {
            binds.push(format!("let {freq} = 0.0;"));
        }

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
            if let Some(table) = &carriers {
                t.line(table);
            }
            let make = if carriers.is_some() { format!("rt::Commits::controlled(&CARRIERS_, {resolution})") } else { "rt::Commits::new()".to_string() };
            // every use of `Commits` but a bare `flush` takes `&mut self`
            let mutable = !committed.is_empty() || carriers.is_some() || values.finishes() || definite.is_some();
            t.line(&format!("let {}commits_ = {make};", if mutable { "mut " } else { "" }));
            match &definite {
                Some(definite) => {
                    t.line(&format!("let definite_ = {definite};"));
                    let body = lines.join("\n");
                    let named = if body.contains("commits_") { "commits_" } else { "_" };
                    t.line(&format!("commits_.run(definite_, |{named}| {{"));
                    t.indented(|t| {
                        for line in &lines {
                            t.line(line);
                        }
                    });
                    t.line("});");
                }
                None => {
                    for line in &lines {
                        t.line(line);
                    }
                }
            }
            if values.finishes() {
                t.line("let flow_ = commits_.flow();");
            }
            t.line("commits_.flush(cx_)?;");
            t.line(if values.finishes() { "Ok(flow_)" } else { "Ok(Flow::Continue)" });
        });
        t.line("}");
        let mut text = t.finish();
        text.pop();
        Ok(text)
    }

    /// The condition under which reaction `plan` surely ran, when one of the events of its head has
    /// a guard that may be unknown: some event of the head fired and its guard is known (a known
    /// or absent guard is known). `None` when no guard may be unknown, so the body is not a path.
    fn definite(&self, values: &ValuePrinter<'_>, plan: &ReactionPlan) -> Result<Option<String>, Error> {
        let ReactionMode::Reaction(r) = plan.mode else { return Ok(None) };
        let Some(reaction) = self.body.reactions().get(r as usize) else { return Ok(None) };
        let flagged = |e: u32| self.body.events().get(e as usize).and_then(|event| event.guard).filter(|g| matches!(self.ir.expr_type(*g), ValueType::Flagged(_)));
        if !reaction.on.iter().any(|e| flagged(*e).is_some()) {
            return Ok(None);
        }
        let mut terms = Vec::with_capacity(reaction.on.len());
        for e in &reaction.on {
            let fired = values.read(&self.names.read_name(Source::Fired(*e)));
            terms.push(match flagged(*e) {
                // the reaction recomputes the guard from the reads it holds (§13 rule 1)
                Some(guard) => format!("{fired} && {}.is_known()", values.operand(guard)?),
                None => fired,
            });
        }
        Ok(Some(terms.join(" || ")))
    }

    fn reaction_unsupported(&self, form: &str) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {}", self.body.name()), origin: self.body.origin().clone() }
    }
}
