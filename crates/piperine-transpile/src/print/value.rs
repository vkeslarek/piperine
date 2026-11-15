//! `ValuePrinter`: the value forms of the IR as Rust expressions, and an IR function as a Rust
//! `fn` generic over `Real`.
//!
//! A real is `R::constant(v)`; arithmetic is Rust's operators on `R`; a comparison of reals
//! compares their values (`a.value() > b.value()`, a literal operand plain), as the
//! interpreter's walk compares the values of its duals; `exp`, `ln`, `sqrt` and `abs` are
//! `Real`'s methods. An `if` and a block print over several lines, one statement each. A local
//! prints under its MSL name, suffixed `_<index>` when another local of its owner shares that
//! name. An environment read (`temperature()`, `gmin()`, `source_scale()`) is a variable of
//! that name: a function that reads one, itself or through a call, takes it as a trailing
//! parameter, and a call passes it on. A probe, a `limit` or an `ac_stim` site prints only
//! inside an `analog` block, as the variables its `Reads` name; elsewhere it, like a
//! contribution, has no lowering here and is `Error::Unsupported` with its form, path and
//! origin.

use std::collections::BTreeSet;

use piperine_ir::{
    BinaryOp, Body, Env, Expr, ExprId, FunctionId, Ir, Local, MathFn, Origin, Param, Stmt, StmtId, Terminal, UnaryOp,
    ValueType,
};

use crate::Error;
use crate::print::Text;

/// Binding strength of a printed expression: an operand weaker than its place is parenthesized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    /// An `if`, a block or a closure: parenthesized wherever it is an operand.
    Low,
    Or,
    And,
    Compare,
    Add,
    Mul,
    Unary,
    Atom,
}

/// A printed expression and how strongly it binds.
struct Piece {
    text: String,
    prec: Prec,
}

impl Piece {
    fn new(text: String, prec: Prec) -> Piece {
        Piece { text, prec }
    }

    /// The text, parenthesized when it binds weaker than `at_least`.
    fn at(self, at_least: Prec) -> String {
        if self.prec < at_least { format!("({})", self.text) } else { self.text }
    }
}

/// Prints the value code of one owner: a function, or a body's `analog` block.
#[derive(Debug)]
pub struct ValuePrinter<'a> {
    ir: &'a Ir,
    /// The owner's IR path, prefixed to an expression's id in an error: `fn pnjlim`.
    path: String,
    /// The printed name of each local of the owner, by index.
    locals: Vec<String>,
    /// The printed name of each param of the owner's body, by index; empty for a function.
    params: Vec<String>,
    /// The owner's origin: where an error with no expression of its own points.
    origin: Origin,
    /// How the analog read forms print; `None` outside an `analog` block.
    analog: Option<Reads>,
}

/// The variables an `analog` block's read forms print as: a probe as the difference of its
/// nodes' variables, a `limit` or `ac_stim` site as the variable of its held value.
#[derive(Clone, Debug, Default)]
pub struct Reads {
    /// By body node.
    pub nodes: Vec<String>,
    /// By `limit` site.
    pub limits: Vec<String>,
    /// By `ac_stim` site.
    pub stims: Vec<String>,
}

impl<'a> ValuePrinter<'a> {
    /// The printer of `function`'s code.
    pub fn of_function(ir: &'a Ir, function: FunctionId) -> ValuePrinter<'a> {
        let f = ir.function(function);
        let (path, origin) = (format!("fn {}", f.name()), f.origin().clone());
        ValuePrinter { ir, path, locals: Self::names(f.locals()), params: Vec::new(), origin, analog: None }
    }

    /// The printer of the code of the body named `body`, with its locals and params.
    pub fn of_body(ir: &'a Ir, body: Body<'a>) -> ValuePrinter<'a> {
        let params = body.params().iter().map(|p: &Param| Text::ident(&p.name)).collect();
        let (path, origin) = (format!("body {}", body.name()), body.origin().clone());
        ValuePrinter { ir, path, locals: Self::names(body.locals()), params, origin, analog: None }
    }

