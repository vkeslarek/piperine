//! Mutation, loops and exits of the IR as Rust (composite-values CV-01 to CV-03; design "Control
//! flow: known decisions print plain Rust").
//!
//! Where every decision is known the text is plain Rust: `let mut`, `x = e;`, `x = x + e;`,
//! `loop`, `while`, `for`, `break`, `continue`, `return`, each loop counted by an `rt::Trips` so
//! that a loop of more than 2^24 iterations is a panic, not a hang (Q5 (b), §1 rule 11). The
//! methods are `ValuePrinter`'s: `value.rs` only dispatches to them.

use piperine_ir::{BinaryOp, Expr, ExprId, FunctionId, Place, StmtId, ValueType};

use crate::Error;
use crate::print::value::{Piece, Prec, ValuePrinter};
use crate::print::Text;

impl<'a> ValuePrinter<'a> {
    /// `place = value;`, or `place = place op value;` for `place op= value;`: the `rt` integers,
    /// `Real` has the operators but no `op=` forms (§1.5.12).
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
            // a temporary an inlined `Option` call assigns may never be read (`f().is_some()`): an
            // attribute on an assignment is unstable, on a block statement it is not
            return Ok(format!("#[allow(unused_assignments)] {{ {name} = {}; }}", self.expr(value)?));
        };
        let real = *ty == ValueType::F64 && !self.plain;
        let (symbol, prec) = self.operator(value, op, real)?;
        if matches!(prec, Prec::Compare | Prec::And | Prec::Or) {
            return Err(self.unsupported(value, &format!("`{op:?}=`: only an arithmetic or bit operator has a compound assignment")));
        }
        let right = self.piece(value)?;
        let right = match prec {
            Prec::BitAnd | Prec::BitOr | Prec::BitXor | Prec::Shift => right.at(Prec::Unary),
            _ => right.at(Self::stronger(prec)),
        };
        // the last assignment of a loop may never be read: an attribute on an assignment is
        // unstable, on a block statement it is not
        Ok(format!("#[allow(unused_assignments)] {{ {name} = {name} {symbol} {right}; }}"))
    }

    /// A loop, `loop`, `while` or `for`: plain Rust counted by an `rt::Trips`.
    pub(super) fn loop_expr(&self, e: ExprId) -> Result<Piece, Error> {
        match self.ir.expr(e) {
            Expr::Loop { body } => self.counted_loop(e, "loop", body),
            Expr::While { cond, body } => self.counted_while(e, *cond, body),
            Expr::ForRange { local, low, high, inclusive, reverse, body } => self.counted_for(e, *local, *low, *high, *inclusive, *reverse, body),
            _ => Err(self.unsupported(e, "a form that is not a loop")),
        }
    }

    /// `loop { .. }` counted by an `rt::Trips`.
    fn counted_loop(&self, e: ExprId, word: &str, body: &[StmtId]) -> Result<Piece, Error> {
        let lines = self.loop_body(body)?;
        Ok(self.counted(e, word, &format!("loop {}", Self::braced(&format!("trips_.tick();{}", Self::after(&lines))))))
    }

    /// `while cond { .. }` counted by an `rt::Trips`.
    fn counted_while(&self, e: ExprId, cond: ExprId, body: &[StmtId]) -> Result<Piece, Error> {
        let lines = self.loop_body(body)?;
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
        let lines = self.loop_body(body)?;
        // the loop variable of `for k in 0..N` may be unread: the loop then counts trips only
        let bound = format!("#[allow(unused_variables)] let {name} = {path}::new({raw});");
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

    /// The lines of `body`, counted as inside a loop while they print.
    fn loop_body(&self, body: &[StmtId]) -> Result<String, Error> {
        self.loops.set(self.loops.get() + 1);
        let lines = self.block_lines(body, None);
        self.loops.set(self.loops.get() - 1);
        lines
    }

    /// `lines` after a line that precedes them: a newline and `lines`, or nothing when empty.
    fn after(lines: &str) -> String {
        if lines.is_empty() { String::new() } else { format!("\n{lines}") }
    }

    /// `break` or `break value` out of the innermost loop.
    pub(super) fn leave_loop(&self, e: ExprId, value: Option<ExprId>) -> Result<Piece, Error> {
        if self.loops.get() == 0 {
            return Err(self.unsupported(e, "`break` outside a loop"));
        }
        let text = match value {
            Some(v) => format!("break {}", self.piece(v)?.at(Prec::Low)),
            None => "break".to_string(),
        };
        Ok(Piece::new(text, Prec::Low))
    }

    /// `continue` in the innermost loop.
    pub(super) fn continue_loop(&self, e: ExprId) -> Result<Piece, Error> {
        if self.loops.get() == 0 {
            return Err(self.unsupported(e, "`continue` outside a loop"));
        }
        Ok(Piece::new("continue".to_string(), Prec::Low))
    }

    /// `return` or `return value` from the function.
    pub(super) fn return_from_function(&self, e: ExprId, value: Option<ExprId>) -> Result<Piece, Error> {
        if self.returns.is_none() {
            return Err(self.unsupported(e, "`return` outside a function"));
        }
        let text = match value {
            Some(v) => format!("return {}", self.piece(v)?.at(Prec::Low)),
            None => "return".to_string(),
        };
        Ok(Piece::new(text, Prec::Low))
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

    /// `callee(args)` where `callee` is a value of function type.
    pub(super) fn call_value(&self, e: ExprId, callee: ExprId, args: &[ExprId]) -> Result<Piece, Error> {
        if !matches!(self.ir.expr_type(callee), ValueType::Fn(..)) {
            return Err(self.unsupported(e, "a call through a value that is not a function"));
        }
        let printed = args.iter().map(|a| self.expr(*a)).collect::<Result<Vec<_>, Error>>()?;
        Ok(Piece::new(format!("{}({})", self.piece(callee)?.at(Prec::Atom), printed.join(", ")), Prec::Atom))
    }
}
