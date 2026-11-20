//! Mutation, loops and exits of the IR as Rust (composite-values CV-01 to CV-03; design "Control
//! flow: known decisions print plain Rust").
//!
//! Where every decision is known the text is plain Rust: `let mut`, `x = e;`, `x = x + e;`,
//! `loop`, `while`, `for`, `break`, `continue`, `return`, each loop counted by an `rt::Trips` so
//! that a loop of more than 2^24 iterations is a panic, not a hang (Q5 (b), §1 rule 11). The
//! methods are `ValuePrinter`'s: `value.rs` only dispatches to them.
//!
//! Where a decision may be unknown (digital code only, §3.3.5) the text is the merge of §3.3.3:
//!
//! - an `if` on a value that may be unknown whose arms write mutable locals returns the value and
//!   each local they write as a tuple, which `Merge` merges field by field, and rebinds the
//!   locals from it (`merged_with_writes`);
//! - a loop with a `while` condition that may be unknown, or a `break`, `continue` or `return`
//!   under such an `if`, runs as `rt::Unrolled` (`unrolled_loop`): its body is a closure from the
//!   tuple of the locals the loop writes to an `rt::Step`, an unknown `if` holding an exit is
//!   `choose` (`commits_.branch` in a reaction) over both of its outcomes as `Step`s, and the rest
//!   of the iteration runs on every path that fell through (`Step::then`). In a reaction the loop
//!   is `Commits::unrolled`, so what it commits after its first unknown decision is a `MAYBE |
//!   MERGE` commit (MD-70).
//!
//! An exit under an unknown `if` anywhere but among the statements of its loop is refused by name:
//! there is no rest of the iteration to hand over.

use piperine_ir::{BinaryOp, Expr, ExprId, FunctionId, Place, Stmt, StmtId, ValueType};

use crate::Error;
use crate::print::value::{Piece, Prec, ValuePrinter};
use crate::print::Text;

/// The kind of loop whose body is being printed: what `break`, `continue` and `return` inside it
/// print as.
#[derive(Clone, Debug)]
pub(super) enum LoopFrame {
    /// A loop of Rust's own (`loop`, `while`, `for`): `break` and `continue` are Rust's. Holds the
    /// type of the loop's value, which a `break` value is lifted to.
    Counted(ValueType),
    /// A loop run as `rt::Unrolled`: `break`, `continue` and `return` are values of `rt::Step`.
    Unrolled {
        /// The printed names of the locals the loop writes: its state, `(x, y)`.
        names: Vec<String>,
        /// The type of the loop's value, which a `break` value is lifted to.
        value: ValueType,
        /// Whether the loop commits, so its unknown decisions are `commits_.branch`.
        commits: bool,
    },
}

/// One thing a body does in order: a statement, or the expression of an arm.
#[derive(Clone, Copy, Debug)]
pub(super) enum Item {
    /// A statement of a block.
    Stmt(StmtId),
    /// An expression run for its effect: an arm that is not a block, or a block's value.
    Expr(ExprId),
}

/// The lints a merge's copies of the locals trip: a copy an arm leaves alone needs no `mut`, an
/// exit before the end of an arm leaves its fall-through unreachable, and the locals a merge
/// hands back may not be read after it.
const SILENCED: &str = "#[allow(unused_mut, unused_assignments, unreachable_code)]";

impl<'a> ValuePrinter<'a> {
    /// `place = value;`, or `place = place op value;` for `place op= value;`: the `rt` integers,
    /// `Flagged` and `Real` have the operators but no `op=` forms (§1.5.12).
    pub(super) fn assign(&self, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        if !place.path.is_empty() {
            return self.assign_through(place, op, value);
        }
        let (Some(name), Some(ty)) = (self.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(self.unsupported(value, "an assignment to a local out of range"));
        };
        self.assign_to(name, ty, op, value)
    }

