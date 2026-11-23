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
//! Phase B forms. An integer is `rt::UInt<N>` or `rt::Int<N>`, named by its `rt::` path in the
//! text. Every value is known, so an `if` is Rust's `if` and a carrier read is the value its pin
//! decoded to; in the digital printers a real is a plain `f64`, in an `analog` block and in an IR
//! function it is `R`.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use piperine_ir::{Addend, AnalysisKind,
    BinaryOp, Body, EmitField, Env, Expr, ExprId, FunctionId, Ir, Local, MathFn, Method, Origin, Param, Stmt, StmtId, Stream, TypeData,
    QuantityRole, Terminal, UnaryOp, ValueType,
};

use piperine_plan::{RecordLayout, StatementPlan};

use crate::Error;
use crate::print::{ContributionSums, Text, TypeItems};

/// Binding strength of a printed expression: an operand weaker than its place is parenthesized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Prec {
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
pub(super) struct Piece {
    pub(super) text: String,
    pub(super) prec: Prec,
}

impl Piece {
    pub(super) fn new(text: String, prec: Prec) -> Piece {
        Piece { text, prec }
    }

    /// The text, parenthesized when it binds weaker than `at_least`.
    pub(super) fn at(self, at_least: Prec) -> String {
        if self.prec < at_least { format!("({})", self.text) } else { self.text }
    }
}

