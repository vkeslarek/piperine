//! `ValuePrinter`: the value forms of the IR as Rust expressions, and an IR function as a Rust
//! `fn` generic over `Real`.
//!
//! A real is `R::constant(v)`; arithmetic is Rust's operators on `R`; a comparison of reals
//! compares their values (`a.value() > b.value()`, a literal operand plain), as the
//! interpreter's walk compares the values of its duals; `exp`, `ln`, `sqrt` and `abs` are
//! `Real`'s methods. An `if` and a block print over several lines, one statement each. A local
//! prints under its MSL name (`Text::ident`), as the generated `<name>_<index>_` when another
//! local of its owner shares that name; a parameter the code never reads prints as `_`. An
//! environment read (`temperature()`, `gmin()`, `source_scale()`) is a generated variable of
//! that name (`temperature_`): a function that reads one, itself or through a call, takes it as
//! a trailing parameter, and a call passes it on. A probe, a `limit` or an `ac_stim` site prints only
//! inside an `analog` block, as the variables its `Reads` name; elsewhere it, like a
//! contribution, has no lowering here and is `Error::Unsupported` with its form, path and
//! origin.
//!
//! Phase B forms. A value that may be unknown is `rt::Flagged<T>`, an integer `rt::UInt<N>` or
//! `rt::Int<N>`, all named by their `rt::` path in the text. An operand that is not unknown is
//! lifted (`rt::Flagged::known(x)`) where the other operand, or the place it flows to, may be.
//! An `if` on an unknown condition is `if c.decide("<origin>")? { … }`, so a function that holds
//! one returns `Result<T, Fault>` and a call to it ends in `?`; analog code has no `?` and refuses
//! such an `if`. In the digital printers a real is a plain `f64`; in an `analog` block and in an
//! IR function it is `R`.

use std::cell::RefCell;
use std::collections::BTreeSet;

use piperine_ir::{
    BinaryOp, Body, EmitField, Env, Expr, ExprId, FunctionId, Ir, Local, MathFn, Method, Origin, Param, Stmt, StmtId, Terminal,
    UnaryOp, ValueType,
};

use piperine_plan::StatementPlan;

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
    BitOr,
    BitXor,
    BitAnd,
    Shift,
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

/// Prints the value code of one owner: a function, a body's `analog` block, or the code of a
/// drive, an event or a reaction.
#[derive(Debug)]
pub struct ValuePrinter<'a> {
    ir: &'a Ir,
    /// The owner's IR path, prefixed to an expression's id in an error: `fn pnjlim`.
    path: String,
    /// The printed name of each local of the owner, by index.
    locals: Vec<String>,
    /// The type of each local of the owner, by index.
    local_types: Vec<ValueType>,
    /// The printed name of each param of the owner's body, by index; empty for a function.
    params: Vec<String>,
    /// The type each carrier of the owner's body holds, by index; empty for a function.
    carriers: Vec<ValueType>,
    /// The owner's origin: where an error with no expression of its own points.
    origin: Origin,
    /// How the analog read forms print; `None` outside an `analog` block.
    analog: Option<Reads>,
    /// A real is a plain `f64` (the digital printers) rather than `R` (an `analog` block, an IR
    /// function).
    plain: bool,
    /// Whether the code may use `?`: it runs in a function returning `Result<_, Fault>`.
    faults: bool,
    /// Whether a commit has a place to go: the code of a reaction.
    commits: bool,
    /// The carrier and `min_after` of each commit printed so far, in printing order: the order
    /// of the reaction's statements in the shape.
    committed: RefCell<Vec<StatementPlan>>,
    /// The names of the reads (params, nodes, carriers, the environment) printed so far: the
    /// variables the code uses, so a function binds no read it leaves alone.
    used: RefCell<BTreeSet<String>>,
}

/// The variables an `analog` block's read forms print as: a probe as the difference of its
/// nodes' variables, a `limit` or `ac_stim` site as the variable of its held value, a carrier
/// read as the variable the carrier's pin was decoded into.
#[derive(Clone, Debug, Default)]
pub struct Reads {
    /// By body node.
    pub nodes: Vec<String>,
    /// By `limit` site.
    pub limits: Vec<String>,
    /// By `ac_stim` site.
    pub stims: Vec<String>,
    /// By body carrier.
    pub carriers: Vec<String>,
}

impl<'a> ValuePrinter<'a> {
    /// The printer of `function`'s code.
    pub fn of_function(ir: &'a Ir, function: FunctionId) -> ValuePrinter<'a> {
        let f = ir.function(function);
        let (path, origin) = (format!("fn {}", f.name()), f.origin().clone());
        ValuePrinter {
            ir,
            path,
            locals: Self::names(f.locals(), &[]),
            local_types: f.locals().iter().map(|l| l.ty.clone()).collect(),
            params: Vec::new(),
            carriers: Vec::new(),
            origin,
            analog: None,
            plain: false,
            faults: true,
            commits: false,
            committed: RefCell::new(Vec::new()),
            used: RefCell::new(BTreeSet::new()),
        }
    }

    /// The printer of the code of the body named `body`, with its locals and params: its
    /// `analog` block, where a real is `R` and there is no `?`.
    pub fn of_body(ir: &'a Ir, body: Body<'a>) -> ValuePrinter<'a> {
        let params: Vec<String> = body.params().iter().map(|p: &Param| Text::ident(&p.name)).collect();
        let (path, origin) = (format!("body {}", body.name()), body.origin().clone());
        let reserved = Self::reserved(&body, &params);
        ValuePrinter {
            ir,
            path,
            locals: Self::names(body.locals(), &reserved),
            local_types: body.locals().iter().map(|l| l.ty.clone()).collect(),
            params,
            carriers: body.carriers().iter().map(|c| c.ty.clone()).collect(),
            origin,
            analog: None,
            plain: false,
            faults: false,
            commits: false,
            committed: RefCell::new(Vec::new()),
            used: RefCell::new(BTreeSet::new()),
        }
    }

