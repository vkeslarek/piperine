//! Loops, exits and function values of the IR as Rust (composite-values CV-01 to CV-07; design
//! "Control flow: known decisions print plain Rust"); an assignment is a statement
//! (`print/statement.rs`).
//!
//! Where every decision is known the text is plain Rust: `loop`, `while`, `for`, `break`,
//! `continue`, `return`, each loop counted by an `rt::Trips` so that a loop of more than 2^24
//! iterations is a panic, not a hang (Q5 (b), §1 rule 11). The methods are
//! `ExpressionPrinter`'s: `print/expression.rs` only dispatches to them.

use piperine_ir::{Expr, ExprId, FunctionId, StmtId, ValueType};

use crate::Error;
use crate::print::expression::{ExpressionPrinter, Piece, Prec};
use crate::print::Text;

impl<'a> ExpressionPrinter<'a> {
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

    /// The Rust type of a function value: `std::rc::Rc<dyn Fn(A, B) -> R>`, one owned
    /// representation for every function value (a param, a local, a result, a capture), so that a
    /// function can return one, a capturing closure included (§1.5.10, §1.5.14). A read clones it
    /// (`ExpressionPrinter::local_value`), as a function value is copied.
    pub(super) fn function_type(&self, params: &[ValueType], ret: &ValueType, path: &str) -> Result<String, Error> {
        let mut printed = Vec::with_capacity(params.len());
        for p in params {
            printed.push(self.rust_type(p, path)?);
        }
        Ok(format!("std::rc::Rc<dyn Fn({}) -> {}>", printed.join(", "), self.rust_type(ret, path)?))
    }

    /// The read of local `name` of type `ty` as a value: a function value is cloned (the `Rc` is
    /// shared, the read leaves the local usable); any other local is its name.
    pub(super) fn local_value(name: &str, ty: &ValueType) -> String {
        if matches!(ty, ValueType::Fn(..)) { format!("{name}.clone()") } else { name.to_string() }
    }

    /// The closure `e` as a function value: `std::rc::Rc::new(move |..| ..)`. A `move` closure
    /// takes each local it reads, so each function value it captures from outside is cloned
    /// first (`{ let f = f.clone(); std::rc::Rc::new(move |..| f(..)) }`) and stays usable after.
    pub(super) fn closure_value(&self, e: ExprId) -> Result<Piece, Error> {
        let value = format!("std::rc::Rc::new({})", self.closure(e)?);
        let mut lines = Vec::new();
        for local in self.captured_function_values(e) {
            let Some(name) = self.local(local) else {
                return Err(self.unsupported(e, "a captured local out of range"));
            };
            lines.push(format!("let {name} = {name}.clone();"));
        }
        if lines.is_empty() {
            return Ok(Piece::new(value, Prec::Atom));
        }
        lines.push(value);
        Ok(Piece::new(format!("{{\n{}\n}}", Self::indent(&lines.join("\n"))), Prec::Low))
    }

    /// The function-valued locals closure `e` reads that are bound outside it, in index order:
    /// every `Local` read in its body but its own params, the params of the closures inside it
    /// and the `let` and `for` locals of the blocks and loops inside it.
    fn captured_function_values(&self, e: ExprId) -> Vec<u32> {
        let mut bound = std::collections::BTreeSet::new();
        let mut read = std::collections::BTreeSet::new();
        let mut exprs = vec![e];
        while let Some(id) = exprs.pop() {
            let kind = self.ir.expr(id);
            match kind {
                Expr::Closure { params, .. } => bound.extend(params.iter().copied()),
                Expr::ForRange { local, .. } => {
                    bound.insert(*local);
                }
                Expr::Local(i) if matches!(self.ir.expr_type(id), ValueType::Fn(..)) => {
                    read.insert(*i);
                }
                _ => {}
            }
            for s in kind.statements() {
                if let piperine_ir::Stmt::Let { local, .. } = self.ir.stmt(*s) {
                    bound.insert(*local);
                }
            }
            Self::children(self.ir, kind, &mut exprs);
        }
        read.difference(&bound).copied().collect()
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

    /// A function used as a value: its path in an `Rc`, or a closure over the trailing arguments it
    /// takes (the environment, the depth) so that its type is the `Fn` of its params.
    pub(super) fn function_value(&self, function: FunctionId) -> Result<Piece, Error> {
        let f = self.ir.function(function);
        let envs = Self::env_reads(self.ir, function);
        let depth = self.depth_argument(function);
        let generic = if Self::is_generic(self.ir, function) { format!("::<{}>", if self.plain { "f64" } else { "R" }) } else { String::new() };
        let name = format!("{}{generic}", Text::ident(f.name()));
        if envs.is_empty() && depth.is_none() {
            return Ok(Piece::new(format!("std::rc::Rc::new({name})"), Prec::Atom));
        }
        let params: Vec<String> = (0..f.params().len()).map(|i| Text::generated(&format!("arg{i}"))).collect();
        let mut arguments = params.clone();
        arguments.extend(envs.into_iter().map(|env| self.read(Self::env_name(env))));
        arguments.extend(depth);
        Ok(Piece::new(format!("std::rc::Rc::new(move |{}| {name}({}))", params.join(", "), arguments.join(", ")), Prec::Atom))
    }

    /// `callee(args)` where `callee` is a value of function type: a local is called by its name,
    /// through the `Rc` it holds, not cloned.
    pub(super) fn call_value(&self, e: ExprId, callee: ExprId, args: &[ExprId]) -> Result<Piece, Error> {
        if !matches!(self.ir.expr_type(callee), ValueType::Fn(..)) {
            return Err(self.unsupported(e, "a call through a value that is not a function"));
        }
        let printed = args.iter().map(|a| self.expr(*a)).collect::<Result<Vec<_>, Error>>()?;
        let called = match self.ir.expr(callee) {
            Expr::Local(i) => match self.local(*i) {
                Some(name) => name.to_string(),
                None => return Err(self.unsupported(callee, "a local out of range")),
            },
            _ => self.piece(callee)?.at(Prec::Atom),
        };
        Ok(Piece::new(format!("{called}({})", printed.join(", ")), Prec::Atom))
    }
}