/// Prints the value code of one owner: a function, a body's `analog` block, or the code of a
/// drive, an event or a reaction.
#[derive(Debug)]
pub struct ValuePrinter<'a> {
    pub(super) ir: &'a Ir,
    /// The owner's IR path, prefixed to an expression's id in an error: `fn pnjlim`.
    pub(super) path: String,
    /// The printed name of each local of the owner, by index.
    locals: Vec<String>,
    /// The type of each local of the owner, by index.
    pub(super) local_types: Vec<ValueType>,
    /// Whether each local of the owner was declared `let mut`, by index: only that local prints
    /// `let mut` and is the target of an assignment (§1.5.12).
    pub(super) mutable: Vec<bool>,
    /// Whether each local of the owner is a temporary the elaborator made (its IR name holds a `$`, which no
    /// written identifier can), by index: such a local may go unread (a `match` temporary whose arm binds nothing).
    synthesized: Vec<bool>,
    /// The result type of the function being printed; `None` for any other owner. A `return`
    /// lifts its value to it.
    pub(super) returns: Option<ValueType>,
    /// The function being printed; `None` for any other owner. A call it makes to a function on a
    /// call cycle with it passes a deeper `rt::Depth`.
    pub(super) function: Option<FunctionId>,
    /// How many loops' bodies are being printed: a `break` or `continue` is Rust's, outside a loop it is refused.
    pub(super) loops: Cell<u32>,
    /// The printed name of each param of the owner's body, by index; empty for a function.
    params: Vec<String>,
    /// The type each carrier of the owner's body holds, by index; empty for a function.
    carriers: Vec<ValueType>,
    /// The record streams of the owner's body, by index; empty for a function.
    streams: Vec<Stream>,
    /// The owner's origin: where an error with no expression of its own points.
    pub(super) origin: Origin,
    /// How the analog read forms print; `None` outside an `analog` block.
    analog: Option<Reads>,
    /// The accumulators a contribution adds to, in the structured form of an `analog` block
    /// (`ContributionSums`); `None` where a contribution has no lowering.
    sums: Option<ContributionSums>,
    /// A real is a plain `f64` (the digital printers) rather than `R` (an `analog` block, an IR
    /// function).
    pub(super) plain: bool,
    /// Whether a commit has a place to go: the code of a reaction.
    pub(super) commits: bool,
    /// The carrier and `min_after` of each commit printed so far, in printing order: the order
    /// of the reaction's statements in the shape.
    committed: RefCell<Vec<StatementPlan>>,
    /// Whether the code printed so far holds an `emit!` on the stream `sim` that sets `finish`:
    /// the reaction then asks its `Commits` for the flow (`Commits::finish`, `Commits::flow`;
    /// msl-fixes FIX-01, §19 Resolution).
    finishes: Cell<bool>,
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
    /// By body branch: the variable of a potential branch's flow pin (`i(a, b)`, msl-digital A5).
    pub flows: Vec<String>,
    /// By derivative site: the variable of its rate pin (`rate(D<k>)`, analog-structure AS-07).
    pub derivatives: Vec<String>,
    /// By body param: the variable its pin's `PREVIOUS` was decoded into (`p.prev()`, §5.3.1).
    pub param_previous: Vec<String>,
    /// By body carrier: the variable its pin's `PREVIOUS` was decoded into (`c.prev()`, A7).
    pub previous: Vec<String>,
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
            mutable: f.locals().iter().map(|l| l.mutable).collect(),
            synthesized: f.locals().iter().map(Self::is_synthesized).collect(),
            returns: Some(f.ret().clone()),
            function: Some(function),
            loops: Cell::new(0),
            params: Vec::new(),
            carriers: Vec::new(),
            streams: Vec::new(),
            origin,
            analog: None,
            sums: None,
            plain: false,
            commits: false,
            committed: RefCell::new(Vec::new()),
            finishes: Cell::new(false),
            used: RefCell::new(BTreeSet::new()),
        }
    }

    /// The printer of a type item's field types: a real is `R`, the item's own generic parameter, or
    /// a plain `f64` for the impls written for the item at `f64` (`plain`).
    pub fn of_type(ir: &'a Ir, data: &TypeData, plain: bool) -> ValuePrinter<'a> {
        ValuePrinter {
            ir,
            path: format!("type {}", data.path),
            locals: Vec::new(),
            local_types: Vec::new(),
            mutable: Vec::new(),
            synthesized: Vec::new(),
            returns: None,
            function: None,
            loops: Cell::new(0),
            params: Vec::new(),
            carriers: Vec::new(),
            streams: Vec::new(),
            origin: data.origin.clone(),
            analog: None,
            sums: None,
            plain,
            commits: false,
            committed: RefCell::new(Vec::new()),
            finishes: Cell::new(false),
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
            mutable: body.locals().iter().map(|l| l.mutable).collect(),
            synthesized: body.locals().iter().map(Self::is_synthesized).collect(),
            returns: None,
            function: None,
            loops: Cell::new(0),
            params,
            carriers: body.carriers().iter().map(|c| c.ty.clone()).collect(),
            streams: body.streams().to_vec(),
            origin,
            analog: None,
            sums: None,
            plain: false,
            commits: false,
            committed: RefCell::new(Vec::new()),
            finishes: Cell::new(false),
            used: RefCell::new(BTreeSet::new()),
        }
    }

    /// The printer of the code of a drive or an event of `body`: its locals are the body's, a
    /// real is a plain `f64`.
    pub fn of_digital(ir: &'a Ir, body: Body<'a>) -> ValuePrinter<'a> {
        ValuePrinter { plain: true, ..Self::of_body(ir, body) }
    }

    /// The printer of an expression a placement gives a child's param (`ChildValue::Expr`): over
    /// the params of `body`, plain reals.
    pub fn of_placement(ir: &'a Ir, body: Body<'a>) -> ValuePrinter<'a> {
        Self::of_digital(ir, body)
    }

    /// The printer of reaction `k` of `body`: its own locals, plain reals, and commits.
    pub fn of_reaction(ir: &'a Ir, body: Body<'a>, k: usize) -> ValuePrinter<'a> {
        let base = Self::of_digital(ir, body);
        let Some(reaction) = body.reactions().get(k) else { return base };
        let params: Vec<String> = body.params().iter().map(|p: &Param| Text::ident(&p.name)).collect();
        let reserved = Self::reserved(&body, &params);
        ValuePrinter {
            path: format!("body {} reaction {k}", body.name()),
            locals: Self::names(&reaction.locals, &reserved),
            local_types: reaction.locals.iter().map(|l| l.ty.clone()).collect(),
            mutable: reaction.locals.iter().map(|l| l.mutable).collect(),
            synthesized: reaction.locals.iter().map(Self::is_synthesized).collect(),
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

    /// This printer with a contribution printing as an addition to the accumulators `sums` names.
    pub fn with_contribution_sums(self, sums: ContributionSums) -> ValuePrinter<'a> {
        ValuePrinter { sums: Some(sums), ..self }
    }

    /// The carrier and `min_after` of each commit printed so far, in printing order.
    pub fn committed(&self) -> Vec<StatementPlan> {
        self.committed.borrow().clone()
    }

    /// Whether the code printed so far ends the analysis: an `emit!` on `sim` with a `finish` field.
    pub fn finishes(&self) -> bool {
        self.finishes.get()
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

    /// The Rust type a pin of `ty` is decoded as: a real is the plain `f64` whatever the code's
    /// own real is.
    pub fn pin_type(&self, ty: &ValueType) -> Result<String, Error> {
        match ty {
            ValueType::F64 => Ok("f64".to_string()),
            other => self.rust_type(other, &self.path),
        }
    }

    /// The expression that decodes the pin bytes `bytes` as a value of the Rust type `rust`.
    pub(crate) fn pin_value(rust: &str, bytes: &str) -> String {
        format!("<{rust} as rt::Carried>::from_pin({bytes})")
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
    pub(crate) fn read(&self, name: &str) -> String {
        self.used.borrow_mut().insert(name.to_string());
        name.to_string()
    }

    /// The read `variable` of a carrier or its previous value, as the code's own real when the
    /// carrier holds one: the pin decodes at `f64`.
    fn carrier_value(&self, e: ExprId, variable: String) -> Piece {
        if *self.ir.expr_type(e) == ValueType::F64 && !self.plain {
            Piece::new(format!("R::constant({variable})"), Prec::Atom)
        } else {
            Piece::new(variable, Prec::Atom)
        }
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
            Terminal::Own(_) => panic!("Ir::check: an own terminal belongs in a child's node slot only"),
        };
        let (p, m) = (node(plus)?, node(minus)?);
        Some(match minus {
            Terminal::Ground if plus != Terminal::Ground => Piece::new(p, Prec::Atom),
            _ => Piece::new(format!("{p} - {m}"), Prec::Add),
        })
    }

    /// `function` as one Rust `fn` under its comment: its params, then one trailing param per
    /// environment read it makes. It is generic over `Real` when a real appears in it.
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
            let ty = if matches!(env, Env::Static | Env::Analysis(_)) { "bool" } else { "R" };
            params.push(format!("{}: {ty}", Self::env_name(*env)));
        }
        // a function on a call cycle carries its depth (Q5 (b)); one that is not keeps its text
        if Self::is_recursive(ir, function) {
            params.push("depth_: rt::Depth".to_string());
        }
        let ret = printer.rust_type(f.ret(), &format!("{} return", printer.path))?;
        let generic = if Self::is_generic(ir, function) { "<R: Real>" } else { "" };
        let mut t = Text::new();
        t.comment(&printer.path, f.origin());
        t.line(&format!("fn {}{generic}({}) -> {ret} {{", Text::ident(f.name()), params.join(", ")));
        let body = printer.function_body(f.body())?;
        t.indented(|t| t.line(&body));
        t.line("}");
        Ok(t.finish())
    }

    /// Whether `function` is generic over `Real`: a real (or a type holding one) is among its params, its return, its
    /// locals, or it reads an environment real (every read but `analysis(..)`, a `bool`).
    pub fn is_generic(ir: &Ir, function: FunctionId) -> bool {
        let f = ir.function(function);
        let items = TypeItems::new(ir);
        let real = |ty: &ValueType| items.holds_real(ty);
        f.params().iter().chain(f.locals()).any(|l| real(&l.ty))
            || real(f.ret())
            || Self::env_reads(ir, function).iter().any(|e| !matches!(e, Env::Static | Env::Analysis(_)))
            || Self::computes_real_value(ir, function)
    }

    /// Whether some expression of the body of `function` (a callee's aside) has a type that holds a
    /// real: an aggregate built and compared where no local or result holds it prints `R`.
    fn computes_real_value(ir: &Ir, function: FunctionId) -> bool {
        let items = TypeItems::new(ir);
        let mut exprs = vec![ir.function(function).body()];
        while let Some(e) = exprs.pop() {
            if matches!(ir.expr_type(e), ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Struct(_) | ValueType::Enum(_) | ValueType::Option(_)) && items.holds_real(ir.expr_type(e)) {
                return true;
            }
            Self::children(ir, ir.expr(e), &mut exprs);
        }
        false
    }

    /// The body of a function: its statements, then its value.
    fn function_body(&self, e: ExprId) -> Result<String, Error> {
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => {
                let mut lines = Vec::with_capacity(stmts.len() + 1);
                for s in stmts {
                    lines.push(self.stmt(*s)?);
                }
                match value {
                    Some(v) => lines.push(self.expr(*v)?),
                    None => {}
                }
                Ok(lines.join("\n"))
            }
            _ => self.expr(e),
        }
    }

    /// `e` as a Rust expression; an `if` or a block spans several lines, each inner line indented
    /// by four spaces relative to the first.
    pub fn expr(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.text)
    }

    /// `e` as the receiver of a method call: parenthesized unless it is an atom.
    pub fn operand(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.at(Prec::Atom))
    }

    /// `e`, a `bool`, as an operand of `&&`.
    pub fn condition(&self, e: ExprId) -> Result<String, Error> {
        Ok(self.piece(e)?.at(Prec::Compare))
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
                    Expr::FunctionRef(function) => functions.push(*function),
                    other => Self::children(ir, other, &mut exprs),
                }
            }
        }
        found.into_iter().collect()
    }

    /// The generated variable an environment read is: `temperature_`, `gmin_`, `source_scale_`,
    /// `time_`, `is_static_`.
    pub fn env_name(env: Env) -> &'static str {
        match env {
            Env::Temperature => "temperature_",
            Env::Gmin => "gmin_",
            Env::SourceScale => "source_scale_",
            Env::Time => "time_",
            Env::Static => "is_static_",
            Env::Freq => "freq_",
            Env::Analysis(AnalysisKind::Dc) => "is_dc_",
            Env::Analysis(AnalysisKind::Transient) => "is_transient_",
            Env::Analysis(AnalysisKind::Ac) => "is_ac_",
            Env::Analysis(AnalysisKind::Noise) => "is_noise_",
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
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Table { x: expr, .. } | Expr::TableSlope { x: expr, .. } => out.push(*expr),
            Expr::Unwrap { is, value, .. } => out.extend([*is, *value]),
            Expr::Same { a, b } => out.extend([*a, *b]),
            Expr::LevelCast { expr, .. } => out.push(*expr),
            Expr::Method { receiver, args, .. } => out.extend([*receiver].into_iter().chain(args.iter().copied())),
            Expr::Binary { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
            aggregate @ (Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_)) => out.extend(aggregate.operands()),
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
            | Expr::Carrier(_)
            | Expr::Flow { .. }
            | Expr::Derivative(_)
            | Expr::ParamPrevious(_)
            | Expr::Fired(_)
            | Expr::Previous(_)
            | Expr::FunctionRef(_) => {}
            control @ (Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::CallValue { .. }) => {
                out.extend(control.operands());
                for s in control.statements() {
                    out.extend(ir.stmt(*s).operands());
                }
            }
        }
    }

    /// Each local's MSL name as `Text::ident` prints it; the generated `<name>_<index>_` when
    /// another local of the same owner, or a name in `reserved` (a param, node or carrier the
    /// code reads), prints the same.
    fn names(locals: &[Local], reserved: &[String]) -> Vec<String> {
        locals
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let printed = Text::ident(&l.name);
                let shared = locals.iter().filter(|o| Text::ident(&o.name) == printed).count() > 1;
                let hides = reserved.contains(&printed);
                if shared || hides { Text::generated(&format!("{}_{i}", l.name)) } else { Text::ident(&l.name) }
            })
            .collect()
    }

    /// `ty` as the source spells it: `UInt<65>`, `Int<0>`.
    fn type_name(ty: &ValueType) -> String {
        match ty {
            ValueType::UInt(n) => format!("UInt<{n}>"),
            ValueType::Int(n) => format!("Int<{n}>"),
            other => other.to_string(),
        }
    }

    /// The Rust type of a value type: `R` (`f64` in plain code), `bool`, `()`, `rt::UInt<N>`,
    /// `rt::Int<N>`. A type
    /// Rust lacks is `Error::Unsupported` naming it: an integer wider than 64 bits (or of no
    /// width), a function type. (The IR has no `Float<16>` or `Float<128>`: the only real is
    /// `f64`, so there is nothing of those to refuse.)
    pub(super) fn rust_type(&self, ty: &ValueType, path: &str) -> Result<String, Error> {
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
            ValueType::Fn(params, ret) => self.function_type(params, ret, path),
            ValueType::Tuple(parts) => {
                let parts: Result<Vec<String>, Error> = parts.iter().map(|part| self.rust_type(part, path)).collect();
                let parts = parts?;
                Ok(if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) })
            }
            ValueType::Array(element, length) => Ok(format!("[{}; {length}]", self.rust_type(element, path)?)),
            ValueType::Option(inner) => Ok(format!("Option<{}>", self.rust_type(inner, path)?)),
            // One item per arena entry (`print/types.rs`); a type that holds a real is generic over it.
            ValueType::Struct(id) | ValueType::Enum(id) => {
                let items = TypeItems::new(self.ir);
                let generic = match (items.holds_real(ty), self.plain) {
                    (false, _) => String::new(),
                    (true, true) => "<f64>".to_string(),
                    (true, false) => "<R>".to_string(),
                };
                Ok(format!("{}{generic}", items.name(*id)))
            }
        }
    }

    /// `rt::UInt::<N>` or `rt::Int::<N>`: the path an integer's associated functions are called
    /// through; a width outside 1..=64 is refused as `rust_type` refuses it.
    pub(super) fn integer_path(&self, ty: &ValueType, e: ExprId) -> Result<String, Error> {
        match ty {
            ValueType::UInt(n) | ValueType::Int(n) if (1..=64).contains(n) => {
                Ok(format!("rt::{}::<{n}>", if matches!(ty, ValueType::UInt(_)) { "UInt" } else { "Int" }))
            }
            other => Err(self.unsupported(e, &format!("the type {}, whose width is outside 1..=64", Self::type_name(other)))),
        }
    }

    pub(super) fn unsupported(&self, e: ExprId, form: &str) -> Error {
        Error::Unsupported {
            form: form.to_string(),
            path: format!("{} e{}", self.path, e.index()),
            origin: self.ir.expr_origin(e).clone(),
        }
    }

    /// A `table_model` (or its slope) as a block: the model is built once, in a `static` of its own, from the samples and
    /// the control; the value and the slope are read at the value of `x`, and the tangent of `x` goes through the slope
    /// (`Real::compose`), so the derivative of a contribution is that of the interpolant (§22.2.15 rule 5).
    fn table_piece(&self, x: ExprId, table: &[(piperine_ir::Real, piperine_ir::Real)], control: &piperine_ir::TableControl, slope_only: bool) -> Result<Piece, Error> {
        let samples: Vec<String> = table.iter().map(|(a, v)| format!("({}, {})", Text::real(a.get()), Text::real(v.get()))).collect();
        let at = self.piece(x)?.at(Prec::Atom);
        let result = if slope_only { "x_.compose(model_.slope(x_.value()), 0.0)" } else { "x_.compose(model_.value(x_.value()), model_.slope(x_.value()))" };
        Ok(Piece::new(
            format!("{{ static TABLE_: std::sync::OnceLock<rt::TableModel> = std::sync::OnceLock::new(); let model_ = TABLE_.get_or_init(|| rt::TableModel::new(&[{}], \"{}\")); let x_ = {at}; {result} }}", samples.join(", "), control.text()),
            Prec::Atom,
        ))
    }

    pub(super) fn piece(&self, e: ExprId) -> Result<Piece, Error> {
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
            // the event pin's `CHANGED` byte of this microstep, bound as a `bool` (abi.md §4.1)
            Expr::Fired(k) => atom(self.read(&Text::generated(&format!("event{k}_fired")))),
            // the forms of §22.1: the levels of the operands, compared (`same`) or moved to another type (`cast`)
            Expr::Same { a, b } => Ok(Piece::new(format!("{} == {}", self.levels(*a)?, self.levels(*b)?), Prec::Compare)),
            Expr::LevelCast { expr, to, .. } => self.level_cast(*expr, to),
            Expr::Unwrap { is, value, message } => {
                let message = message.as_ref().map_or_else(|| "None".to_string(), |m| format!("Some({m:?})"));
                atom(format!("rt::Unwrap::of({}, {}, {message})", self.expr(*is)?, self.expr(*value)?))
            }
            Expr::Carrier(c) => match self.analog.as_ref().and_then(|r| r.carriers.get(*c as usize)) {
                Some(name) => Ok(self.carrier_value(e, self.read(name))),
                None => Err(self.unsupported(e, "a carrier read")),
            },
            Expr::Unary { op, expr } => {
                let operand = self.piece(*expr)?.at(Prec::Atom);
                let sign = if *op == UnaryOp::Neg { "-" } else { "!" };
                Ok(Piece::new(format!("{sign}{operand}"), Prec::Unary))
            }
            Expr::Binary { op, lhs, rhs } if self.is_aggregate_operand(*lhs) => self.aggregate_comparison(e, *op, *lhs, *rhs),
            Expr::Binary { op, lhs, rhs } => self.binary(e, *op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => self.if_else(e, *cond, *then, *otherwise),
            Expr::Block { stmts, value } => {
                let lines = self.block_lines(stmts, *value)?;
                let text = if lines.is_empty() { "{}".to_string() } else { format!("{{\n{}\n}}", Self::indent(&lines)) };
                Ok(Piece::new(text, Prec::Low))
            }
            Expr::Call { function, args } => {
                let params = self.ir.function(*function).params();
                let mut printed = args.iter().map(|a| self.expr(*a)).collect::<Result<Vec<_>, Error>>()?;
                printed.extend(Self::env_reads(self.ir, *function).into_iter().map(|env| self.read(Self::env_name(env))));
                printed.extend(self.depth_argument(*function));
                // a generic function called with no argument that fixes its `Real` type names it
                let items = TypeItems::new(self.ir);
                // `R` is fixed by what the call passes or returns; a function that holds a real only in
                // a local (a struct field) leaves it open, so the call names it
                let ret = self.ir.function(*function).ret();
                let aggregate_result = matches!(ret, ValueType::Struct(_) | ValueType::Enum(_) | ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Option(_)) && items.holds_real(ret);
                // in reaction code (`plain`) a result alone does not fix it: a value that goes on through a
                // generic call leaves it open
                // a scalar result alone, with arguments that hold no real (a method of an enum that returns an `f64`),
                // leaves `R` open where the call is an argument of a generic function, so the call names it
                let result_alone = !params.is_empty() && !params.iter().any(|p| items.holds_real(&p.ty));
                let fixed = params.iter().any(|p| items.holds_real(&p.ty)) || (items.holds_real(ret) && !aggregate_result && !result_alone && !(self.plain && Self::computes_real_value(self.ir, *function))) || !Self::env_reads(self.ir, *function).is_empty();
                let real = if (printed.is_empty() || !fixed) && Self::is_generic(self.ir, *function) { format!("::<{}>", if self.plain { "f64" } else { "R" }) } else { String::new() };
                atom(format!("{}{real}({})", Text::ident(self.ir.function(*function).name()), printed.join(", ")))
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
            Expr::Table { x, table, control } => self.table_piece(*x, table, control, false),
            Expr::TableSlope { x, table, control } => self.table_piece(*x, table, control, true),
            // a closure is a function value, which is the reference `&dyn Fn` takes
            Expr::Closure { .. } => Ok(Piece::new(format!("&{}", self.closure(e)?), Prec::Unary)),
            Expr::Probe { plus, minus } => self.probe_piece(*plus, *minus).ok_or_else(|| self.unsupported(e, "probe")),
            Expr::Derivative(k) => match self.analog.as_ref().and_then(|r| r.derivatives.get(*k as usize)) {
                Some(name) => atom(self.read(name)),
                None => Err(self.unsupported(e, "the rate of a derivative site")),
            },
            Expr::ParamPrevious(p) => match self.analog.as_ref().and_then(|r| r.param_previous.get(*p as usize)) {
                Some(name) => atom(self.read(name)),
                None => Err(self.unsupported(e, "the previous value of a param")),
            },
            Expr::Flow { branch } => match self.analog.as_ref().and_then(|r| r.flows.get(*branch as usize)) {
                Some(name) => atom(self.read(name)),
                None => Err(self.unsupported(e, "the flow of a potential branch")),
            },
            Expr::Previous(c) => match self.analog.as_ref().and_then(|r| r.previous.get(*c as usize)) {
                Some(name) => Ok(self.carrier_value(e, self.read(name))),
                None => Err(self.unsupported(e, "the previous value of a carrier")),
            },
            Expr::Limit { site, .. } => match self.analog.as_ref().and_then(|r| r.limits.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "limit")),
            },
            Expr::Stim { site, .. } => match self.analog.as_ref().and_then(|r| r.stims.get(*site as usize)) {
                Some(name) => atom(name.clone()),
                None => Err(self.unsupported(e, "ac_stim")),
            },
            Expr::Cast { expr, to } => self.cast(e, *expr, to),
            Expr::Method { method, receiver, args } => self.method(*method, *receiver, args),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => self.loop_expr(e),
            Expr::Break(value) => self.leave_loop(e, *value),
            Expr::Continue => self.continue_loop(e),
            Expr::Return(value) => self.return_from_function(e, *value),
            Expr::FunctionRef(function) => self.function_value(*function),
            Expr::CallValue { callee, args } => self.call_value(e, *callee, args),
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => self.aggregate(e),
        }
    }

    /// The closure `e` as the closure itself: `move |a, b| body`, which captures by value
    /// (§1.5.14). A `limit` function is this text; as a value it is the reference to it.
    pub fn closure(&self, e: ExprId) -> Result<String, Error> {
        let Expr::Closure { params, body } = self.ir.expr(e) else {
            return Err(self.unsupported(e, "a form that is not a closure"));
        };
        let mut names = Vec::with_capacity(params.len());
        for p in params {
            match self.locals.get(*p as usize) {
                Some(name) => names.push(Self::unused_prefix(self.ir, *body, *p, name)),
                None => return Err(self.unsupported(e, "a closure parameter out of range")),
            }
        }
        Ok(format!("move |{}| {}", names.join(", "), self.expr(*body)?))
    }

    /// The levels of `e` as `rt::LevelWords`: what `same` compares and `cast` moves. A real of the
    /// code's own type (`R`) is read at its value.
    fn levels(&self, e: ExprId) -> Result<String, Error> {
        // a call argument needs no parentheses; the `.value()` of a real binds to an atom
        let real = *self.ir.expr_type(e) == ValueType::F64 && !self.plain;
        let operand = self.piece(e)?.at(if real { Prec::Atom } else { Prec::Low });
        let operand = if real { format!("{operand}.value()") } else { operand };
        Ok(format!("rt::Carried::to_levels({operand})"))
    }

    /// `cast::<A, B>(x)` (§22.1.3): the levels of `x` under `to`, an `Option<B>`: `None` for a tag that
    /// names no variant of an enum `to` (or of one in a struct or a payload of it).
    fn level_cast(&self, expr: ExprId, to: &ValueType) -> Result<Piece, Error> {
        let target = self.pin_type(to)?;
        let cast = format!("<{target} as rt::Carried>::try_from_levels(&{})", self.levels(expr)?);
        let text = if *to == ValueType::F64 && !self.plain { format!("{cast}.map(R::constant)") } else { cast };
        Ok(Piece::new(text, Prec::Atom))
    }

    /// `expr as to`.
    fn cast(&self, e: ExprId, expr: ExprId, to: &ValueType) -> Result<Piece, Error> {
        let from = self.ir.expr_type(expr);
        let (source, target) = (from, to);
        let generic_real = *source == ValueType::F64 && !self.plain;
        let operand = self.piece(expr)?.at(Prec::Atom);
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
            // an enum without fields converts to its discriminant, which Rust gives as the enum cast to an integer
            (ValueType::Enum(_), ValueType::UInt(_)) => format!("{}::new(({operand} as i64) as u64)", self.integer_path(target, e)?),
            (ValueType::Enum(_), ValueType::Int(_)) => format!("{}::new({operand} as i64)", self.integer_path(target, e)?),
            (ValueType::UInt(_) | ValueType::Int(_), ValueType::F64) => {
                self.integer_path(source, e)?;
                if self.plain { format!("{operand}.to_f64()") } else { format!("R::constant({operand}.to_f64())") }
            }
            _ => return Err(self.unsupported(e, &format!("a cast from {from} to {to}"))),
        };
        Ok(Piece::new(converted, Prec::Atom))
    }

    /// The width of integer type `ty`, checked to be in 1..=64.
    fn integer_width(&self, ty: &ValueType, e: ExprId) -> Result<u32, Error> {
        self.integer_path(ty, e)?;
        Ok(match ty {
            ValueType::UInt(n) | ValueType::Int(n) => *n,
            _ => 0,
        })
    }

    /// `receiver.method(args)`, each row by its MSL name (`Method::name`).
    fn method(&self, method: Method, receiver: ExprId, args: &[ExprId]) -> Result<Piece, Error> {
        let name = method.name();
        let mut printed = Vec::with_capacity(args.len());
        for a in args {
            let text = match (method, self.ir.expr(*a)) {
                // `powi` takes a Rust `i32`; the IR's `i32` is an `rt::Int<32>`. A literal exponent
                // is an `i32` literal: analog code has no `rt::Int`.
                (Method::Powi, Expr::Int(v)) => format!("{v}_i32"),
                (Method::Powi, _) => format!("{}.get() as i32", self.piece(*a)?.at(Prec::Atom)),
                _ => self.expr(*a)?,
            };
            printed.push(text);
        }
        Ok(Piece::new(format!("{}.{name}({})", self.piece(receiver)?.at(Prec::Atom), printed.join(", ")), Prec::Atom))
    }

    /// The Rust operator `op` prints as and how strongly it binds; `real` is a real in an `R`
    /// context, which has no remainder, bit or shift operator (§12 of the analog subset).
    pub(super) fn operator(&self, e: ExprId, op: BinaryOp, real: bool) -> Result<(&'static str, Prec), Error> {
        Ok(match op {
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
        })
    }

    fn binary(&self, e: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Piece, Error> {
        let real = *self.ir.expr_type(lhs) == ValueType::F64 && !self.plain;
        let (symbol, prec) = self.operator(e, op, real)?;
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

    pub(super) fn stronger(prec: Prec) -> Prec {
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
    /// and a unit `if` with no `else` to speak of has none.
    fn if_else(&self, e: ExprId, cond: ExprId, then: ExprId, otherwise: ExprId) -> Result<Piece, Error> {
        let ty = self.ir.expr_type(e);
        let condition = if matches!(self.ir.expr(cond), Expr::If { .. } | Expr::Block { .. }) {
            // `if if a { b } else { c } { .. }` and `if { s; c } { .. }` are valid Rust; parentheses
            // around either are a warning.
            self.piece(cond)?.text
        } else {
            self.piece(cond)?.at(Prec::Or)
        };
        let mut text = format!("if {condition} {}", Self::braced(&self.arm(then)?));
        match self.ir.expr(otherwise) {
            Expr::Unit if *ty == ValueType::Unit => {}
            // an arm that does nothing (a `match` arm with no contribution) prints no `else`
            Expr::Block { stmts, value: None } if stmts.is_empty() && *ty == ValueType::Unit => {}
            Expr::If { cond, then, otherwise: next } => {
                text.push_str(" else ");
                text.push_str(&self.if_else(otherwise, *cond, *then, *next)?.text);
            }
            _ => text.push_str(&format!(" else {}", Self::braced(&self.arm(otherwise)?))),
        }
        Ok(Piece::new(text, Prec::Low))
    }

    /// `lines` in a pair of braces, indented; `{}` when there is nothing inside.
    pub(super) fn braced(lines: &str) -> String {
        if lines.is_empty() { "{}".to_string() } else { format!("{{\n{}\n}}", Self::indent(lines)) }
    }

    /// The inside of a pair of braces: a block's lines without its braces, any other expression
    /// as it is.
    pub(super) fn arm(&self, e: ExprId) -> Result<String, Error> {
        match self.ir.expr(e) {
            Expr::Block { stmts, value } => self.block_lines(stmts, *value),
            _ => self.expr(e),
        }
    }

    /// A block's statements, one per line, then its value.
    pub(super) fn block_lines(&self, stmts: &[StmtId], value: Option<ExprId>) -> Result<String, Error> {
        let mut lines = Vec::with_capacity(stmts.len() + 1);
        for s in stmts {
            lines.push(self.stmt(*s)?);
        }
        if let Some(v) = value {
            lines.push(self.expr(v)?);
        }
        Ok(lines.join("\n"))
    }

    /// Whether `local` is a temporary of the elaborator rather than a name the author wrote.
    fn is_synthesized(local: &Local) -> bool {
        local.name.contains('$')
    }

    /// `e` as a statement: an `if` closes with its brace; anything else takes a `;`.
    pub(super) fn expression_statement(&self, e: ExprId) -> Result<String, Error> {
        let piece = self.piece(e)?;
        let block = matches!(self.ir.expr(e), Expr::If { .. }) && *self.ir.expr_type(e) == ValueType::Unit && piece.prec == Prec::Low;
        Ok(if block { piece.text } else { format!("{};", piece.text) })
    }

    /// `let name = init;`, `expr;`, or a commit.
    pub fn stmt(&self, s: StmtId) -> Result<String, Error> {
        match self.ir.stmt(s) {
            Stmt::Let { local, init } => {
                let Some(name) = self.locals.get(*local as usize) else {
                    return Err(self.unsupported(*init, "a let of a local out of range"));
                };
                let init = self.expr(*init)?;
                // a deferred `let x;` starts at a placeholder its first assignment overwrites
                // a temporary the elaborator makes for a `match` (`match$0.val`) is unread when its arm binds nothing (`Some(_)`)
                let keyword = if self.mutable.get(*local as usize).copied().unwrap_or(false) {
                    "#[allow(unused_assignments, unused_variables)] let mut"
                } else if self.synthesized.get(*local as usize).copied().unwrap_or(false) {
                    "#[allow(unused_variables)] let"
                } else {
                    "let"
                };
                // a function value has no type of its own to infer: the local is the `&dyn Fn` it holds
                let annotation = match self.local_types.get(*local as usize) {
                    Some(ty @ ValueType::Fn(..)) => format!(": {}", self.rust_type(ty, &self.path)?),
                    _ => String::new(),
                };
                Ok(format!("{keyword} {name}{annotation} = {init};"))
            }
            Stmt::Expr(e) => self.expression_statement(*e),
            Stmt::Contribute { branch, quantity, addends } => match &self.sums {
                Some(sums) => self.contribution(sums, *branch, *quantity, addends),
                None => Err(Error::Unsupported {
                    form: "contribution".into(),
                    path: format!("{} s{}", self.path, s.index()),
                    origin: addends.first().map_or_else(|| self.origin.clone(), |a| self.ir.expr_origin(a.value).clone()),
                }),
            },
            Stmt::Assign { place, op, value } => self.assign(place, *op, *value),
            Stmt::BoundStep { value, .. } => Err(self.unsupported(*value, "`bound_step` outside the top level of the `analog` block")),
            Stmt::Emit { stream, fields } => self.emit(s, *stream, fields),
            Stmt::Commit { carrier, value, after } => {
                if !self.commits {
                    return Err(Error::Unsupported {
                        form: "commit".into(),
                        path: format!("{} s{}", self.path, s.index()),
                        origin: self.ir.expr_origin(*value).clone(),
                    });
                }
                if self.carriers.get(*carrier as usize).is_none() {
                    return Err(self.unsupported(*value, "a commit to a carrier out of range"));
                }
                let value = self.expr(*value)?;
                let statement = StatementPlan::commit(self.ir, *carrier, *after);
                let after = match after {
                    Some(a) => self.expr(*a)?,
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
                    let (value, after) = (Self::indent(&value), Self::indent(&after));
                    let call = format!("commits_.commit({index}, value_, after_);");
                    return Ok(format!("{{\n    let value_ =\n{value};\n    let after_ =\n{after};\n    {call}\n}}"));
                }
                Ok(format!("commits_.commit({index}, {value}, {after});"))
            }
        }
    }

    /// A contribution of the structured form (`ContributionSums`): each addend added to its
    /// branch's sum, then the flag of its quantity set when the branch has one. An arm not taken
    /// never runs this, so it adds nothing (§11 rule 4).
    fn contribution(&self, sums: &ContributionSums, branch: u32, quantity: QuantityRole, addends: &[Addend]) -> Result<String, Error> {
        let mut lines = Vec::with_capacity(addends.len() + 2);
        if let Some(first) = addends.first() {
            lines.push(sums.comment(branch, quantity, self.ir.expr_origin(first.value)));
        }
        for addend in addends {
            let sum = sums.sum(branch, quantity, addend.operator);
            let value = if addend.negated { self.negated(addend.value)? } else { self.right_of_add(addend.value)? };
            lines.push(format!("{sum} = {sum} + {value};"));
        }
        lines.extend(sums.flag(branch, quantity).map(|flag| format!("{flag} = true;")));
        Ok(lines.join("\n"))
    }

    /// An emit site (msl-digital A4): the record's bytes in its stream's layout (`RecordLayout`),
    /// all zero first so a field the site leaves out has its zero (abi.md C5), each value
    /// field written where the layout puts it, and the record committed through the site's
    /// statement. The text fields stay in the IR (`Body::record_site`): a comment names them.
    fn emit(&self, s: StmtId, stream: u32, fields: &[EmitField]) -> Result<String, Error> {
        let origin = fields
            .iter()
            .find_map(|f| match f {
                EmitField::Value { value, .. } => Some(self.ir.expr_origin(*value).clone()),
                EmitField::Text { .. } => None,
            })
            .unwrap_or_else(|| self.origin.clone());
        let unsupported = |form: &str| Error::Unsupported { form: form.to_string(), path: format!("{} s{}", self.path, s.index()), origin: origin.clone() };
        if !self.commits {
            return Err(unsupported("emit! outside a reaction"));
        }
        let Some(declared) = self.streams.get(stream as usize) else {
            return Err(unsupported("emit! to a stream out of range"));
        };
        let Some(layout) = RecordLayout::in_ir(self.ir, declared) else {
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
                    let mut value = self.expr(*value)?;
                    if value.contains("commits_") {
                        // a value that emits is evaluated before the record's own `commits_`
                        // calls, so that two never borrow it at once
                        let name = format!("field{field}_");
                        evaluated.push(format!("let {name} =\n{};", Self::indent(&value)));
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

    /// Every line of `text` four spaces deeper; empty lines stay empty.
    pub(super) fn indent(text: &str) -> String {
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
        let locals: Vec<Local> = params.iter().map(|(n, t)| Local { name: (*n).to_string(), ty: t.clone(), mutable: false }).collect();
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
    fn each_new_row_prints_as_its_method() {
        // CAT-01 to CAT-03 (Appendix H): each row of `f64` and of the integers prints as the
        // method of its MSL name on the receiver; `powi` takes its `i32` exponent as a Rust `i32`;
        // `unwrap` and `expect` are `rt::Unwrap::of` of the flag and the payload.
        let f = ValueType::F64;
        let (u8_, i8_, i32_, u32_) = (Ty::u(8), Ty::i(8), Ty::i(32), Ty::u(32));
        // (row, receiver, other operands, result type, text)
        type Row = (Method, ValueType, Vec<ValueType>, ValueType, &'static str);
        let rows: Vec<Row> = vec![
            (Method::Sin, f.clone(), vec![], f.clone(), "x.sin()"),
            (Method::Cos, f.clone(), vec![], f.clone(), "x.cos()"),
            (Method::Tan, f.clone(), vec![], f.clone(), "x.tan()"),
            (Method::Asin, f.clone(), vec![], f.clone(), "x.asin()"),
            (Method::Acos, f.clone(), vec![], f.clone(), "x.acos()"),
            (Method::Atan, f.clone(), vec![], f.clone(), "x.atan()"),
            (Method::Sinh, f.clone(), vec![], f.clone(), "x.sinh()"),
            (Method::Cosh, f.clone(), vec![], f.clone(), "x.cosh()"),
            (Method::Tanh, f.clone(), vec![], f.clone(), "x.tanh()"),
            (Method::Log10, f.clone(), vec![], f.clone(), "x.log10()"),
            (Method::Log2, f.clone(), vec![], f.clone(), "x.log2()"),
            (Method::Ceil, f.clone(), vec![], f.clone(), "x.ceil()"),
            (Method::Round, f.clone(), vec![], f.clone(), "x.round()"),
            (Method::Trunc, f.clone(), vec![], f.clone(), "x.trunc()"),
            (Method::Fract, f.clone(), vec![], f.clone(), "x.fract()"),
            (Method::Signum, f.clone(), vec![], f.clone(), "x.signum()"),
            (Method::IsNan, f.clone(), vec![], ValueType::Bool, "x.is_nan()"),
            (Method::IsFinite, f.clone(), vec![], ValueType::Bool, "x.is_finite()"),
            (Method::IsInfinite, f.clone(), vec![], ValueType::Bool, "x.is_infinite()"),
            (Method::Atan2, f.clone(), vec![f.clone()], f.clone(), "x.atan2(y)"),
            (Method::Powf, f.clone(), vec![f.clone()], f.clone(), "x.powf(y)"),
            (Method::Hypot, f.clone(), vec![f.clone()], f.clone(), "x.hypot(y)"),
            (Method::RemEuclid, f.clone(), vec![f.clone()], f.clone(), "x.rem_euclid(y)"),
            (Method::Powi, f.clone(), vec![i32_.clone()], f.clone(), "x.powi(y.get() as i32)"),
            (Method::IntAbs, i8_.clone(), vec![], i8_.clone(), "x.abs()"),
            (Method::Signum, i8_.clone(), vec![], i8_.clone(), "x.signum()"),
            (Method::Pow, u8_.clone(), vec![u32_.clone()], u8_.clone(), "x.pow(y)"),
            (Method::RemEuclid, u8_.clone(), vec![u8_.clone()], u8_.clone(), "x.rem_euclid(y)"),
            (Method::SaturatingAdd, u8_.clone(), vec![u8_.clone()], u8_.clone(), "x.saturating_add(y)"),
            (Method::SaturatingSub, i8_.clone(), vec![i8_.clone()], i8_.clone(), "x.saturating_sub(y)"),
            (Method::SaturatingMul, u8_.clone(), vec![u8_.clone()], u8_.clone(), "x.saturating_mul(y)"),
            (Method::AddOverflows, u8_.clone(), vec![u8_.clone()], ValueType::Bool, "x.add_overflows(y)"),
            (Method::SubOverflows, i8_.clone(), vec![i8_.clone()], ValueType::Bool, "x.sub_overflows(y)"),
            (Method::MulOverflows, u8_.clone(), vec![u8_.clone()], ValueType::Bool, "x.mul_overflows(y)"),
        ];
        let mut b = IrBuilder::new();
        for (k, (method, recv, others, result, ..)) in rows.iter().enumerate() {
            let mut params = vec![("x", recv.clone())];
            params.extend(others.iter().enumerate().map(|(i, t)| (["y", "z"][i], t.clone())));
            defined(&mut b, &format!("row{k}"), &params, result.clone(), |x| {
                let receiver = x.local(0, recv.clone());
                let args: Vec<ExprId> = others.iter().enumerate().map(|(i, t)| x.local(i as u32 + 1, t.clone())).collect();
                x.method(*method, receiver, args, result.clone())
            });
        }
        // A literal exponent prints as an `i32` literal (analog code has no `rt::Int`).
        defined(&mut b, "powi_literal", &[("x", f.clone())], f.clone(), |x| {
            let receiver = x.local(0, ValueType::F64);
            let exponent = x.int(3, Ty::i(32));
            x.method(Method::Powi, receiver, vec![exponent], ValueType::F64)
        });
        // `unwrap` and `expect` of the pair: `rt::Unwrap::of(is, value, message)`.
        defined(&mut b, "unwrapped", &[("is", ValueType::Bool), ("v", u8_.clone())], u8_.clone(), |x| {
            let (is, v) = (x.local(0, ValueType::Bool), x.local(1, Ty::u(8)));
            let o = x.origin();
            x.b.expr(Expr::Unwrap { is, value: v, message: None }, Ty::u(8), o)
        });
        defined(&mut b, "expected", &[("is", ValueType::Bool), ("v", u8_.clone())], u8_.clone(), |x| {
            let (is, v) = (x.local(0, ValueType::Bool), x.local(1, Ty::u(8)));
            let o = x.origin();
            x.b.expr(Expr::Unwrap { is, value: v, message: Some("need the \"reading\"".to_string()) }, Ty::u(8), o)
        });
        let ir = Alone::finish(b);
        for (k, (method, _, _, _, want)) in rows.iter().enumerate() {
            let (name, text) = (format!("row{k}"), function(&ir, &format!("row{k}")));
            assert!(text.lines().any(|l| l.trim() == *want), "{method:?}: `{want}` is the body of {name}:\n{text}");
        }
        assert!(function(&ir, "powi_literal").lines().any(|l| l.trim() == "x.powi(3_i32)"), "{}", function(&ir, "powi_literal"));
        assert!(function(&ir, "unwrapped").contains("rt::Unwrap::of(is, v, None)"), "{}", function(&ir, "unwrapped"));
        assert!(function(&ir, "expected").contains("rt::Unwrap::of(is, v, Some(\"need the \\\"reading\\\"\"))"), "{}", function(&ir, "expected"));
    }

    #[test]
    fn integer_expressions_print() {
        let mut b = IrBuilder::new();
        let rev3 = Buck::rev3(&mut b);
        Buck::dithered_width(&mut b, rev3);
        // §1 rule 6: `-(a % b)` on `i16`; a cast from an integer; a real cast to an integer takes
        // the real's value; `min` and `clamp`
        defined(&mut b, "negated_rem", &[("a", Ty::i(16)), ("b", Ty::i(16))], Ty::i(16), |x| {
            let (a, c) = (x.local(0, Ty::i(16)), x.local(1, Ty::i(16)));
            let rem = x.bin(BinaryOp::Rem, a, c, Ty::i(16));
            x.un(UnaryOp::Neg, rem, Ty::i(16))
        });
        defined(&mut b, "widen", &[("x", Ty::u(10))], Ty::i(16), |x| {
            let v = x.local(0, Ty::u(10));
            x.cast(v, Ty::i(16), Ty::i(16))
        });
        defined(&mut b, "truncated", &[("x", ValueType::F64)], Ty::u(10), |x| {
            let v = x.local(0, ValueType::F64);
            x.cast(v, Ty::u(10), Ty::u(10))
        });
        defined(&mut b, "capped", &[("x", Ty::u(8))], Ty::u(8), |x| {
            let v = x.local(0, Ty::u(8));
            let cap = x.int(168, Ty::u(8));
            x.method(Method::Min, v, vec![cap], Ty::u(8))
        });
        defined(&mut b, "held", &[("x", Ty::i(32))], Ty::i(32), |x| {
            let v = x.local(0, Ty::i(32));
            let (lo, hi) = (x.int(0, Ty::i(32)), x.int(2880, Ty::i(32)));
            x.method(Method::Clamp, v, vec![lo, hi], Ty::i(32))
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

        // the integer operators of §1
        let text = |name: &str| function(&ir, name).lines().skip(1).collect::<Vec<_>>().join("\n");
        assert_eq!(text("negated_rem"), "fn negated_rem(a: rt::Int<16>, b: rt::Int<16>) -> rt::Int<16> {\n    -(a % b)\n}");
        assert_eq!(text("widen"), "fn widen(x: rt::UInt<10>) -> rt::Int<16> {\n    x.to_int::<16>()\n}");
        assert_eq!(
            text("truncated"),
            "fn truncated<R: Real>(x: R) -> rt::UInt<10> {\n    rt::UInt::<10>::from_f64(x.value())\n}"
        );
        assert_eq!(text("capped"), "fn capped(x: rt::UInt<8>) -> rt::UInt<8> {\n    x.min(rt::UInt::<8>::new(168))\n}");
        assert_eq!(text("held"), "fn held(x: rt::Int<32>) -> rt::Int<32> {\n    x.clamp(rt::Int::<32>::new(0), rt::Int::<32>::new(2880))\n}");
    }

    #[test]
    fn a_type_rust_lacks_is_refused_by_name() {
        // RS-Q6: `UInt<N>` and `Int<N>` hold 1..=64 bits; the ABI's leaves end at 64. (The IR has
        // no `Float<16>` or `Float<128>`: its only real is `f64`, so no `Ir` holds one to refuse.)
        let at = Origin { file: "dpwm.msl".into(), line: 1, col: 1 };
        for (ty, name) in [(Ty::u(65), "UInt<65>"), (Ty::i(65), "Int<65>"), (Ty::u(0), "UInt<0>")] {
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