    /// `target = value;` or `target = target op value;`, `target` a Rust place of type `ty`.
    pub(super) fn assign_to(&self, name: &str, ty: &ValueType, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        let Some(op) = op else {
            return Ok(format!("{name} = {};", self.expr_as(value, ty)?));
        };
        let real = *Self::base(ty) == ValueType::F64 && !(Self::is_flagged(ty) || self.plain);
        let (symbol, prec) = self.operator(value, op, real)?;
        if matches!(prec, Prec::Compare | Prec::And | Prec::Or) {
            return Err(self.unsupported(value, &format!("`{op:?}=`: only an arithmetic or bit operator has a compound assignment")));
        }
        // a value that may be unknown is the operand as it is (`Flagged` has the operator for any
        // plain right side's lift), as the same operator prints it between two expressions
        let right = if Self::is_flagged(ty) { self.flagged(value)? } else { self.piece(value)? };
        let right = match prec {
            Prec::BitAnd | Prec::BitOr | Prec::BitXor | Prec::Shift => right.at(Prec::Unary),
            _ => right.at(Self::stronger(prec)),
        };
        Ok(format!("{name} = {name} {symbol} {right};"))
    }

    /// A loop, `loop`, `while` or `for`: plain Rust counted by an `rt::Trips`, or `rt::Unrolled` when
    /// a decision of it may be unknown.
    pub(super) fn loop_expr(&self, e: ExprId) -> Result<Piece, Error> {
        match self.ir.expr(e) {
            Expr::Loop { body } if self.is_unrolled(e) => self.unrolled_loop(e, None, body),
            Expr::While { cond, body } if self.is_unrolled(e) => self.unrolled_loop(e, Some(*cond), body),
            Expr::ForRange { .. } if self.is_unrolled(e) => Err(self.unsupported(e, "a `for` loop with an exit under a value that may be unknown (write it as a `while`)")),
            Expr::Loop { body } => self.counted_loop(e, "loop", body),
            Expr::While { cond, body } => self.counted_while(e, *cond, body),
            Expr::ForRange { local, low, high, inclusive, reverse, body } => self.counted_for(e, *local, *low, *high, *inclusive, *reverse, body),
            _ => Err(self.unsupported(e, "a form that is not a loop")),
        }
    }

    /// `loop { .. }` counted by an `rt::Trips`.
    fn counted_loop(&self, e: ExprId, word: &str, body: &[StmtId]) -> Result<Piece, Error> {
        let lines = self.loop_body(LoopFrame::Counted(self.ir.expr_type(e).clone()), body)?;
        Ok(self.counted(e, word, &format!("loop {}", Self::braced(&format!("trips_.tick();{}", Self::after(&lines))))))
    }

    /// `while cond { .. }` counted by an `rt::Trips`; the condition is known.
    fn counted_while(&self, e: ExprId, cond: ExprId, body: &[StmtId]) -> Result<Piece, Error> {
        let lines = self.loop_body(LoopFrame::Counted(ValueType::Unit), body)?;
        let condition = self.piece(cond)?.at(Prec::Or);
        Ok(self.counted(e, "while", &format!("while {condition} {}", Self::braced(&format!("trips_.tick();{}", Self::after(&lines))))))
    }

    /// `for local in low..high { .. }` counted by an `rt::Trips`. A range over the `rt` integers
    /// is not iterable, so Rust counts the raw values and the body binds the loop's local from
    /// each (`.rev()` walks the range from the top down).
    fn counted_for(&self, e: ExprId, local: u32, low: ExprId, high: ExprId, inclusive: bool, reverse: bool, body: &[StmtId]) -> Result<Piece, Error> {
        let (low_ty, high_ty) = (self.ir.expr_type(low), self.ir.expr_type(high));
        let Some(name) = self.local(local) else {
            return Err(self.unsupported(e, "a loop variable out of range"));
        };
        let path = match (low_ty, high_ty) {
            (ValueType::UInt(_) | ValueType::Int(_), _) if low_ty == high_ty => self.integer_path(low_ty, e)?,
            _ => return Err(self.unsupported(e, "a `for` range whose bounds are not one known integer type")),
        };
        let raw = crate::print::Text::generated("index");
        let (low, high) = (self.piece(low)?.at(Prec::Atom), self.piece(high)?.at(Prec::Atom));
        let dots = if inclusive { "..=" } else { ".." };
        let range = if reverse { format!("({low}.get(){dots}{high}.get()).rev()") } else { format!("{low}.get(){dots}{high}.get()") };
        let lines = self.loop_body(LoopFrame::Counted(ValueType::Unit), body)?;
        let bound = format!("let {name} = {path}::new({raw});");
        Ok(self.counted(e, "for", &format!("for {raw} in {range} {}", Self::braced(&format!("trips_.tick();\n{bound}{}", Self::after(&lines))))))
    }

