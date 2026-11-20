//! Mutation, loops and exits of the IR as Rust (composite-values CV-01 to CV-03; design "Control
//! flow: known decisions print plain Rust").
//!
//! Where every decision is known the text is plain Rust: `let mut`, `x = e;`, `x = x + e;`,
//! `loop`, `while`, `for`, `break`, `continue`, `return`, each loop counted by an `rt::Trips` so
//! that a loop of more than 2^24 iterations is a panic, not a hang (Q5 (b), §1 rule 11). The
//! methods are `ValuePrinter`'s: `value.rs` only dispatches to them.

use piperine_ir::{BinaryOp, ExprId, Place, StmtId, ValueType};

use crate::Error;
use crate::print::value::{Piece, Prec, ValuePrinter};

/// The kind of loop whose body is being printed: what `break`, `continue` and `return` inside it
/// print as.
#[derive(Clone, Debug)]
pub(super) enum LoopFrame {
    /// A loop of Rust's own (`loop`, `while`, `for`): `break` and `continue` are Rust's. Holds the
    /// type of the loop's value, which a `break` value is lifted to.
    Counted(ValueType),
}

impl<'a> ValuePrinter<'a> {
    /// `place = value;`, or `place = place op value;` for `place op= value;`: the `rt` integers,
    /// `Flagged` and `Real` have the operators but no `op=` forms (§1.5.12).
    pub(super) fn assign(&self, place: &Place, op: Option<BinaryOp>, value: ExprId) -> Result<String, Error> {
        if !place.path.is_empty() {
            return Err(self.unsupported(value, "an assignment to a field of a local"));
        }
        let (Some(name), Some(ty)) = (self.local(place.local), self.local_types.get(place.local as usize)) else {
            return Err(self.unsupported(value, "an assignment to a local out of range"));
        };
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

    /// `loop { .. }` counted by an `rt::Trips`.
    pub(super) fn counted_loop(&self, e: ExprId, word: &str, body: &[StmtId]) -> Result<Piece, Error> {
        let lines = self.loop_body(LoopFrame::Counted(self.ir.expr_type(e).clone()), body)?;
        Ok(self.counted(e, word, &format!("loop {}", Self::braced(&format!("trips_.tick();{}", Self::after(&lines))))))
    }

    /// `while cond { .. }` counted by an `rt::Trips`; the condition is known.
    pub(super) fn counted_while(&self, e: ExprId, cond: ExprId, body: &[StmtId]) -> Result<Piece, Error> {
        if Self::is_flagged(self.ir.expr_type(cond)) {
            return Err(self.unsupported(e, "a `while` loop on a value that may be unknown"));
        }
        let lines = self.loop_body(LoopFrame::Counted(ValueType::Unit), body)?;
        let condition = self.piece(cond)?.at(Prec::Or);
        Ok(self.counted(e, "while", &format!("while {condition} {}", Self::braced(&format!("trips_.tick();{}", Self::after(&lines))))))
    }

    /// `for local in low..high { .. }` counted by an `rt::Trips`. A range over the `rt` integers
    /// is not iterable, so Rust counts the raw values and the body binds the loop's local from
    /// each (`.rev()` walks the range from the top down).
    pub(super) fn counted_for(&self, e: ExprId, local: u32, low: ExprId, high: ExprId, inclusive: bool, reverse: bool, body: &[StmtId]) -> Result<Piece, Error> {
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
        let Some(LoopFrame::Counted(ty)) = frame else {
            return Err(self.unsupported(e, "`break` outside a loop"));
        };
        let text = match value {
            Some(v) => format!("break {}", self.typed(v, &ty)?.at(Prec::Low)),
            None => "break".to_string(),
        };
        Ok(Piece::new(text, Prec::Low))
    }

    /// `continue` in the innermost loop.
    pub(super) fn continue_loop(&self, e: ExprId) -> Result<Piece, Error> {
        match self.loops.borrow().last() {
            Some(LoopFrame::Counted(_)) => Ok(Piece::new("continue".to_string(), Prec::Low)),
            None => Err(self.unsupported(e, "`continue` outside a loop")),
        }
    }

    /// `return` or `return value` from the function, the value lifted to its result type.
    pub(super) fn return_from_function(&self, e: ExprId, value: Option<ExprId>) -> Result<Piece, Error> {
        let Some(ty) = &self.returns else {
            return Err(self.unsupported(e, "`return` outside a function"));
        };
        let text = match value {
            Some(v) => format!("return {}", self.typed(v, ty)?.at(Prec::Low)),
            None => "return".to_string(),
        };
        Ok(Piece::new(text, Prec::Low))
    }
}
