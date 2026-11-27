//! `RecordPrinter`: the record writes of the IR as Rust: a trace (`emit!` in `analog`), an
//! `emit!` of a reaction, and a commit of a reaction to a carrier. Each write of a reaction goes
//! through a statement of the shape (`StatementPlan`), numbered in printing order; the printer
//! keeps that order (`committed`) and whether an `emit!` ends the analysis (`finishes`). The
//! values a write holds print through the owner's `ExpressionPrinter`.

use std::cell::{Cell, RefCell};

use piperine_ir::{Body, CommitTarget, EmitField, ExprId, StmtId, Stream, ValueType};
use piperine_plan::{RecordLayout, StatementPlan, Target};

use crate::Error;
use crate::print::ExpressionPrinter;

/// Prints the record writes of one owner: the record streams and trace sites of its body, the
/// carriers a reaction commits to, and what the code printed so far commits.
#[derive(Debug)]
pub struct RecordPrinter {
    /// The type each carrier of the owner's body holds, by index; empty for a function.
    carriers: Vec<ValueType>,
    /// The record streams of the owner's body, by index; empty for a function.
    streams: Vec<Stream>,
    /// The stream each trace site of the owner's body writes, by site; empty for a function.
    traces: Vec<u32>,
    /// Whether a commit has a place to go: the code of a reaction.
    commits: bool,
    /// The carrier and `min_after` of each commit printed so far, in printing order: the order
    /// of the reaction's statements in the shape.
    committed: RefCell<Vec<StatementPlan>>,
    /// Whether the code printed so far holds an `emit!` on the stream `sim` that sets `finish`:
    /// the reaction then asks its `Commits` for the flow (`Commits::finish`, `Commits::flow`;
    /// msl-fixes FIX-01, §19 Resolution).
    finishes: Cell<bool>,
}

impl RecordPrinter {
    /// The name of the buffer `terms_block` fills with the records of its trace sites.
    pub const TRACES: &'static str = "traces_";

    /// The printer of the record writes of an owner with no body: a function or a type item, which has none.
    pub(crate) fn of_function() -> RecordPrinter {
        RecordPrinter { carriers: Vec::new(), streams: Vec::new(), traces: Vec::new(), commits: false, committed: RefCell::new(Vec::new()), finishes: Cell::new(false) }
    }

    /// The printer of the record writes of the code of `body`: its streams, trace sites and carriers, no commit.
    pub(crate) fn of_body(body: Body<'_>) -> RecordPrinter {
        RecordPrinter {
            carriers: body.carriers().iter().map(|c| c.ty.clone()).collect(),
            streams: body.streams().to_vec(),
            traces: body.traces().iter().map(|t| t.stream).collect(),
            commits: false,
            committed: RefCell::new(Vec::new()),
            finishes: Cell::new(false),
        }
    }

    /// This printer where a commit has a place to go: the code of a reaction.
    pub(crate) fn committing(self) -> RecordPrinter {
        RecordPrinter { commits: true, ..self }
    }

    /// The carrier and `min_after` of each commit printed so far, in printing order.
    pub fn committed(&self) -> Vec<StatementPlan> {
        self.committed.borrow().clone()
    }

    /// Whether the code printed so far ends the analysis: an `emit!` on `sim` with a `finish` field.
    pub fn finishes(&self) -> bool {
        self.finishes.get()
    }

    /// The commit of statement `s` of a reaction: `value` (from `after`, `0.0` when none) to `carrier` through
    /// `target`, numbered as the next statement of the shape.
    pub(crate) fn commit(&self, expressions: &ExpressionPrinter<'_>, s: StmtId, carrier: u32, value: ExprId, after: Option<ExprId>, target: Option<&CommitTarget>) -> Result<String, Error> {
        if !self.commits {
            return Err(Error::Unsupported {
                form: "commit".into(),
                path: format!("{} s{}", expressions.path, s.index()),
                origin: expressions.ir.expr_origin(value).clone(),
            });
        }
        if self.carriers.get(carrier as usize).is_none() {
            return Err(expressions.unsupported(value, "a commit to a carrier out of range"));
        }
        let statement = Self::commit_statement(expressions, value, carrier, after, target)?;
        let value = expressions.expr(value)?;
        let after = match after {
            Some(a) => expressions.expr(a)?,
            None => "0.0".to_string(),
        };
        let index = {
            let mut done = self.committed.borrow_mut();
            done.push(statement);
            done.len() - 1
        };
        if value.contains("commits_") || after.contains("commits_") {
            // a value that emits (a block that holds an `emit!`) borrows `commits_`
            // mutably: as an argument of `commits_.commit(..)` it would borrow it twice.
            // The arguments are evaluated first, in the order they print
            let (value, after) = (ExpressionPrinter::indent(&value), ExpressionPrinter::indent(&after));
            let call = Self::commit_call(expressions, index, carrier, target, "value_", "after_")?;
            return Ok(format!("{{\n    let value_ =\n{value};\n    let after_ =\n{after};\n    {call}\n}}"));
        }
        Self::commit_call(expressions, index, carrier, target, &value, &after)
    }