    /// `text`, a loop, as a block that makes its `rt::Trips` first: `form` and the loop's origin
    /// name the loop when it runs past the bound.
    fn counted(&self, e: ExprId, form: &str, text: &str) -> Piece {
        let origin = self.ir.expr_origin(e);
        let named = format!("{form} @ {}:{}:{}", origin.file, origin.line, origin.col);
        let inside = format!("let mut trips_ = rt::Trips::new({named:?});\n{text}");
        Piece::new(Self::braced(&inside), Prec::Low)
    }

    /// The lines of `body` with `frame` as the innermost loop while they print.
    fn loop_body(&self, frame: LoopFrame, body: &[StmtId]) -> Result<String, Error> {
        self.loops.borrow_mut().push(frame);
        let lines = self.block_lines(body, None, &ValueType::Unit);
        self.loops.borrow_mut().pop();
        lines
    }

    /// `lines` after a line that precedes them: a newline and `lines`, or nothing when empty.
    fn after(lines: &str) -> String {
        if lines.is_empty() { String::new() } else { format!("\n{lines}") }
    }

    /// `break` or `break value` out of the innermost loop.
    pub(super) fn leave_loop(&self, e: ExprId, value: Option<ExprId>) -> Result<Piece, Error> {
        let frame = self.loops.borrow().last().cloned();
        let text = match frame {
            Some(LoopFrame::Counted(ty)) => match value {
                Some(v) => format!("break {}", self.typed(v, &ty)?.at(Prec::Low)),
                None => "break".to_string(),
            },
            Some(LoopFrame::Unrolled { names, value: ty, .. }) => {
                let state = Self::tuple_of(&names);
                let value = match value {
                    Some(v) => self.typed(v, &ty)?.text,
                    None => "()".to_string(),
                };
                format!("return rt::Step::Exit({state}, {value})")
            }
            None => return Err(self.unsupported(e, "`break` outside a loop")),
        };
        Ok(Piece::new(text, Prec::Low))
    }

    /// `continue` in the innermost loop.
    pub(super) fn continue_loop(&self, e: ExprId) -> Result<Piece, Error> {
        let text = match self.loops.borrow().last() {
            Some(LoopFrame::Counted(_)) => "continue".to_string(),
            Some(LoopFrame::Unrolled { names, .. }) => format!("return rt::Step::Next({})", Self::tuple_of(names)),
            None => return Err(self.unsupported(e, "`continue` outside a loop")),
        };
        Ok(Piece::new(text, Prec::Low))
    }

    /// `return` or `return value` from the function, the value lifted to its result type; inside
    /// a loop run as `rt::Unrolled` it is the step `Returned` of that loop's closure.
    pub(super) fn return_from_function(&self, e: ExprId, value: Option<ExprId>) -> Result<Piece, Error> {
        let Some(ty) = &self.returns else {
            return Err(self.unsupported(e, "`return` outside a function"));
        };
        let value = match value {
            Some(v) => Some(self.typed(v, ty)?.at(Prec::Low)),
            None => None,
        };
        Ok(Piece::new(self.returning(value), Prec::Low))
    }

    /// The text that returns `value` (`()` when none) from the function: Rust's `return`, or the
    /// step of the innermost loop run as `rt::Unrolled` that holds this code.
    fn returning(&self, value: Option<String>) -> String {
        let inside_unrolled = self.loops.borrow().iter().any(|f| matches!(f, LoopFrame::Unrolled { .. }));
        match (inside_unrolled, value) {
            (true, Some(v)) => format!("return rt::Step::Returned({v})"),
            (true, None) => "return rt::Step::Returned(())".to_string(),
            (false, Some(v)) => format!("return {v}"),
            (false, None) => "return".to_string(),
        }
    }