    /// The printer of the code of a drive or an event of `body`: its locals are the body's, a
    /// real is a plain `f64`, and an unknown `if` is a `Fault`.
    pub fn of_digital(ir: &'a Ir, body: Body<'a>) -> ValuePrinter<'a> {
        ValuePrinter { plain: true, faults: true, ..Self::of_body(ir, body) }
    }

    /// The printer of reaction `k` of `body`: its own locals, plain reals, `?`, and commits.
    pub fn of_reaction(ir: &'a Ir, body: Body<'a>, k: usize) -> ValuePrinter<'a> {
        let base = Self::of_digital(ir, body);
        let Some(reaction) = body.reactions().get(k) else { return base };
        let params: Vec<String> = body.params().iter().map(|p: &Param| Text::ident(&p.name)).collect();
        let reserved = Self::reserved(&body, &params);
        ValuePrinter {
            path: format!("body {} reaction {k}", body.name()),
            locals: Self::names(&reaction.locals, &reserved),
            local_types: reaction.locals.iter().map(|l| l.ty.clone()).collect(),
            origin: reaction.origin.clone(),
            commits: true,
            ..base
        }
    }

    /// The names a local of `body` must not print as: its params, nodes and carriers.
    fn reserved(body: &Body<'_>, params: &[String]) -> Vec<String> {
        params
            .iter()
            .cloned()
            .chain(body.nodes().iter().map(|n| Text::ident(&n.name)))
            .chain(body.carriers().iter().map(|c| Text::ident(&c.name)))
            .collect()
    }

    /// This printer with the analog read forms printing as `reads` names them.
    pub fn with_reads(self, reads: Reads) -> ValuePrinter<'a> {
        ValuePrinter { analog: Some(reads), ..self }
    }

    /// The carrier and `min_after` of each commit printed so far, in printing order.
    pub fn committed(&self) -> Vec<StatementPlan> {
        self.committed.borrow().clone()
    }

    /// Whether the code printed so far reads the variable `name` (a param, node, carrier or
    /// environment read).
    pub fn uses(&self, name: &str) -> bool {
        self.used.borrow().contains(name)
    }

    /// The Rust type a pin of `ty` is decoded as: `bool`, `f64`, `rt::UInt<N>`, `rt::Int<N>`.
    pub fn type_of(&self, ty: &ValueType) -> Result<String, Error> {
        self.rust_type(ty, &self.path)
    }

    /// The Rust type a pin of `ty` holds inside `rt::Flagged<…>`: a real is the plain `f64`
    /// whatever the code's own real is.
    pub fn pin_type(&self, ty: &ValueType) -> Result<String, Error> {
        match ty {
            ValueType::F64 => Ok("f64".to_string()),
            other => self.rust_type(other, &self.path),
        }
    }

    /// `rt::UInt::<N>::new(0)` or `rt::Int::<N>::new(0)`: the value of an integer param no one
    /// wrote.
    pub fn zero(ty: &ValueType) -> String {
        match ty {
            ValueType::UInt(n) => format!("rt::UInt::<{n}>::new(0)"),
            ValueType::Int(n) => format!("rt::Int::<{n}>::new(0)"),
            _ => "0".to_string(),
        }
    }

    /// Notes that the code reads the variable `name`, and gives it back.
    fn read(&self, name: &str) -> String {
        self.used.borrow_mut().insert(name.to_string());
        name.to_string()
    }

    /// The printed name of local `i`.
    pub fn local(&self, i: u32) -> Option<&str> {
        self.locals.get(i as usize).map(String::as_str)
    }

    /// `plus − minus` as the probe reads it: a node's variable, `R::constant(0.0)` (`0.0` in
    /// plain code) for ground on the left, nothing subtracted for ground on the right (`x − 0`
    /// is `x`, bit for bit).
    pub fn probe(&self, plus: Terminal, minus: Terminal) -> Option<String> {
        Some(self.probe_piece(plus, minus)?.text)
    }

    fn probe_piece(&self, plus: Terminal, minus: Terminal) -> Option<Piece> {
        let reads = self.analog.as_ref()?;
        let ground = if self.plain { "0.0" } else { "R::constant(0.0)" };
        let node = |t: Terminal| match t {
            Terminal::Node(i) => reads.nodes.get(i as usize).map(|n| self.read(n)),
            Terminal::Ground => Some(ground.to_string()),
        };
        let (p, m) = (node(plus)?, node(minus)?);
        Some(match minus {
            Terminal::Ground if plus != Terminal::Ground => Piece::new(p, Prec::Atom),
            _ => Piece::new(format!("{p} - {m}"), Prec::Add),
        })
    }

    /// `function` as one Rust `fn` under its comment: its params, then one trailing param per
    /// environment read it makes. It is generic over `Real` when a real appears in it, returns
    /// `Result<_, Fault>` when it holds an `if` on an unknown condition, directly or through a
    /// call.
    pub fn function(ir: &'a Ir, function: FunctionId) -> Result<String, Error> {
        let f = ir.function(function);
        let printer = Self::of_function(ir, function);
        let mut params = Vec::new();
        for (i, local) in f.params().iter().enumerate() {
            let ty = printer.rust_type(&local.ty, &format!("{} param {}", printer.path, local.name))?;
            params.push(format!("{}: {ty}", Self::unused_prefix(ir, f.body(), i as u32, &printer.locals[i])));
        }
        let envs = Self::env_reads(ir, function);
        for env in &envs {
            params.push(format!("{}: R", Self::env_name(*env)));
        }
        let ret = printer.rust_type(f.ret(), &format!("{} return", printer.path))?;
        let faulting = Self::faults(ir, function);
        let generic = if Self::is_generic(ir, function) { "<R: Real>" } else { "" };
        let returns = if faulting { format!("Result<{ret}, Fault>") } else { ret };
        let mut t = Text::new();
        t.comment(&printer.path, f.origin());
        t.line(&format!("fn {}{generic}({}) -> {returns} {{", Text::ident(f.name()), params.join(", ")));
        let body = printer.function_body(f.body(), f.ret(), faulting)?;
        t.indented(|t| t.line(&body));
        t.line("}");
        Ok(t.finish())
    }

    /// Whether `function` is generic over `Real`: a real is among its params, its return, its
    /// locals, or it reads the environment.
    pub fn is_generic(ir: &Ir, function: FunctionId) -> bool {
        let f = ir.function(function);
        let real = |ty: &ValueType| *ty == ValueType::F64;
        f.params().iter().chain(f.locals()).any(|l| real(&l.ty)) || real(f.ret()) || !Self::env_reads(ir, function).is_empty()
    }

    /// Whether `function` holds an `if` on an unknown condition, directly or through a call.
    pub fn faults(ir: &Ir, function: FunctionId) -> bool {
        let mut seen = BTreeSet::new();
        let mut functions = vec![function];
        while let Some(f) = functions.pop() {
            if !seen.insert(f.index()) {
                continue;
            }
            let mut exprs = vec![ir.function(f).body()];
            while let Some(e) = exprs.pop() {
                match ir.expr(e) {
                    Expr::If { cond, .. } if Self::is_flagged(ir.expr_type(*cond)) => return true,
                    Expr::Call { function, args } => {
                        functions.push(*function);
                        exprs.extend(args);
                    }
                    other => Self::children(ir, other, &mut exprs),
                }
            }
        }
        false
    }

    /// The body of a function: its statements, then its value, wrapped in `Ok(…)` when the
    /// function returns a `Result`.
    fn function_body(&self, e: ExprId, ret: &ValueType, faulting: bool) -> Result<String, Error> {
        let wrap = |value: String| if faulting { format!("Ok({value})") } else { value };
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => {
                let mut lines = Vec::with_capacity(stmts.len() + 1);
                for s in stmts {
                    lines.push(self.stmt(*s)?);
                }
                match value {
                    Some(v) => lines.push(wrap(self.typed(*v, ret)?.text)),
                    None if faulting => lines.push("Ok(())".to_string()),
                    None => {}
                }
                Ok(lines.join("\n"))
            }
            _ => Ok(wrap(self.typed(e, ret)?.text)),
        }
    }

    /// `e` as a Rust expression; an `if` or a block spans several lines, each inner line indented
    /// by four spaces relative to the first.
    pub fn expr(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.text)
    }

    /// `e` as an expression of type `want`: lifted to `rt::Flagged` when `want` may be unknown
    /// and `e` is not.
    pub fn expr_as(&self, e: ExprId, want: &ValueType) -> Result<String, Error> {
        Ok(self.typed(e, want)?.text)
    }

    /// `e`, a `bool`, as an operand of `&&`: a value that may be unknown is decided
    /// (`e.decide("<origin of e>")?`, a fault naming `e` when unknown, RS-11 AC 5).
    pub fn condition(&self, e: ExprId) -> Result<String, Error> {
        if !Self::is_flagged(self.ir.expr_type(e)) {
            return Ok(self.piece(e)?.at(Prec::Compare));
        }
        if !self.faults {
            return Err(self.unsupported(e, "a condition on a value that may be unknown in analog code"));
        }
        Ok(format!("{}.decide({:?})?", self.piece(e)?.at(Prec::Atom), self.at(e)))
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

    /// The generated variable an environment read is: `temperature_`, `gmin_`, `source_scale_`, `time_`, `is_static`.
    pub fn env_name(env: Env) -> &'static str {
        match env {
            Env::Temperature => "temperature_",
            Env::Gmin => "gmin_",
            Env::SourceScale => "source_scale_",
            Env::Time => "time_",
            Env::Static => "is_static",
        }
    }

    /// `name`, or `_` when `body` never reads local `local`: a parameter the code ignores. Not
    /// `_name`, which an MSL name could spell.
    fn unused_prefix(ir: &Ir, body: ExprId, local: u32, name: &str) -> String {
        let mut exprs = vec![body];
        while let Some(e) = exprs.pop() {
            match ir.expr(e) {
                Expr::Local(i) if *i == local => return name.to_string(),
                other => Self::children(ir, other, &mut exprs),
            }
        }
        "_".to_string()
    }

    /// The expressions `expr` holds, statements included.
    pub(crate) fn children(ir: &Ir, expr: &Expr, out: &mut Vec<ExprId>) {
        match expr {
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => out.push(*expr),
            Expr::KnownOr { value, default } => out.extend([*value, *default]),
            Expr::Method { receiver, args, .. } => out.extend([*receiver].into_iter().chain(args.iter().copied())),
            Expr::Binary { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
            Expr::If { cond, then, otherwise } => out.extend([*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => {
                for s in stmts {
                    out.extend(ir.stmt(*s).operands());
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
            | Expr::Env(_)
            | Expr::Int(_)
            | Expr::Unknown
            | Expr::Carrier(_)
            | Expr::Flow { .. }
            | Expr::Previous(_) => {}
        }
    }

    /// Each local's MSL name as `Text::ident` prints it; the generated `<name>_<index>_` when
    /// another local of the same owner, or a name in `reserved` (a param, node or carrier the
    /// code reads), has the same name.
    fn names(locals: &[Local], reserved: &[String]) -> Vec<String> {
        locals
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let shared = locals.iter().filter(|o| o.name == l.name).count() > 1;
                let hides = reserved.contains(&Text::ident(&l.name));
                if shared || hides { Text::generated(&format!("{}_{i}", l.name)) } else { Text::ident(&l.name) }
            })
            .collect()
    }

    /// Whether `ty` may be unknown.
    fn is_flagged(ty: &ValueType) -> bool {
        matches!(ty, ValueType::Flagged(_))
    }

    /// `ty` without its flag.
    fn base(ty: &ValueType) -> &ValueType {
        match ty {
            ValueType::Flagged(inner) => inner,
            other => other,
        }
    }

    /// `ty` as the source spells it: `UInt<65>`, `Int<0>`, `Flagged<UInt<9>>`.
    fn type_name(ty: &ValueType) -> String {
        match ty {
            ValueType::UInt(n) => format!("UInt<{n}>"),
            ValueType::Int(n) => format!("Int<{n}>"),
            ValueType::Flagged(inner) => format!("Flagged<{}>", Self::type_name(inner)),
            other => other.to_string(),
        }
    }

    /// The Rust type of a value type: `R` (`f64` in plain code), `bool`, `()`, `rt::UInt<N>`,
    /// `rt::Int<N>`, `rt::Flagged<T>`. A real inside `Flagged` is always the plain `f64`. A type
    /// Rust lacks is `Error::Unsupported` naming it: an integer wider than 64 bits (or of no
    /// width), a function type. (The IR has no `Float<16>` or `Float<128>`: the only real is
    /// `f64`, so there is nothing of those to refuse.)
    fn rust_type(&self, ty: &ValueType, path: &str) -> Result<String, Error> {
        match ty {
            ValueType::F64 => Ok(if self.plain { "f64" } else { "R" }.to_string()),
            ValueType::Bool => Ok("bool".to_string()),
            ValueType::Unit => Ok("()".to_string()),
            ValueType::UInt(n) | ValueType::Int(n) if !(1..=64).contains(n) => Err(Error::Unsupported {
                form: format!("the type {}, whose width is outside 1..=64", Self::type_name(ty)),
                path: path.to_string(),
                origin: self.origin.clone(),
            }),
            ValueType::UInt(n) => Ok(format!("rt::UInt<{n}>")),
            ValueType::Int(n) => Ok(format!("rt::Int<{n}>")),
            ValueType::Flagged(inner) => {
                let inner = if **inner == ValueType::F64 { "f64".to_string() } else { self.rust_type(inner, path)? };
                Ok(format!("rt::Flagged<{inner}>"))
            }
            ValueType::Fn(..) => Err(Error::Unsupported {
                form: "a function-typed value".into(),
                path: path.to_string(),
                origin: self.origin.clone(),
            }),
        }
    }

    /// `rt::UInt::<N>` or `rt::Int::<N>`: the path an integer's associated functions are called
    /// through; a width outside 1..=64 is refused as `rust_type` refuses it.
    fn integer_path(&self, ty: &ValueType, e: ExprId) -> Result<String, Error> {
        match ty {
            ValueType::UInt(n) | ValueType::Int(n) if (1..=64).contains(n) => {
                Ok(format!("rt::{}::<{n}>", if matches!(ty, ValueType::UInt(_)) { "UInt" } else { "Int" }))
            }
            other => Err(self.unsupported(e, &format!("the type {}, whose width is outside 1..=64", Self::type_name(other)))),
        }
    }

    fn unsupported(&self, e: ExprId, form: &str) -> Error {
        Error::Unsupported {
            form: form.to_string(),
            path: format!("{} e{}", self.path, e.index()),
            origin: self.ir.expr_origin(e).clone(),
        }
    }

    /// `file:line:col` of `e`.
    fn at(&self, e: ExprId) -> String {
        let o = self.ir.expr_origin(e);
        format!("{}:{}:{}", o.file, o.line, o.col)
    }

    /// `e` lifted to `want`: `rt::Flagged::known(e)` when `want` may be unknown and `e` is not.
    fn typed(&self, e: ExprId, want: &ValueType) -> Result<Piece, Error> {
        let piece = self.piece(e)?;
        Ok(self.lift(piece, self.ir.expr_type(e), want))
    }

    /// `piece`, of type `from`, as a value of type `want`.
    fn lift(&self, piece: Piece, from: &ValueType, want: &ValueType) -> Piece {
        if !Self::is_flagged(want) || Self::is_flagged(from) {
            return piece;
        }
        let plain = if *from == ValueType::F64 && !self.plain { format!("{}.value()", piece.at(Prec::Atom)) } else { piece.text };
        Piece::new(format!("rt::Flagged::known({plain})"), Prec::Atom)
    }

    /// `e` as a value that may be unknown.
    fn flagged(&self, e: ExprId) -> Result<Piece, Error> {
        let want = ValueType::Flagged(Box::new(Self::base(self.ir.expr_type(e)).clone()));
        self.typed(e, &want)
    }

    fn piece(&self, e: ExprId) -> Result<Piece, Error> {
        let atom = |text: String| Ok(Piece::new(text, Prec::Atom));
        match self.ir.expr(e) {
            Expr::Real(r) if self.plain => {
                let text = Text::real(r.get());
                let prec = if text.starts_with('-') { Prec::Unary } else { Prec::Atom };
                Ok(Piece::new(text, prec))
            }
            Expr::Real(r) => atom(format!("R::constant({})", Text::real(r.get()))),
            Expr::Bool(b) => atom(b.to_string()),
            Expr::Unit => atom("()".to_string()),
            Expr::Int(v) => atom(format!("{}::new({v})", self.integer_path(self.ir.expr_type(e), e)?)),
            Expr::Local(i) => match self.locals.get(*i as usize) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "a local out of range")),
            },
            Expr::Param(i) => match self.params.get(*i as usize) {
                Some(name) => atom(self.read(name)),
                None => Err(self.unsupported(e, "a param read outside a body")),
            },
            Expr::Env(env) => atom(self.read(Self::env_name(*env))),
            Expr::Carrier(c) => match self.analog.as_ref().and_then(|r| r.carriers.get(*c as usize)) {
                Some(name) => atom(self.read(name)),
                None => Err(self.unsupported(e, "a carrier read")),
            },
            Expr::Unknown => {
                let ty = self.ir.expr_type(e);
                let inner = match Self::base(ty) {
                    ValueType::F64 => "f64".to_string(),
                    other => self.rust_type(other, &format!("{} e{}", self.path, e.index()))?,
                };
                atom(format!("rt::Flagged::<{inner}>::unknown()"))
            }
            Expr::Unary { op, expr } => {
                let operand = self.piece(*expr)?.at(Prec::Atom);
                let sign = if *op == UnaryOp::Neg { "-" } else { "!" };
                Ok(Piece::new(format!("{sign}{operand}"), Prec::Unary))
            }
            Expr::Binary { op, lhs, rhs } => self.binary(e, *op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => Ok(Piece::new(self.if_else(e, *cond, *then, *otherwise)?, Prec::Low)),
            Expr::Block { stmts, value } => {
                let ty = self.ir.expr_type(e);
                let lines = self.block_lines(stmts, *value, ty)?;
                let text = if lines.is_empty() { "{}".to_string() } else { format!("{{\n{}\n}}", Self::indent(&lines)) };
                Ok(Piece::new(text, Prec::Low))
            }
            Expr::Call { function, args } => {
                let params = self.ir.function(*function).params();
                let mut printed = Vec::with_capacity(args.len());
                for (i, a) in args.iter().enumerate() {
                    printed.push(match params.get(i) {
                        Some(p) => self.expr_as(*a, &p.ty)?,
                        None => self.expr(*a)?,
                    });
                }
                printed.extend(Self::env_reads(self.ir, *function).into_iter().map(|env| self.read(Self::env_name(env))));
                let call = format!("{}({})", Text::ident(self.ir.function(*function).name()), printed.join(", "));
                if !Self::faults(self.ir, *function) {
                    return atom(call);
                }
                if !self.faults {
                    return Err(self.unsupported(e, "a call that may fault (an `if` on an unknown condition) in analog code"));
                }
                atom(format!("{call}?"))
            }
            Expr::Math { f, args } => {
                let [arg] = args.as_slice() else {
                    return Err(self.unsupported(e, "a math function with other than one argument"));
                };
                if Self::is_flagged(self.ir.expr_type(*arg)) {
                    return Err(self.unsupported(e, "a math function of a value that may be unknown"));
                }
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
            Expr::Flow { .. } => Err(self.unsupported(e, "the flow of a potential branch")),
            Expr::Previous(_) => Err(self.unsupported(e, "the previous value of a carrier")),
            Expr::Limit { site, .. } => match self.analog.as_ref().and_then(|r| r.limits.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "limit")),
            },
            Expr::Stim { site, .. } => match self.analog.as_ref().and_then(|r| r.stims.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "ac_stim")),
            },
            Expr::Cast { expr, to } => self.cast(e, *expr, to),
            Expr::Method { method, receiver, args } => self.method(e, *method, *receiver, args),
            Expr::KnownOr { value, default } => {
                if !Self::is_flagged(self.ir.expr_type(*value)) {
                    return self.piece(*value);
                }
                let default_type = self.ir.expr_type(*default);
                let d = self.piece(*default)?;
                if *default_type == ValueType::F64 && !self.plain {
                    // the default is an `R`; the value is a plain `f64`
                    atom(format!("R::constant({}.known_or({}.value()))", self.piece(*value)?.at(Prec::Atom), d.at(Prec::Atom)))
                } else {
                    atom(format!("{}.known_or({})", self.piece(*value)?.at(Prec::Atom), d.text))
                }
            }
            Expr::IsKnown(value) => {
                if Self::is_flagged(self.ir.expr_type(*value)) {
                    atom(format!("{}.is_known()", self.piece(*value)?.at(Prec::Atom)))
                } else {
                    atom("true".to_string())
                }
            }
            Expr::High(value) => {
                if Self::is_flagged(self.ir.expr_type(*value)) {
                    atom(format!("{}.high()", self.piece(*value)?.at(Prec::Atom)))
                } else {
                    self.piece(*value)
                }
            }
        }
    }

    /// `expr as to`.
    fn cast(&self, e: ExprId, expr: ExprId, to: &ValueType) -> Result<Piece, Error> {
        let from = self.ir.expr_type(expr);
        let (source, target) = (Self::base(from), Self::base(to));
        let flagged = Self::is_flagged(from);
        // a real inside `Flagged` is a plain `f64`, so a mapped closure never sees an `R`
        let generic_real = *source == ValueType::F64 && !self.plain && !flagged;
        let operand = if flagged { "v_".to_string() } else { self.piece(expr)?.at(Prec::Atom) };
        let converted = match (source, target) {
            (ValueType::UInt(_) | ValueType::Int(_), ValueType::UInt(_)) => {
                self.integer_path(source, e)?;
                format!("{operand}.to_uint::<{}>()", self.integer_width(target, e)?)
            }
            (ValueType::UInt(_) | ValueType::Int(_), ValueType::Int(_)) => {
                self.integer_path(source, e)?;
                format!("{operand}.to_int::<{}>()", self.integer_width(target, e)?)
            }
            (ValueType::F64, ValueType::UInt(_) | ValueType::Int(_)) => {
                let x = if generic_real { format!("{operand}.value()") } else { operand };
                format!("{}::from_f64({x})", self.integer_path(target, e)?)
            }
            (ValueType::Bool, ValueType::UInt(_) | ValueType::Int(_)) => {
                format!("{}::from_bool({operand})", self.integer_path(target, e)?)
            }
            (ValueType::UInt(_) | ValueType::Int(_), ValueType::F64) => {
                self.integer_path(source, e)?;
                if self.plain || flagged { format!("{operand}.to_f64()") } else { format!("R::constant({operand}.to_f64())") }
            }
            _ => return Err(self.unsupported(e, &format!("a cast from {from} to {to}"))),
        };
        if flagged {
            let source_text = self.piece(expr)?.at(Prec::Atom);
            Ok(Piece::new(format!("{source_text}.map(|v_| {converted})"), Prec::Atom))
        } else {
            Ok(Piece::new(converted, Prec::Atom))
        }
    }

    /// The width of integer type `ty`, checked to be in 1..=64.
    fn integer_width(&self, ty: &ValueType, e: ExprId) -> Result<u32, Error> {
        self.integer_path(ty, e)?;
        Ok(match ty {
            ValueType::UInt(n) | ValueType::Int(n) => *n,
            _ => 0,
        })
    }

    /// `receiver.method(args)`.
    fn method(&self, e: ExprId, method: Method, receiver: ExprId, args: &[ExprId]) -> Result<Piece, Error> {
        let name = match method {
            Method::Min => "min",
            Method::Max => "max",
            Method::Clamp => "clamp",
            Method::Floor => "floor",
        };
        let operands: Vec<ExprId> = [receiver].into_iter().chain(args.iter().copied()).collect();
        let flagged = operands.iter().any(|o| Self::is_flagged(self.ir.expr_type(*o)));
        if !flagged {
            if *self.ir.expr_type(receiver) == ValueType::F64 && !self.plain {
                return Err(self.unsupported(e, &format!("`{name}` of a real in analog code")));
            }
            let mut printed = Vec::with_capacity(args.len());
            for a in args {
                printed.push(self.expr(*a)?);
            }
            return Ok(Piece::new(format!("{}.{name}({})", self.piece(receiver)?.at(Prec::Atom), printed.join(", ")), Prec::Atom));
        }
        let mut printed = Vec::with_capacity(operands.len());
        for o in &operands {
            printed.push(self.flagged(*o)?.at(Prec::Atom));
        }
        let text = match (method, printed.as_slice()) {
            (Method::Floor, [x]) => format!("{x}.map(|x_| x_.floor())"),
            (Method::Min | Method::Max, [x, y]) => format!("{x}.zip({y}, |x_, y_| x_.{name}(y_))"),
            (Method::Clamp, [x, lo, hi]) => format!("{x}.zip3({lo}, {hi}, |x_, lo_, hi_| x_.clamp(lo_, hi_))"),
            _ => return Err(self.unsupported(e, &format!("`{name}` with {} arguments", args.len()))),
        };
        Ok(Piece::new(text, Prec::Atom))
    }

    fn binary(&self, e: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Piece, Error> {
        let (lt, rt) = (self.ir.expr_type(lhs), self.ir.expr_type(rhs));
        let flagged = Self::is_flagged(lt) || Self::is_flagged(rt);
        if flagged && matches!(op, BinaryOp::And | BinaryOp::Or) {
            return Err(self.unsupported(e, "`&&` or `||` over a value that may be unknown (the source lowers it to `&` or `|`)"));
        }
        let real = *Self::base(lt) == ValueType::F64 && !(flagged || self.plain);
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
            BinaryOp::Rem if !real => ("%", Prec::Mul),
            BinaryOp::BitAnd if !real => ("&", Prec::BitAnd),
            BinaryOp::BitOr if !real => ("|", Prec::BitOr),
            BinaryOp::BitXor if !real => ("^", Prec::BitXor),
            BinaryOp::Shl if !real => ("<<", Prec::Shift),
            BinaryOp::Shr if !real => (">>", Prec::Shift),
            BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
                return Err(self.unsupported(e, &format!("the operator {op:?} on a real in analog code")));
            }
        };
        if flagged {
            return self.flagged_binary(op, symbol, prec, lhs, rhs);
        }
        let (l, r) = match prec {
            Prec::Compare => (self.compared(lhs)?, self.compared(rhs)?),
            // a bit operator and a shift keep every operand parenthesized: Rust's precedence for
            // them is not the one a reader expects
            Prec::BitAnd | Prec::BitOr | Prec::BitXor | Prec::Shift => {
                (self.piece(lhs)?.at(Prec::Unary), self.piece(rhs)?.at(Prec::Unary))
            }
            // left-associative: an equal-strength operand binds on the left, not on the right
            _ => (self.piece(lhs)?.at(prec), self.piece(rhs)?.at(Self::stronger(prec))),
        };
        Ok(Piece::new(format!("{l} {symbol} {r}"), prec))
    }

    /// A binary operation with an operand that may be unknown: both operands lifted, and an
    /// operator of `rt::Flagged` (a comparison is a method returning `Flagged<bool>`).
    fn flagged_binary(&self, op: BinaryOp, symbol: &str, prec: Prec, lhs: ExprId, rhs: ExprId) -> Result<Piece, Error> {
        let method = match op {
            BinaryOp::Lt => Some("less"),
            BinaryOp::Le => Some("less_or_equal"),
            BinaryOp::Gt => Some("greater"),
            BinaryOp::Ge => Some("greater_or_equal"),
            BinaryOp::Eq => Some("equal"),
            BinaryOp::Ne => Some("not_equal"),
            _ => None,
        };
        if let Some(method) = method {
            return Ok(Piece::new(format!("{}.{method}({})", self.flagged(lhs)?.at(Prec::Atom), self.flagged(rhs)?.text), Prec::Atom));
        }
        let (l, r) = match prec {
            Prec::BitAnd | Prec::BitOr | Prec::BitXor | Prec::Shift => {
                (self.flagged(lhs)?.at(Prec::Unary), self.flagged(rhs)?.at(Prec::Unary))
            }
            _ => (self.flagged(lhs)?.at(prec), self.flagged(rhs)?.at(Self::stronger(prec))),
        };
        Ok(Piece::new(format!("{l} {symbol} {r}"), prec))
    }

    /// An operand of a comparison: a real in an `R` context compares by value (a literal plain),
    /// anything else as it is; Rust's comparisons do not chain, so a weaker operand is
    /// parenthesized.
    fn compared(&self, e: ExprId) -> Result<String, Error> {
        if *self.ir.expr_type(e) != ValueType::F64 || self.plain {
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
            Prec::Compare => Prec::BitOr,
            Prec::BitOr => Prec::BitXor,
            Prec::BitXor => Prec::BitAnd,
            Prec::BitAnd => Prec::Shift,
            Prec::Shift => Prec::Add,
            Prec::Add => Prec::Mul,
            Prec::Mul => Prec::Unary,
            Prec::Unary | Prec::Atom => Prec::Atom,
        }
    }

    /// `if c {` arm `} else {` arm `}`; an `else` arm that is itself an `if` chains as `else if`,
    /// and a unit `if` with no `else` to speak of has none. The condition of an unknown `if` is
    /// `c.decide("<origin>")?`.
    fn if_else(&self, e: ExprId, cond: ExprId, then: ExprId, otherwise: ExprId) -> Result<String, Error> {
        let ty = self.ir.expr_type(e);
        let condition = if Self::is_flagged(self.ir.expr_type(cond)) {
            if !self.faults {
                return Err(self.unsupported(e, "an `if` on an unknown condition in analog code"));
            }
            format!("{}.decide({:?})?", self.piece(cond)?.at(Prec::Atom), self.at(e))
        } else {
            self.piece(cond)?.at(Prec::Or)
        };
        let mut text = format!("if {condition} {{\n{}\n}}", Self::indent(&self.arm(then, ty)?));
        match self.ir.expr(otherwise) {
            Expr::Unit if *ty == ValueType::Unit => {}
            Expr::If { cond, then, otherwise: next } => {
                text.push_str(" else ");
                text.push_str(&self.if_else(otherwise, *cond, *then, *next)?);
            }
            _ => text.push_str(&format!(" else {{\n{}\n}}", Self::indent(&self.arm(otherwise, ty)?))),
        }
        Ok(text)
    }

    /// The inside of a pair of braces: a block's lines without its braces, any other expression
    /// as it is, its value lifted to `want`.
    fn arm(&self, e: ExprId, want: &ValueType) -> Result<String, Error> {
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => self.block_lines(stmts, *value, want),
            _ => Ok(self.typed(e, want)?.text),
        }
    }

    /// A block's statements, one per line, then its value lifted to `want`.
    fn block_lines(&self, stmts: &[StmtId], value: Option<ExprId>, want: &ValueType) -> Result<String, Error> {
        let mut lines = Vec::with_capacity(stmts.len() + 1);
        for s in stmts {
            lines.push(self.stmt(*s)?);
        }
        if let Some(v) = value {
            lines.push(self.typed(v, want)?.text);
        }
        Ok(lines.join("\n"))
    }

    /// `let name = init;`, `expr;`, or a commit.
    pub fn stmt(&self, s: StmtId) -> Result<String, Error> {
        match self.ir.stmt(s) {
            Stmt::Let { local, init } => {
                let Some(name) = self.locals.get(*local as usize) else {
                    return Err(self.unsupported(*init, "a let of a local out of range"));
                };
                let init = match self.local_types.get(*local as usize) {
                    Some(ty) => self.expr_as(*init, ty)?,
                    None => self.expr(*init)?,
                };
                Ok(format!("let {name} = {init};"))
            }
            Stmt::Expr(e) => {
                let text = self.expr(*e)?;
                let block = matches!(self.ir.expr(*e), Expr::If { .. }) && *self.ir.expr_type(*e) == ValueType::Unit;
                Ok(if block { text } else { format!("{text};") })
            }
            Stmt::Contribute { addends, .. } => Err(Error::Unsupported {
                form: "contribution".into(),
                path: format!("{} s{}", self.path, s.index()),
                origin: addends.first().map_or_else(|| self.origin.clone(), |a| self.ir.expr_origin(a.value).clone()),
            }),
            Stmt::Emit { fields, .. } => Err(Error::Unsupported {
                form: "emit!".into(),
                path: format!("{} s{}", self.path, s.index()),
                origin: fields.iter().find_map(|f| match f {
                    EmitField::Value { value, .. } => Some(self.ir.expr_origin(*value).clone()),
                    EmitField::Text { .. } => None,
                }).unwrap_or_else(|| self.origin.clone()),
            }),
            Stmt::Commit { carrier, value, after } => {
                if !self.commits {
                    return Err(Error::Unsupported {
                        form: "commit".into(),
                        path: format!("{} s{}", self.path, s.index()),
                        origin: self.ir.expr_origin(*value).clone(),
                    });
                }
                let Some(ty) = self.carriers.get(*carrier as usize) else {
                    return Err(self.unsupported(*value, "a commit to a carrier out of range"));
                };
                let want = ValueType::Flagged(Box::new(ty.clone()));
                let value = self.expr_as(*value, &want)?;
                let min_after = after.and_then(|a| StatementPlan::min_after(self.ir, a));
                let after = match after {
                    Some(a) => self.expr_as(*a, &ValueType::Flagged(Box::new(ValueType::F64)))?,
                    None => "rt::Flagged::known(0.0)".to_string(),
                };
                let index = {
                    let mut done = self.committed.borrow_mut();
                    done.push(StatementPlan { carrier: *carrier, min_after });
                    done.len() - 1
                };
                Ok(format!("commits_.commit({index}, {value}, {after});"))
            }
        }
    }

    /// Every line of `text` four spaces deeper; empty lines stay empty.
    fn indent(text: &str) -> String {
        text.lines().map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect::<Vec<_>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{
        BinaryOp, Expr, ExprId, FunctionId, FunctionSpec, Ir, IrBuilder, Local, Method, Origin, Stmt, UnaryOp, ValueType,
    };

    use super::ValuePrinter;
    use crate::Error;
    use crate::alone::Alone;
    use crate::devices::Devices;
    use crate::digital::{Buck, Dx, Ty};

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
        // folded; `temperature()` becomes the trailing param `temperature_`
        let vt = "\
// fn vt @ analog.msl:10:1
fn vt<R: Real>(temperature_: R) -> R {
    R::constant(1.380649e-23) * temperature_ / R::constant(1.602176634e-19)
}
";
        assert_eq!(function(&ir, "vt"), vt);

        // std/analog.msl:21-31, line by line; its two `let arg` are distinct locals of one owner, printed `arg_4_` and `arg_5_`
        let pnjlim = "\
// fn pnjlim @ analog.msl:21:1
fn pnjlim<R: Real>(new: R, old: R, nvt: R, vcrit: R) -> R {
    if new.value() > vcrit.value() && (new - old).abs().value() > (R::constant(2.0) * nvt).value() {
        if old.value() > 0.0 {
            let arg_4_ = (new - old) / nvt;
            if arg_4_.value() > 0.0 {
                old + nvt * (R::constant(2.0) + (arg_4_ - R::constant(2.0)).ln())
            } else {
                old - nvt * (R::constant(2.0) + (R::constant(2.0) - arg_4_).ln())
            }
        } else {
            nvt * (new / nvt).ln()
        }
    } else if new.value() < 0.0 {
        let arg_5_ = if old.value() > 0.0 {
            -old - R::constant(1.0)
        } else {
            R::constant(2.0) * old - R::constant(1.0)
        };
        if new.value() < arg_5_.value() {
            arg_5_
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

    /// A function of one expression: `fn name(params) -> ret { body }` at `dpwm.msl:1`.
    fn defined(
        b: &mut IrBuilder,
        name: &str,
        params: &[(&str, ValueType)],
        ret: ValueType,
        body: impl FnOnce(&mut Dx<'_>) -> ExprId,
    ) -> FunctionId {
        let mut x = Dx::new(b, "dpwm.msl", 1, 1);
        let body = body(&mut x);
        let locals: Vec<Local> = params.iter().map(|(n, t)| Local { name: (*n).to_string(), ty: t.clone() }).collect();
        b.function(FunctionSpec {
            name: name.into(),
            origin: Origin { file: "dpwm.msl".into(), line: 1, col: 1 },
            params: locals.clone(),
            ret,
            locals,
            body,
        })
    }

    #[test]
    fn integer_and_flagged_expressions_print() {
        let mut b = IrBuilder::new();
        let rev3 = Buck::rev3(&mut b);
        let rev3_flagged = Buck::rev3_flagged(&mut b);
        Buck::dithered_width(&mut b, rev3);
        Buck::dithered_width_flagged(&mut b, rev3_flagged);
        let flag_u8 = Ty::flagged(Ty::u(8));
        // §3.3.4: `x.known_or(7)`, `x.is_known()`, `p.high()`, and the literal `Unknown`
        defined(&mut b, "or_seven", &[("x", flag_u8.clone())], Ty::u(8), |x| {
            let v = x.local(0, Ty::flagged(Ty::u(8)));
            let seven = x.int(7, Ty::u(8));
            x.known_or(v, seven, Ty::u(8))
        });
        defined(&mut b, "known", &[("x", flag_u8.clone())], ValueType::Bool, |x| {
            let v = x.local(0, Ty::flagged(Ty::u(8)));
            x.is_known(v)
        });
        defined(&mut b, "level", &[("p", Ty::flagged(ValueType::Bool))], ValueType::Bool, |x| {
            let v = x.local(0, Ty::flagged(ValueType::Bool));
            x.high(v)
        });
        defined(&mut b, "nothing", &[], flag_u8.clone(), |x| x.unknown(Ty::flagged(Ty::u(8))));
        // §1 rule 6: `-(a % b)` on `i16`; a cast from a flagged integer maps; a real cast to an
        // integer takes the real's value; `min` and `clamp` of a flagged receiver zip
        defined(&mut b, "negated_rem", &[("a", Ty::i(16)), ("b", Ty::i(16))], Ty::i(16), |x| {
            let (a, c) = (x.local(0, Ty::i(16)), x.local(1, Ty::i(16)));
            let rem = x.bin(BinaryOp::Rem, a, c, Ty::i(16));
            x.un(UnaryOp::Neg, rem, Ty::i(16))
        });
        defined(&mut b, "widen", &[("x", Ty::flagged(Ty::u(10)))], Ty::flagged(Ty::i(16)), |x| {
            let v = x.local(0, Ty::flagged(Ty::u(10)));
            x.cast(v, Ty::i(16), Ty::flagged(Ty::i(16)))
        });
        defined(&mut b, "truncated", &[("x", ValueType::F64)], Ty::u(10), |x| {
            let v = x.local(0, ValueType::F64);
            x.cast(v, Ty::u(10), Ty::u(10))
        });
        defined(&mut b, "capped", &[("x", Ty::flagged(Ty::u(8)))], Ty::flagged(Ty::u(8)), |x| {
            let v = x.local(0, Ty::flagged(Ty::u(8)));
            let cap = x.int(168, Ty::u(8));
            x.method(Method::Min, v, vec![cap], Ty::flagged(Ty::u(8)))
        });
        defined(&mut b, "held", &[("x", Ty::flagged(Ty::i(32)))], Ty::flagged(Ty::i(32)), |x| {
            let v = x.local(0, Ty::flagged(Ty::i(32)));
            let (lo, hi) = (x.int(0, Ty::i(32)), x.int(2880, Ty::i(32)));
            x.method(Method::Clamp, v, vec![lo, hi], Ty::flagged(Ty::i(32)))
        });
        let ir = Alone::finish(b);

        // dpwm.msl:14: `fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }`; a
        // literal is `rt::UInt::<3>::new(1)`, the shift amount literal `i32`; every operand of a
        // bit operator or a shift keeps its parentheses
        let rev3_text = "\
// fn rev3 @ dpwm.msl:14:1
fn rev3(k: rt::UInt<3>) -> rt::UInt<3> {
    (((k & rt::UInt::<3>::new(1)) << rt::Int::<32>::new(2)) | (k & rt::UInt::<3>::new(2))) | ((k >> rt::Int::<32>::new(2)) & rt::UInt::<3>::new(1))
}
";
        assert_eq!(function(&ir, "rev3"), rev3_text);

        // dpwm.msl:18-22: a cast is the type's method, `rev3(frame) < fine` compares the integers
        let dithered_text = "\
// fn dithered_width @ dpwm.msl:18:1
fn dithered_width(duty: rt::UInt<12>, frame: rt::UInt<3>) -> rt::UInt<9> {
    let coarse = (duty >> rt::Int::<32>::new(3)).to_uint::<9>();
    let fine = (duty & rt::UInt::<12>::new(7)).to_uint::<3>();
    if rev3(frame) < fine {
        coarse + rt::UInt::<9>::new(1)
    } else {
        coarse
    }
}
";
        assert_eq!(function(&ir, "dithered_width"), dithered_text);

        // the variant for a frame that may be unknown (msl-digital's `rev3~1`, `~` printing as
        // `_v`): every operand is lifted to `Flagged`, the `i32` shift amount too
        let rev3_flagged_text = "\
// fn rev3~1 @ dpwm.msl:14:1
fn rev3_v1(k: rt::Flagged<rt::UInt<3>>) -> rt::Flagged<rt::UInt<3>> {
    (((k & rt::Flagged::known(rt::UInt::<3>::new(1))) << rt::Flagged::known(rt::Int::<32>::new(2))) | (k & rt::Flagged::known(rt::UInt::<3>::new(2)))) | ((k >> rt::Flagged::known(rt::Int::<32>::new(2))) & rt::Flagged::known(rt::UInt::<3>::new(1)))
}
";
        assert_eq!(function(&ir, "rev3~1"), rev3_flagged_text);

        // the same `if` on a comparison that may be unknown: `decide` names the `if`
        // (dpwm.msl:21:5), so the function returns `Result` and wraps its value in `Ok`; the
        // known branches are lifted to the `if`'s type
        let dithered_flagged_text = "\
// fn dithered_width~01 @ dpwm.msl:18:1
fn dithered_width_v01(duty: rt::UInt<12>, frame: rt::Flagged<rt::UInt<3>>) -> Result<rt::Flagged<rt::UInt<9>>, Fault> {
    let coarse = (duty >> rt::Int::<32>::new(3)).to_uint::<9>();
    let fine = (duty & rt::UInt::<12>::new(7)).to_uint::<3>();
    Ok(if rev3_v1(frame).less(rt::Flagged::known(fine)).decide(\"dpwm.msl:21:5\")? {
        rt::Flagged::known(coarse + rt::UInt::<9>::new(1))
    } else {
        rt::Flagged::known(coarse)
    })
}
";
        assert_eq!(function(&ir, "dithered_width~01"), dithered_flagged_text);

        // the unknown-value forms of §3.3.4 and the integer operators of §1
        let text = |name: &str| function(&ir, name).lines().skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(text("or_seven"), "fn or_seven(x: rt::Flagged<rt::UInt<8>>) -> rt::UInt<8> {\n    x.known_or(rt::UInt::<8>::new(7))\n}");
        assert_eq!(text("known"), "fn known(x: rt::Flagged<rt::UInt<8>>) -> bool {\n    x.is_known()\n}");
        assert_eq!(text("level"), "fn level(p: rt::Flagged<bool>) -> bool {\n    p.high()\n}");
        assert_eq!(text("nothing"), "fn nothing() -> rt::Flagged<rt::UInt<8>> {\n    rt::Flagged::<rt::UInt<8>>::unknown()\n}");
        assert_eq!(text("negated_rem"), "fn negated_rem(a: rt::Int<16>, b: rt::Int<16>) -> rt::Int<16> {\n    -(a % b)\n}");
        assert_eq!(
            text("widen"),
            "fn widen(x: rt::Flagged<rt::UInt<10>>) -> rt::Flagged<rt::Int<16>> {\n    x.map(|v_| v_.to_int::<16>())\n}"
        );
        assert_eq!(
            text("truncated"),
            "fn truncated<R: Real>(x: R) -> rt::UInt<10> {\n    rt::UInt::<10>::from_f64(x.value())\n}"
        );
        assert_eq!(
            text("capped"),
            "fn capped(x: rt::Flagged<rt::UInt<8>>) -> rt::Flagged<rt::UInt<8>> {\n    x.zip(rt::Flagged::known(rt::UInt::<8>::new(168)), |x_, y_| x_.min(y_))\n}"
        );
        assert_eq!(
            text("held"),
            "fn held(x: rt::Flagged<rt::Int<32>>) -> rt::Flagged<rt::Int<32>> {\n    x.zip3(rt::Flagged::known(rt::Int::<32>::new(0)), rt::Flagged::known(rt::Int::<32>::new(2880)), |x_, lo_, hi_| x_.clamp(lo_, hi_))\n}"
        );
    }

    #[test]
    fn a_type_rust_lacks_is_refused_by_name() {
        // RS-Q6: `UInt<N>` and `Int<N>` hold 1..=64 bits; the ABI's leaves end at 64. (The IR has
        // no `Float<16>` or `Float<128>`: its only real is `f64`, so no `Ir` holds one to refuse.)
        let at = Origin { file: "dpwm.msl".into(), line: 1, col: 1 };
        for (ty, name) in [(Ty::u(65), "UInt<65>"), (Ty::i(65), "Int<65>"), (Ty::u(0), "UInt<0>"), (Ty::flagged(Ty::u(65)), "UInt<65>")] {
            let mut b = IrBuilder::new();
            let f = defined(&mut b, "wide", &[("x", ty.clone())], ValueType::Bool, |x| x.boolean(true));
            let ir = Alone::finish_unchecked(b);
            assert_eq!(
                ValuePrinter::function(&ir, f),
                Err(Error::Unsupported {
                    form: format!("the type {name}, whose width is outside 1..=64"),
                    path: "fn wide param x".into(),
                    origin: at.clone()
                }),
                "a parameter of {name}"
            );
        }
        // a literal, or a cast, of an integer too wide names the type and the expression
        let mut b = IrBuilder::new();
        let mut literal = None;
        let f = defined(&mut b, "big", &[], Ty::u(65), |x| {
            let e = x.int(1, Ty::u(65));
            literal = Some(e);
            e
        });
        let ir = Alone::finish_unchecked(b);
        let e = literal.expect("built");
        let err = ValuePrinter::function(&ir, f).expect_err("UInt<65> has no Rust type");
        assert_eq!(
            err,
            Error::Unsupported { form: "the type UInt<65>, whose width is outside 1..=64".into(), path: "fn big return".into(), origin: at.clone() }
        );
        let err = ValuePrinter::of_function(&ir, f).expr(e).expect_err("a UInt<65> literal has no Rust form");
        assert_eq!(
            err,
            Error::Unsupported {
                form: "the type UInt<65>, whose width is outside 1..=64".into(),
                path: format!("fn big e{}", e.index()),
                origin: at
            }
        );
        // and `check()` refuses the same IR, so `Transpiler` never reaches a printer with it
        assert!(!ir.check().is_empty(), "check() names the type too");
    }
}