    /// The statement a commit of `carrier` through `target` goes through: the whole pin, one element, or the range of
    /// a field (what `piperine-plan` declares for the same commit).
    fn commit_statement(expressions: &ExpressionPrinter<'_>, e: ExprId, carrier: u32, after: Option<ExprId>, target: Option<&CommitTarget>) -> Result<StatementPlan, Error> {
        let Some(held) = expressions.facts.get(carrier as usize) else {
            return Err(expressions.unsupported(e, "a commit to a carrier out of range"));
        };
        let path = target.map_or(&[][..], |t| t.path.as_slice());
        let statement = StatementPlan::commit_through(expressions.ir, carrier, &held.ty, held.count, after, path);
        // a commit of a resolved carrier writes its drive pin
        Ok(if held.resolved { StatementPlan { target: Target::ResolvedDrive(carrier), ..statement } } else { statement })
    }

    /// The call that makes a commit through statement number `index`: an element commit names the element, checked
    /// against the count (CV-25 AC 5), every other commit goes through the statement's range.
    fn commit_call(expressions: &ExpressionPrinter<'_>, index: usize, carrier: u32, target: Option<&CommitTarget>, value: &str, after: &str) -> Result<String, Error> {
        match target.and_then(|t| t.element) {
            Some(element) => {
                let facts = &expressions.facts[carrier as usize];
                let count = facts.count.unwrap_or(0);
                Ok(format!("commits_.commit_element({index}, {}, {count}, {:?}, {value}, {after});", expressions.index_value(element)?, facts.path))
            }
            None if expressions.facts[carrier as usize].resolved => Ok(format!("commits_.commit_drive({index}, {value}, {after});")),
            None => Ok(format!("commits_.commit({index}, {value}, {after});")),
        }
    }

    /// A trace (`emit!` in `analog`, §19 rule 4): the record of its stream, its `BOOL` presence byte
    /// set and each value field written where the layout puts it, stored in the `traces_` buffer of
    /// the site; `terms` copies the buffer to the site's `TRACE` watch. A statement that does not
    /// execute leaves the buffer empty, which is the empty record (abi.md A5). The text fields stay
    /// in the IR.
    pub(crate) fn trace(&self, expressions: &ExpressionPrinter<'_>, s: StmtId, site: u32, fields: &[EmitField]) -> Result<String, Error> {
        let origin = fields
            .iter()
            .find_map(|f| match f {
                EmitField::Value { value, .. } => Some(expressions.ir.expr_origin(*value).clone()),
                EmitField::Text { .. } => None,
            })
            .unwrap_or_else(|| expressions.origin.clone());
        let unsupported = |form: &str| Error::Unsupported { form: form.to_string(), path: format!("{} s{}", expressions.path, s.index()), origin: origin.clone() };
        let Some(declared) = self.traces.get(site as usize).and_then(|stream| self.streams.get(*stream as usize)) else {
            return Err(unsupported("a trace of a site the body does not have"));
        };
        let Some(layout) = RecordLayout::in_ir(expressions.ir, declared) else {
            return Err(unsupported(&format!("a trace to stream `{}`, whose fields no pin holds", declared.name)));
        };
        let start = layout.trace_start();
        let mut writes = Vec::new();
        let mut texts = Vec::new();
        for f in fields {
            match f {
                EmitField::Value { field, value } => {
                    let (Some((offset, _)), Some(field_type)) = (layout.fields.get(*field as usize), declared.fields.get(*field as usize)) else {
                        return Err(unsupported(&format!("a trace of field {field} of stream `{}`, out of range", declared.name)));
                    };
                    let mut text = expressions.expr(*value)?;
                    // a real is an `R` in a generic block: its value is what the record holds
                    if field_type.ty == ValueType::F64 && !expressions.plain {
                        text = format!("({text}).value()");
                    }
                    writes.push(format!("rt::Carried::write_pin({text}, &mut record_[{}..]); // {}", start + offset, field_type.name));
                }
                EmitField::Text { name, text } => texts.push(format!("{name} = {text:?}")),
            }
        }
        let note = if texts.is_empty() { String::new() } else { format!("; its text {} stays in the IR", texts.join(", ")) };
        let mut lines = vec![format!("// trace to stream `{}` @ {}:{}:{}{note}", declared.name, origin.file, origin.line, origin.col)];
        lines.push(format!("let mut record_ = [0u8; {}];", layout.trace_size()));
        lines.push("record_[0] = 1;".to_string());
        lines.extend(writes);
        lines.push(format!("{}[{site}] = Some(record_.to_vec());", Self::TRACES));
        Ok(lines.join("\n"))
    }