    // ---- control under unknown decisions (§3.3.3) ---------------------------------------------

    /// Whether loop `e` runs as `rt::Unrolled`: its `while` condition may be unknown, or an `if`
    /// on a value that may be unknown holds a `break`, `continue` or `return` among its statements.
    fn is_unrolled(&self, e: ExprId) -> bool {
        let kind = self.ir.expr(e);
        if let Expr::While { cond, .. } = kind {
            if Self::is_flagged(self.ir.expr_type(*cond)) {
                return true;
            }
        }
        let mut stack: Vec<ExprId> = kind.statements().iter().flat_map(|s| self.ir.stmt(*s).operands()).collect();
        while let Some(id) = stack.pop() {
            let inner = self.ir.expr(id);
            if let Expr::If { cond, then, otherwise } = inner {
                if Self::is_flagged(self.ir.expr_type(*cond)) && (self.holds_exit(&[Item::Expr(*then)], false) || self.holds_exit(&[Item::Expr(*otherwise)], false)) {
                    return true;
                }
            }
            // a nested loop decides for itself
            if !matches!(inner, Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. }) {
                stack.extend(inner.operands());
                stack.extend(inner.statements().iter().flat_map(|s| self.ir.stmt(*s).operands()));
            }
        }
        false
    }

    /// Whether `items` hold a `return`, or, unless `returns_only`, a `break` or `continue` of
    /// the loop they sit in (one of a nested loop is that loop's).
    fn holds_exit(&self, items: &[Item], returns_only: bool) -> bool {
        let mut stack: Vec<(ExprId, bool)> = items.iter().flat_map(|i| self.operands_of(*i)).map(|e| (e, false)).collect();
        while let Some((id, nested)) = stack.pop() {
            let kind = self.ir.expr(id);
            match kind {
                Expr::Return(_) => return true,
                Expr::Break(_) | Expr::Continue if !nested && !returns_only => return true,
                _ => {}
            }
            let inner = nested || matches!(kind, Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. });
            stack.extend(kind.operands().into_iter().map(|o| (o, inner)));
            stack.extend(kind.statements().iter().flat_map(|s| self.ir.stmt(*s).operands()).map(|o| (o, inner)));
        }
        false
    }

    /// The expressions `item` evaluates directly.
    fn operands_of(&self, item: Item) -> Vec<ExprId> {
        match item {
            Item::Stmt(s) => self.ir.stmt(s).operands(),
            Item::Expr(e) => vec![e],
        }
    }

    /// The items of an arm: a block's statements then its value, any other expression as one.
    fn arm_items(&self, e: ExprId) -> Vec<Item> {
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => stmts.iter().map(|s| Item::Stmt(*s)).chain(value.iter().map(|v| Item::Expr(*v))).collect(),
            Expr::Unit => Vec::new(),
            _ => vec![Item::Expr(e)],
        }
    }

    /// The text of one item as a line of a block.
    fn item_text(&self, item: Item) -> Result<String, Error> {
        match item {
            Item::Stmt(s) => self.stmt(s),
            Item::Expr(e) => self.expression_statement(e),
        }
    }

    /// `lines` as a block that is the value of a `let` under [`SILENCED`]: Rust takes a lint
    /// attribute on a `let`, not on an assignment.
    fn silenced(lines: &str) -> String {
        Self::braced(&format!("{SILENCED}\nlet result_ = {};\nresult_", Self::braced(lines)))
    }

    /// `(a, b)`, `(a,)` or `()`: a tuple of `items`.
    fn tuple_of(items: &[String]) -> String {
        match items {
            [one] => format!("({one},)"),
            many => format!("({})", many.join(", ")),
        }
    }

    /// `(mut a, mut b)`: a pattern that binds `names` as mutable locals.
    fn mutable_pattern(names: &[String]) -> String {
        Self::tuple_of(&names.iter().map(|n| format!("mut {n}")).collect::<Vec<_>>())
    }

    /// The printed names of the locals `e` writes. Under an unknown decision each is merged
    /// with its other path, so its type must be one that may be unknown; the tuple of them has at
    /// most 11 fields beside a value (`Merge` is written for tuples of 12).
    fn written_names(&self, e: ExprId) -> Result<Vec<String>, Error> {
        let written = self.ir.written_locals(e);
        if written.len() > 11 {
            return Err(self.unsupported(e, &format!("{} locals written under an unknown decision (at most 11)", written.len())));
        }
        let mut names = Vec::with_capacity(written.len());
        for local in written {
            let (Some(name), Some(ty)) = (self.local(local), self.local_types.get(local as usize)) else {
                return Err(self.unsupported(e, "a written local out of range"));
            };
            if !Self::is_flagged(ty) {
                return Err(self.unsupported(e, &format!("the local `{name}`, written under an unknown decision, has a type that cannot be unknown")));
            }
            names.push(name.to_string());
        }
        Ok(names)
    }

    /// The names the written locals take once the merge hands them back: `x_out_`.
    fn merged_names(names: &[String]) -> Vec<String> {
        names.iter().map(|n| Text::generated(&format!("{n}_out"))).collect()
    }

    /// `condition.choose(|| { first }, || { second })`, or in a reaction whose arms commit
    /// `commits_.branch(condition, |commits_| { first }, |commits_| { second })`; each arm's text
    /// without its braces. `joins` is the reaction case.
    fn merge_call(&self, condition: Piece, first: &str, second: &str, joins: bool) -> String {
        let (first, second) = (Self::indent(first), Self::indent(second));
        if joins {
            self.branches.set(true);
            // an arm that makes no commit of its own leaves its `Commits` unnamed
            let named = |arm: &str| if arm.contains("commits_") { "commits_" } else { "_" };
            format!("commits_.branch({}, |{}| {{\n{first}\n}}, |{}| {{\n{second}\n}})", condition.text, named(&first), named(&second))
        } else {
            format!("{}.choose(|| {{\n{first}\n}}, || {{\n{second}\n}})", condition.at(Prec::Atom))
        }
    }

    /// An `if` on a value that may be unknown whose arms write mutable locals (§3.3.3: the
    /// effects of a path are its value and each local it writes): each arm returns its value and
    /// those locals as a tuple, the merge merges the tuples field by field, and the locals are
    /// assigned from the result.
    pub(super) fn merged_with_writes(&self, e: ExprId, cond: ExprId, then: ExprId, otherwise: ExprId) -> Result<Piece, Error> {
        let names = self.written_names(e)?;
        let ty = self.ir.expr_type(e);
        let condition = self.piece(cond)?;
        let joins = self.commits && (self.holds_effect(then) || self.holds_effect(otherwise));
        // the arms print in source order, which is the order their commits are numbered
        let first = self.arm_with_writes(then, ty, &names)?;
        let second = self.arm_with_writes(otherwise, ty, &names)?;
        let call = self.merge_call(condition, &first, &second, joins);
        let outs = Self::merged_names(&names);
        let unit = *ty == ValueType::Unit;
        let mut fields = vec![if unit { "_".to_string() } else { "value_".to_string() }];
        fields.extend(outs.iter().cloned());
        let mut lines = vec![format!("let {} = {call};", Self::tuple_of(&fields))];
        lines.extend(names.iter().zip(&outs).map(|(n, o)| format!("{n} = {o};")));
        if !unit {
            lines.push("value_".to_string());
        }
        Ok(Piece::new(Self::silenced(&lines.join("\n")), Prec::Low))
    }

    /// An arm of `merged_with_writes`: its statements on copies of the locals it writes, then
    /// the tuple of its value and those locals.
    fn arm_with_writes(&self, arm: ExprId, want: &ValueType, names: &[String]) -> Result<String, Error> {
        let mut lines = Vec::new();
        if !names.is_empty() {
            lines.push(format!("let {} = {};", Self::mutable_pattern(names), Self::tuple_of(names)));
        }
        // the arm's value is a block's value, or the whole arm when it is no block
        let (stmts, value) = match self.ir.expr(arm) {
            Expr::Block { stmts, value } => (stmts.as_slice(), *value),
            Expr::Unit => (&[][..], None),
            _ => (&[][..], Some(arm)),
        };
        for s in stmts {
            lines.push(self.stmt(*s)?);
        }
        let value = match value {
            Some(v) => self.typed(v, want)?.text,
            None => "()".to_string(),
        };
        let mut fields = vec![value];
        fields.extend(names.iter().cloned());
        lines.push(Self::tuple_of(&fields));
        Ok(lines.join("\n"))
    }

    /// Loop `e` as `rt::Unrolled` (or `Commits::unrolled` in a reaction that commits in it); `cond`
    /// is the `while` condition, `body` the loop's statements.
    fn unrolled_loop(&self, e: ExprId, cond: Option<ExprId>, body: &[StmtId]) -> Result<Piece, Error> {
        let names = self.written_names(e)?;
        let value_ty = self.ir.expr_type(e).clone();
        if value_ty != ValueType::Unit && !Self::is_flagged(&value_ty) {
            return Err(self.unsupported(e, "a loop value that cannot be unknown, under an unknown decision"));
        }
        let commits = self.commits && self.holds_effect(e);
        if commits {
            self.branches.set(true);
        }
        let (state, pattern) = (Self::tuple_of(&names), Self::mutable_pattern(&names));
        self.loops.borrow_mut().push(LoopFrame::Unrolled { names: names.clone(), value: value_ty.clone(), commits });
        let lines = self.iteration(cond, body, &state, &pattern, commits);
        self.loops.borrow_mut().pop();
        let lines = lines?;
        let origin = self.ir.expr_origin(e);
        let named = format!("{} @ {}:{}:{}", if cond.is_some() { "while" } else { "loop" }, origin.file, origin.line, origin.col);
        let holds_return = self.holds_exit(&body.iter().map(|s| Item::Stmt(*s)).collect::<Vec<_>>(), true);
        let returns = match (&self.returns, holds_return) {
            (Some(ty), true) => self.type_of(ty)?,
            _ => "()".to_string(),
        };
        let value_type = self.type_of(&value_ty)?;
        let run = if commits {
            format!("commits_.unrolled::<_, {value_type}, {returns}>({named:?}, {state}, |commits_, {pattern}| {})", Self::braced(&lines))
        } else {
            format!("rt::Unrolled::<_, {value_type}, {returns}>::run({named:?}, {state}, |{pattern}| {})", Self::braced(&lines))
        };
        let outs = Self::merged_names(&names);
        let unit = value_ty == ValueType::Unit;
        let exit_pattern = format!("({}, {})", Self::tuple_of(&outs), if unit { "_" } else { "value_" });
        let exit = if holds_return {
            let leave = self.returning(Some("returned_".to_string()));
            format!("match outcome_.finish() {{\n    Ok(exit_) => exit_,\n    Err(returned_) => {leave},\n}}")
        } else {
            "outcome_.exit()".to_string()
        };
        let mut block = vec![format!("let outcome_ = {run};"), format!("let {exit_pattern} = {exit};")];
        block.extend(names.iter().zip(&outs).map(|(n, o)| format!("{n} = {o};")));
        if !unit {
            block.push("value_".to_string());
        }
        Ok(Piece::new(Self::silenced(&block.join("\n")), Prec::Low))
    }

    /// The lines of one iteration of an unrolled loop: the `while` condition, then the body,
    /// ending in `Next`.
    fn iteration(&self, cond: Option<ExprId>, body: &[StmtId], state: &str, pattern: &str, commits: bool) -> Result<String, Error> {
        let items: Vec<Item> = body.iter().map(|s| Item::Stmt(*s)).collect();
        let tail = format!("rt::Step::Next({state})");
        let Some(cond) = cond else {
            return self.steps(&items, &tail);
        };
        let condition = self.piece(cond)?;
        if !Self::is_flagged(self.ir.expr_type(cond)) {
            let rest = self.steps(&items, &tail)?;
            return Ok(format!("if !{} {{\n    return rt::Step::Exit({state}, ());\n}}\n{rest}", condition.at(Prec::Unary)));
        }
        // a condition that may be unknown: the iteration goes on where it holds and the loop
        // is left where it does not, both when it is unknown
        let call = self.merge_call(condition, &format!("rt::Step::Fall({state})"), &format!("rt::Step::Exit({state}, ())"), commits);
        let rest = self.steps(&items, &tail)?;
        Ok(format!("return {call}.then(|{pattern}| {});", Self::braced(&rest)))
    }

    /// `items` in order, then `tail`. An `if` on a value that may be unknown with an exit in an
    /// arm splits there: both arms end in `Fall`, or in the exit, and the rest of the items runs
    /// on every path that fell through.
    fn steps(&self, items: &[Item], tail: &str) -> Result<String, Error> {
        let mut lines = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let Some((cond, then, otherwise)) = self.exit_split(*item) else {
                lines.push(self.item_text(*item)?);
                continue;
            };
            let Some((names, commits)) = self.innermost_unrolled() else {
                return Err(self.unsupported(cond, "an exit under an unknown `if` outside a loop run as its unrolling"));
            };
            let (state, pattern) = (Self::tuple_of(&names), Self::mutable_pattern(&names));
            let condition = self.piece(cond)?;
            let fall = format!("rt::Step::Fall({state})");
            let first = self.arm_steps(then, &names, &fall)?;
            let second = self.arm_steps(otherwise, &names, &fall)?;
            let call = self.merge_call(condition, &first, &second, commits);
            let rest = self.steps(&items[i + 1..], tail)?;
            lines.push(format!("return {call}.then(|{pattern}| {});", Self::braced(&rest)));
            return Ok(lines.join("\n"));
        }
        lines.push(tail.to_string());
        Ok(lines.join("\n"))
    }

    /// An arm of an unknown `if` in an unrolled loop: its items on copies of the loop's locals,
    /// then `fall`.
    fn arm_steps(&self, arm: ExprId, names: &[String], fall: &str) -> Result<String, Error> {
        let copies = if names.is_empty() { String::new() } else { format!("let {} = {};\n", Self::mutable_pattern(names), Self::tuple_of(names)) };
        Ok(format!("{copies}{}", self.steps(&self.arm_items(arm), fall)?))
    }

    /// The written names of the innermost loop when it runs as `rt::Unrolled`, and whether it
    /// commits.
    fn innermost_unrolled(&self) -> Option<(Vec<String>, bool)> {
        match self.loops.borrow().last() {
            Some(LoopFrame::Unrolled { names, commits, .. }) => Some((names.clone(), *commits)),
            _ => None,
        }
    }

    /// The condition and arms of `item` when it is an `if` on a value that may be unknown with a
    /// `break`, `continue` or `return` in an arm.
    fn exit_split(&self, item: Item) -> Option<(ExprId, ExprId, ExprId)> {
        let e = match item {
            Item::Expr(e) => e,
            Item::Stmt(s) => match self.ir.stmt(s) {
                Stmt::Expr(e) => *e,
                _ => return None,
            },
        };
        let Expr::If { cond, then, otherwise } = self.ir.expr(e) else { return None };
        let arms = self.holds_exit(&[Item::Expr(*then)], false) || self.holds_exit(&[Item::Expr(*otherwise)], false);
        (Self::is_flagged(self.ir.expr_type(*cond)) && arms).then_some((*cond, *then, *otherwise))
    }

    /// Whether an unknown `if` with an exit in an arm sits where `steps` does not reach it.
    pub(super) fn holds_unreachable_exit(&self, then: ExprId, otherwise: ExprId) -> bool {
        self.holds_exit(&[Item::Expr(then)], false) || self.holds_exit(&[Item::Expr(otherwise)], false)
    }

    // ---- function values and recursion (CV-05 to CV-07) ---------------------------------------

    /// The Rust type of a function value: `&dyn Fn(A, B) -> R`, a reference so that a local of
    /// it is `Copy` as a function value is (§1.5.10). A function cannot return one: the reference
    /// would outlive its closure.
    pub(super) fn function_type(&self, params: &[ValueType], ret: &ValueType, path: &str) -> Result<String, Error> {
        let mut printed = Vec::with_capacity(params.len());
        for p in params {
            printed.push(self.rust_type(p, path)?);
        }
        let result = match ret {
            ValueType::Fn(..) => {
                return Err(Error::Unsupported { form: "a function value returning a function value".into(), path: path.to_string(), origin: self.origin.clone() });
            }
            other => self.rust_type(other, path)?,
        };
        Ok(format!("&dyn Fn({}) -> {result}", printed.join(", ")))
    }

    /// Whether `function` is on a call cycle: it reaches itself through calls and function values.
    pub fn is_recursive(ir: &piperine_ir::Ir, function: FunctionId) -> bool {
        Self::reaches(ir, function, function)
    }

    /// Whether `from` reaches `to` through one or more calls or function values.
    fn reaches(ir: &piperine_ir::Ir, from: FunctionId, to: FunctionId) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        let mut pending = vec![from];
        while let Some(f) = pending.pop() {
            let mut exprs = vec![ir.function(f).body()];
            while let Some(e) = exprs.pop() {
                let target = match ir.expr(e) {
                    Expr::Call { function, .. } | Expr::FunctionRef(function) => Some(*function),
                    _ => None,
                };
                if let Some(g) = target {
                    if g == to {
                        return true;
                    }
                    if seen.insert(g.index()) {
                        pending.push(g);
                    }
                }
                Self::children(ir, ir.expr(e), &mut exprs);
            }
        }
        false
    }

    /// The trailing `rt::Depth` argument of a call to `callee`: `depth_.deeper("callee")` from
    /// a function on a call cycle with it, `rt::Depth::root()` from anywhere else; nothing when
    /// `callee` is on no cycle.
    pub(super) fn depth_argument(&self, callee: FunctionId) -> Option<String> {
        if !Self::is_recursive(self.ir, callee) {
            return None;
        }
        let same_cycle = self.function.is_some_and(|caller| Self::reaches(self.ir, callee, caller) && Self::reaches(self.ir, caller, callee));
        Some(if same_cycle { format!("depth_.deeper({:?})", self.ir.function(callee).name()) } else { "rt::Depth::root()".to_string() })
    }

    /// A function used as a value: its path as a reference, or a closure over the trailing
    /// arguments it takes (the environment, the depth) so that its type is the `Fn` of its params.
    pub(super) fn function_value(&self, function: FunctionId) -> Result<Piece, Error> {
        let f = self.ir.function(function);
        let envs = Self::env_reads(self.ir, function);
        let depth = self.depth_argument(function);
        let generic = if Self::is_generic(self.ir, function) { format!("::<{}>", if self.plain { "f64" } else { "R" }) } else { String::new() };
        let name = format!("{}{generic}", Text::ident(f.name()));
        if envs.is_empty() && depth.is_none() {
            return Ok(Piece::new(format!("&{name}"), Prec::Unary));
        }
        let params: Vec<String> = (0..f.params().len()).map(|i| Text::generated(&format!("arg{i}"))).collect();
        let mut arguments = params.clone();
        arguments.extend(envs.into_iter().map(|env| self.read(Self::env_name(env))));
        arguments.extend(depth);
        Ok(Piece::new(format!("&move |{}| {name}({})", params.join(", "), arguments.join(", ")), Prec::Unary))
    }

    /// `callee(args)` where `callee` is a value of function type; each argument lifted to the
    /// parameter type the function type gives.
    pub(super) fn call_value(&self, e: ExprId, callee: ExprId, args: &[ExprId]) -> Result<Piece, Error> {
        let ValueType::Fn(params, _) = self.ir.expr_type(callee) else {
            return Err(self.unsupported(e, "a call through a value that is not a function"));
        };
        let mut printed = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            printed.push(match params.get(i) {
                Some(ty) => self.expr_as(*a, ty)?,
                None => self.expr(*a)?,
            });
        }
        Ok(Piece::new(format!("{}({})", self.piece(callee)?.at(Prec::Atom), printed.join(", ")), Prec::Atom))
    }
}