    /// This printer with the analog read forms printing as `reads` names them.
    pub fn with_reads(self, reads: Reads) -> ValuePrinter<'a> {
        ValuePrinter { analog: Some(reads), ..self }
    }

    /// The printed name of local `i`.
    pub fn local(&self, i: u32) -> Option<&str> {
        self.locals.get(i as usize).map(String::as_str)
    }

    /// `plus − minus` as the probe reads it: a node's variable, `R::constant(0.0)` for ground
    /// on the left, nothing subtracted for ground on the right (`x − 0` is `x`, bit for bit).
    pub fn probe(&self, plus: Terminal, minus: Terminal) -> Option<String> {
        Some(self.probe_piece(plus, minus)?.text)
    }

    fn probe_piece(&self, plus: Terminal, minus: Terminal) -> Option<Piece> {
        let reads = self.analog.as_ref()?;
        let node = |t: Terminal| match t {
            Terminal::Node(i) => reads.nodes.get(i as usize).cloned(),
            Terminal::Ground => Some("R::constant(0.0)".to_string()),
        };
        let (p, m) = (node(plus)?, node(minus)?);
        Some(match minus {
            Terminal::Ground if plus != Terminal::Ground => Piece::new(p, Prec::Atom),
            _ => Piece::new(format!("{p} - {m}"), Prec::Add),
        })
    }

    /// `function` as one Rust `fn` generic over `Real`, under its comment: its params, then one
    /// trailing param per environment read it makes.
    pub fn function(ir: &'a Ir, function: FunctionId) -> Result<String, Error> {
        let f = ir.function(function);
        let printer = Self::of_function(ir, function);
        let mut params = Vec::new();
        for (i, local) in f.params().iter().enumerate() {
            let ty = printer.type_of(&local.ty, &format!("{} param {}", printer.path, local.name))?;
            params.push(format!("{}: {ty}", Self::unused_prefix(ir, f.body(), i as u32, &printer.locals[i])));
        }
        for env in Self::env_reads(ir, function) {
            params.push(format!("{}: R", Self::env_name(env)));
        }
        let ret = printer.type_of(f.ret(), &format!("{} return", printer.path))?;
        let mut t = Text::new();
        t.comment(&printer.path, f.origin());
        t.line(&format!("fn {}<R: Real>({}) -> {ret} {{", Text::ident(f.name()), params.join(", ")));
        let body = printer.arm(f.body())?;
        t.indented(|t| t.line(&body));
        t.line("}");
        Ok(t.finish())
    }

    /// `e` as a Rust expression; an `if` or a block spans several lines, each inner line indented
    /// by four spaces relative to the first.
    pub fn expr(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.text)
    }

    /// `-e`, `e` parenthesized unless it is an atom.
    pub fn negated(&self, e: ExprId) -> Result<String, Error> {
        Ok(format!("-{}", self.piece(e)?.at(Prec::Atom)))
    }

    /// `e` as the right operand of a `+`: parenthesized when it binds no stronger than `+`.
    pub fn right_of_add(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.at(Prec::Mul))
    }

    /// The environment reads of `function`, its own and those of every function it calls, in
    /// `Env` order.
    pub fn env_reads(ir: &Ir, function: FunctionId) -> Vec<Env> {
        let mut found = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut functions = vec![function];
        while let Some(f) = functions.pop() {
            if !seen.insert(f.index()) {
                continue;
            }
            let mut exprs = vec![ir.function(f).body()];
            while let Some(e) = exprs.pop() {
                match ir.expr(e) {
                    Expr::Env(env) => {
                        found.insert(*env);
                    }
                    Expr::Call { function, args } => {
                        functions.push(*function);
                        exprs.extend(args);
                    }
                    other => Self::children(ir, other, &mut exprs),
                }
            }
        }
        found.into_iter().collect()
    }

    /// The variable an environment read is: `temperature`, `gmin`, `source_scale`.
    pub fn env_name(env: Env) -> &'static str {
        match env {
            Env::Temperature => "temperature",
            Env::Gmin => "gmin",
            Env::SourceScale => "source_scale",
        }
    }

    /// `name`, or `_name` when `body` never reads local `local`: a parameter the code ignores.
    fn unused_prefix(ir: &Ir, body: ExprId, local: u32, name: &str) -> String {
        let mut exprs = vec![body];
        while let Some(e) = exprs.pop() {
            match ir.expr(e) {
                Expr::Local(i) if *i == local => return name.to_string(),
                other => Self::children(ir, other, &mut exprs),
            }
        }
        format!("_{name}")
    }

    /// The expressions `expr` holds, statements included.
    pub(crate) fn children(ir: &Ir, expr: &Expr, out: &mut Vec<ExprId>) {
        match expr {
            Expr::Unary { expr, .. } => out.push(*expr),
            Expr::Binary { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
            Expr::If { cond, then, otherwise } => out.extend([*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => {
                for s in stmts {
                    match ir.stmt(*s) {
                        Stmt::Let { init, .. } => out.push(*init),
                        Stmt::Expr(e) => out.push(*e),
                        Stmt::Contribute { addends, .. } => out.extend(addends.iter().map(|a| a.value)),
                    }
                }
                out.extend(value);
            }
            Expr::Call { args, .. } | Expr::Math { args, .. } => out.extend(args),
            Expr::Closure { body, .. } => out.push(*body),
            Expr::Limit { x, f, seed, .. } => out.extend([*x, *f].into_iter().chain(*seed)),
            Expr::Stim { mag, phase, .. } => out.extend([*mag].into_iter().chain(*phase)),
            Expr::Real(_)
            | Expr::Bool(_)
            | Expr::Unit
            | Expr::Local(_)
            | Expr::Param(_)
            | Expr::Probe { .. }
            | Expr::Env(_) => {}
        }
    }

    /// Each local's MSL name, raw where Rust reserves it, suffixed `_<index>` when another local
    /// of the same owner has the same name.
    fn names(locals: &[Local]) -> Vec<String> {
        locals
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let shared = locals.iter().filter(|o| o.name == l.name).count() > 1;
                if shared { format!("{}_{i}", l.name) } else { Text::ident(&l.name) }
            })
            .collect()
    }

    /// The Rust type of a value type: `R`, `bool`, `()`.
    fn type_of(&self, ty: &ValueType, path: &str) -> Result<&'static str, Error> {
        match ty {
            ValueType::F64 => Ok("R"),
            ValueType::Bool => Ok("bool"),
            ValueType::Unit => Ok("()"),
            ValueType::Fn(..) => Err(Error::Unsupported {
                form: "a function-typed value".into(),
                path: path.to_string(),
                origin: self.origin.clone(),
            }),
        }
    }

    fn unsupported(&self, e: ExprId, form: &str) -> Error {
        Error::Unsupported {
            form: form.to_string(),
            path: format!("{} e{}", self.path, e.index()),
            origin: self.ir.expr_origin(e).clone(),
        }
    }

    fn piece(&self, e: ExprId) -> Result<Piece, Error> {
        let atom = |text: String| Ok(Piece::new(text, Prec::Atom));
        match self.ir.expr(e) {
            Expr::Real(r) => atom(format!("R::constant({})", Text::real(r.get()))),
            Expr::Bool(b) => atom(b.to_string()),
            Expr::Unit => atom("()".to_string()),
            Expr::Local(i) => match self.locals.get(*i as usize) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "a local out of range")),
            },
            Expr::Param(i) => match self.params.get(*i as usize) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "a param read outside a body")),
            },
            Expr::Env(env) => atom(Self::env_name(*env).to_string()),
            Expr::Unary { op, expr } => {
                let operand = self.piece(*expr)?.at(Prec::Atom);
                let sign = if *op == UnaryOp::Neg { "-" } else { "!" };
                Ok(Piece::new(format!("{sign}{operand}"), Prec::Unary))
            }
            Expr::Binary { op, lhs, rhs } => self.binary(*op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => Ok(Piece::new(self.if_else(*cond, *then, *otherwise)?, Prec::Low)),
            Expr::Block { stmts, value } => {
                let lines = self.block_lines(stmts, *value)?;
                let text = if lines.is_empty() { "{}".to_string() } else { format!("{{\n{}\n}}", Self::indent(&lines)) };
                Ok(Piece::new(text, Prec::Low))
            }
            Expr::Call { function, args } => {
                let mut printed = Vec::with_capacity(args.len());
                for a in args {
                    printed.push(self.expr(*a)?);
                }
                printed.extend(Self::env_reads(self.ir, *function).into_iter().map(|env| Self::env_name(env).to_string()));
                atom(format!("{}({})", Text::ident(self.ir.function(*function).name()), printed.join(", ")))
            }
            Expr::Math { f, args } => {
                let [arg] = args.as_slice() else {
                    return Err(self.unsupported(e, "a math function with other than one argument"));
                };
                let name = match f {
                    MathFn::Exp => "exp",
                    MathFn::Ln => "ln",
                    MathFn::Sqrt => "sqrt",
                    MathFn::Abs => "abs",
                };
                atom(format!("{}.{name}()", self.piece(*arg)?.at(Prec::Atom)))
            }
            Expr::Closure { params, body } => {
                let mut names = Vec::with_capacity(params.len());
                for p in params {
                    match self.locals.get(*p as usize) {
                        Some(name) => names.push(Self::unused_prefix(self.ir, *body, *p, name)),
                        None => return Err(self.unsupported(e, "a closure parameter out of range")),
                    }
                }
                Ok(Piece::new(format!("|{}| {}", names.join(", "), self.expr(*body)?), Prec::Low))
            }
            Expr::Probe { plus, minus } => self.probe_piece(*plus, *minus).ok_or_else(|| self.unsupported(e, "probe")),
            Expr::Limit { site, .. } => match self.analog.as_ref().and_then(|r| r.limits.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "limit")),
            },
            Expr::Stim { site, .. } => match self.analog.as_ref().and_then(|r| r.stims.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "ac_stim")),
            },
        }
    }

    fn binary(&self, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Piece, Error> {
        let (symbol, prec) = match op {
            BinaryOp::Add => ("+", Prec::Add),
            BinaryOp::Sub => ("-", Prec::Add),
            BinaryOp::Mul => ("*", Prec::Mul),
            BinaryOp::Div => ("/", Prec::Mul),
            BinaryOp::Lt => ("<", Prec::Compare),
            BinaryOp::Le => ("<=", Prec::Compare),
            BinaryOp::Gt => (">", Prec::Compare),
            BinaryOp::Ge => (">=", Prec::Compare),
            BinaryOp::Eq => ("==", Prec::Compare),
            BinaryOp::Ne => ("!=", Prec::Compare),
            BinaryOp::And => ("&&", Prec::And),
            BinaryOp::Or => ("||", Prec::Or),
        };
        let (l, r) = if prec == Prec::Compare {
            (self.compared(lhs)?, self.compared(rhs)?)
        } else {
            // left-associative: an equal-strength operand binds on the left, not on the right
            (self.piece(lhs)?.at(prec), self.piece(rhs)?.at(Self::stronger(prec)))
        };
        Ok(Piece::new(format!("{l} {symbol} {r}"), prec))
    }

    /// An operand of a comparison: a real compares by value (a literal plain), anything else as
    /// it is; Rust's comparisons do not chain, so a weaker operand is parenthesized.
    fn compared(&self, e: ExprId) -> Result<String, Error> {
        if *self.ir.expr_type(e) != ValueType::F64 {
            return Ok(self.piece(e)?.at(Self::stronger(Prec::Compare)));
        }
        match self.ir.expr(e) {
            Expr::Real(r) => Ok(Text::real(r.get())),
            _ => Ok(format!("{}.value()", self.piece(e)?.at(Prec::Atom))),
        }
    }

    fn stronger(prec: Prec) -> Prec {
        match prec {
            Prec::Low => Prec::Or,
            Prec::Or => Prec::And,
            Prec::And => Prec::Compare,
            Prec::Compare => Prec::Add,
            Prec::Add => Prec::Mul,
            Prec::Mul => Prec::Unary,
            Prec::Unary | Prec::Atom => Prec::Atom,
        }
    }

    /// `if c {` arm `} else {` arm `}`; an `else` arm that is itself an `if` chains as `else if`.
    fn if_else(&self, cond: ExprId, then: ExprId, otherwise: ExprId) -> Result<String, Error> {
        let mut text = format!("if {} {{\n{}\n}} else ", self.piece(cond)?.at(Prec::Or), Self::indent(&self.arm(then)?));
        match self.ir.expr(otherwise) {
            Expr::If { cond, then, otherwise } => text.push_str(&self.if_else(*cond, *then, *otherwise)?),
            _ => text.push_str(&format!("{{\n{}\n}}", Self::indent(&self.arm(otherwise)?))),
        }
        Ok(text)
    }

    /// The inside of a pair of braces: a block's lines without its braces, any other expression
    /// as it is.
    fn arm(&self, e: ExprId) -> Result<String, Error> {
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => self.block_lines(stmts, *value),
            _ => self.expr(e),
        }
    }

    /// A block's statements, one per line, then its value.
    fn block_lines(&self, stmts: &[StmtId], value: Option<ExprId>) -> Result<String, Error> {
        let mut lines = Vec::with_capacity(stmts.len() + 1);
        for s in stmts {
            lines.push(self.stmt(*s)?);
        }
        if let Some(v) = value {
            lines.push(self.expr(v)?);
        }
        Ok(lines.join("\n"))
    }

    /// `let name = init;` or `expr;`.
    pub fn stmt(&self, s: StmtId) -> Result<String, Error> {
        match self.ir.stmt(s) {
            Stmt::Let { local, init } => {
                let Some(name) = self.locals.get(*local as usize) else {
                    return Err(self.unsupported(*init, "a let of a local out of range"));
                };
                Ok(format!("let {name} = {};", self.expr(*init)?))
            }
            Stmt::Expr(e) => Ok(format!("{};", self.expr(*e)?)),
            Stmt::Contribute { addends, .. } => Err(Error::Unsupported {
                form: "contribution".into(),
                path: format!("{} s{}", self.path, s.index()),
                origin: addends.first().map_or_else(|| self.origin.clone(), |a| self.ir.expr_origin(a.value).clone()),
            }),
        }
    }

    /// Every line of `text` four spaces deeper; empty lines stay empty.
    fn indent(text: &str) -> String {
        text.lines().map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect::<Vec<_>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{Expr, Ir, IrBuilder, Origin, Stmt};

    use super::ValuePrinter;
    use crate::Error;
    use crate::devices::Devices;

    /// The resistor, the voltage source and the diode (with `vt` and `pnjlim`) in one `Ir`.
    fn library() -> Ir {
        let mut b = IrBuilder::new();
        let r = Devices::resistor(&mut b);
        Devices::vsource(&mut b);
        Devices::diode(&mut b);
        let top = Devices::alone(&mut b, r, "R", 1, vec![1000.0]);
        b.finish(top)
    }

    fn function(ir: &Ir, name: &str) -> String {
        let f = ir.functions().find(|f| f.name() == name).unwrap_or_else(|| panic!("no fn {name}"));
        ValuePrinter::function(ir, f.id()).unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    #[test]
    fn a_function_prints_as_a_generic_fn() {
        let ir = library();
        // std/analog.msl:10: `pub fn vt() -> f64 { K_B * temperature() / Q }`, the constants
        // folded; `temperature()` becomes a trailing param
        let vt = "\
// fn vt @ analog.msl:10:1
fn vt<R: Real>(temperature: R) -> R {
    R::constant(1.380649e-23) * temperature / R::constant(1.602176634e-19)
}
";
        assert_eq!(function(&ir, "vt"), vt);

        // std/analog.msl:21-31, line by line; its two `let arg` are distinct locals of one owner
        let pnjlim = "\
// fn pnjlim @ analog.msl:21:1
fn pnjlim<R: Real>(new: R, old: R, nvt: R, vcrit: R) -> R {
    if new.value() > vcrit.value() && (new - old).abs().value() > (R::constant(2.0) * nvt).value() {
        if old.value() > 0.0 {
            let arg_4 = (new - old) / nvt;
            if arg_4.value() > 0.0 {
                old + nvt * (R::constant(2.0) + (arg_4 - R::constant(2.0)).ln())
            } else {
                old - nvt * (R::constant(2.0) + (R::constant(2.0) - arg_4).ln())
            }
        } else {
            nvt * (new / nvt).ln()
        }
    } else if new.value() < 0.0 {
        let arg_5 = if old.value() > 0.0 {
            -old - R::constant(1.0)
        } else {
            R::constant(2.0) * old - R::constant(1.0)
        };
        if new.value() < arg_5.value() {
            arg_5
        } else {
            new
        }
    } else {
        new
    }
}
";
        assert_eq!(function(&ir, "pnjlim"), pnjlim);
    }

    /// The value of the first addend of `body`'s only contribution.
    fn contributed<'a>(ir: &'a Ir, body: &str) -> (piperine_ir::Body<'a>, piperine_ir::ExprId) {
        let body = ir.bodies().find(|b| b.name() == body).unwrap_or_else(|| panic!("no body {body}"));
        let value = body
            .analog()
            .unwrap_or_default()
            .iter()
            .find_map(|s| match ir.stmt(*s) {
                Stmt::Contribute { addends, .. } => Some(addends[0].value),
                _ => None,
            })
            .expect("a contribution");
        (body, value)
    }

    #[test]
    fn an_unsupported_form_is_named_with_path_and_origin() {
        let ir = library();
        let at = |line, col| Origin { file: "devices.msl".into(), line, col };

        // `i(p, n) +> v(p, n) / r`: the probe `v(p, n)` (devices.msl:8) has no value lowering
        let (resistor, value) = contributed(&ir, "Resistor");
        let Expr::Binary { lhs: probe, .. } = ir.expr(value) else { panic!("v(p, n) / r") };
        let err = ValuePrinter::of_body(&ir, resistor).expr(value).expect_err("a probe is not a value form");
        assert_eq!(
            err,
            Error::Unsupported { form: "probe".into(), path: format!("body Resistor e{}", probe.index()), origin: at(8, 32) }
        );

        // `v(p, n) +> dc * source_scale() + ac_stim(ac)`: the stimulus (devices.msl:21) neither
        let (vsource, value) = contributed(&ir, "Vsource");
        let Expr::Binary { rhs: stim, .. } = ir.expr(value) else { panic!("dc * source_scale() + ac_stim(ac)") };
        let err = ValuePrinter::of_body(&ir, vsource).expr(value).expect_err("ac_stim is not a value form");
        assert_eq!(
            err,
            Error::Unsupported { form: "ac_stim".into(), path: format!("body Vsource e{}", stim.index()), origin: at(21, 30) }
        );
        // the message names the three
        let text = err.to_string();
        assert!(text.contains("ac_stim") && text.contains("body Vsource") && text.contains("devices.msl:21:30"), "{text}");
    }
}