    /// An emit site (msl-digital A4): the record's bytes in its stream's layout (`RecordLayout`),
    /// all zero first so a field the site leaves out has its zero (abi.md C5), each value
    /// field written where the layout puts it, and the record committed through the site's
    /// statement. The text fields stay in the IR (`Body::record_site`): a comment names them.
    pub(crate) fn emit(&self, expressions: &ExpressionPrinter<'_>, s: StmtId, stream: u32, fields: &[EmitField]) -> Result<String, Error> {
        let origin = fields
            .iter()
            .find_map(|f| match f {
                EmitField::Value { value, .. } => Some(expressions.ir.expr_origin(*value).clone()),
                EmitField::Text { .. } => None,
            })
            .unwrap_or_else(|| expressions.origin.clone());
        let unsupported = |form: &str| Error::Unsupported { form: form.to_string(), path: format!("{} s{}", expressions.path, s.index()), origin: origin.clone() };
        if !self.commits {
            return Err(unsupported("emit! outside a reaction"));
        }
        let Some(declared) = self.streams.get(stream as usize) else {
            return Err(unsupported("emit! to a stream out of range"));
        };
        let Some(layout) = RecordLayout::in_ir(expressions.ir, declared) else {
            return Err(unsupported(&format!("emit! to stream `{}`, whose fields no pin holds", declared.name)));
        };
        let stream_name = declared.name.as_str();
        let mut evaluated = Vec::new();
        let mut writes = Vec::new();
        let mut texts = Vec::new();
        for f in fields {
            match f {
                EmitField::Value { field, value } => {
                    let (Some((offset, _)), Some(declared)) = (layout.fields.get(*field as usize), declared.fields.get(*field as usize)) else {
                        return Err(unsupported(&format!("emit! of field {field} of stream `{}`, out of range", declared.name)));
                    };
                    let mut value = expressions.expr(*value)?;
                    if value.contains("commits_") {
                        // a value that emits is evaluated before the record's own `commits_`
                        // calls, so that two never borrow it at once
                        let name = format!("field{field}_");
                        evaluated.push(format!("let {name} =\n{};", ExpressionPrinter::indent(&value)));
                        value = name;
                    }
                    writes.push(format!("rt::Carried::write_pin({value}, &mut record_[{offset}..]); // {}", declared.name));
                    if declared.name == "finish" && stream_name == "sim" {
                        writes.push(format!("commits_.finish({value});"));
                        self.finishes.set(true);
                    }
                }
                EmitField::Text { name, text } => texts.push(format!("{name} = {text:?}")),
            }
        }
        let index = {
            let mut done = self.committed.borrow_mut();
            done.push(StatementPlan::record(stream));
            done.len() - 1
        };
        let text = if texts.is_empty() { String::new() } else { format!("; its text {} stays in the IR", texts.join(", ")) };
        let mut lines = vec![format!("// emit! to stream `{}` @ {}:{}:{}{text}", declared.name, origin.file, origin.line, origin.col)];
        lines.extend(evaluated);
        // a stream with no value field writes nothing into its zero bytes
        let binding = if writes.is_empty() { "let" } else { "let mut" };
        lines.push(format!("{binding} record_ = [0u8; {}];", layout.size));
        lines.extend(writes);
        lines.push(format!("commits_.record({index}, &record_);"));
        Ok(lines.join("\n"))
    }

}
