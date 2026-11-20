//! The walk: evaluation of IR code over [`Dual`] numbers. Constants, locals, params, operators
//! (`&&`/`||` short-circuit), `if`, blocks, `let`, calls, `Math`, and closures called with plain
//! reals. An analog walk (`Walk::analog`) runs a body's `analog` block over a [`Frame`] of the
//! values its function reads, and adds probes, environment reads, the sites and the
//! contributions; a value walk (`Walk::new`) faults on those, naming the form and its origin.
//!
//! A cell the running function does not read cannot reach what it writes (its dependency set,
//! `msl-ir` IR-09, is the reason it is not read), yet the walk may pass code that reads it: a
//! `let` only another site uses, a dead `if` over a param. Such a read is [`Value::Unread`], a
//! poison that never steers the walk: an operator, a method or a call over it gives `Unread`,
//! an `if`, `&&` or `||` whose condition is `Unread` runs no arm and gives `Unread`, and every
//! place that needs a number or a decision (a contribution, a site's arguments, a `where`, a
//! term) faults on it. Contributions sit at the top level and sites unconditionally
//! (`Ir::check`), so an arm skipped this way holds no write; a wrong dependency set faults
//! instead of choosing a branch.

use piperine_abi::device::Fault;
use piperine_ir::{
    Body, BinaryOp, Env, Expr, ExprId, FunctionId, Ir, MathFn, Operator, Origin, Stmt, StmtId, Terminal, UnaryOp,
};

use crate::dual::Dual;
use piperine_plan::Mode;

/// What an expression evaluates to.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(Dual),
    Bool(bool),
    Unit,
    /// A cell the running function does not declare as a read, or a value computed from one.
    Unread,
    /// A closure expression; called with plain reals through [`Walk::call_closure`].
    Closure(ExprId),
}

/// The values an analog walk reads: one per body node, branch flow pin, site and ambient. `None`
/// is a cell the running function does not read.
#[derive(Debug, Clone, Default)]
pub(crate) struct Frame {
    pub nodes: Vec<Option<Dual>>,
    /// The private flow pin of each branch (potential branches only).
    pub flows: Vec<Option<Dual>>,
    pub limits: Vec<Option<Dual>>,
    pub stims: Vec<Option<Dual>>,
    pub temperature: Option<Dual>,
    pub gmin: Option<Dual>,
    pub source_scale: Option<Dual>,
}

impl Frame {
    /// A read value, or `None` when the function does not read the cell.
    fn read(cell: Option<&Option<Dual>>) -> Option<Dual> {
        cell.and_then(|c| c.clone())
    }

    /// The potential of a terminal: its node's pin, or 0 for ground.
    pub fn potential(&self, t: Terminal) -> Option<Dual> {
        match t {
            Terminal::Node(i) => Self::read(self.nodes.get(i as usize)),
            Terminal::Ground => Some(Dual::constant(0.0)),
            Terminal::Own(_) => panic!("Ir::check: an own terminal belongs in a child's node slot only"),
        }
    }

    /// `plus − minus`: what a probe reads.
    pub fn probe(&self, plus: Terminal, minus: Terminal) -> Option<Dual> {
        Some(self.potential(plus)? - self.potential(minus)?)
    }

    /// The private flow pin of branch `b`.
    pub fn flow(&self, b: u32) -> Option<Dual> {
        Self::read(self.flows.get(b as usize))
    }
}

/// The sums a `Terms` walk builds: per branch, its direct and its `ddt` addends, each signed.
#[derive(Debug, Clone, Default)]
pub(crate) struct Sums {
    pub direct: Vec<Option<Dual>>,
    pub ddt: Vec<Option<Dual>>,
}

impl Sums {
    fn add(slot: &mut Option<Dual>, v: Dual) {
        *slot = Some(match slot.take() {
            Some(acc) => acc + v,
            None => v,
        });
    }
}

/// What a `limit` site's function reads besides its dependency set (abi.md §4.1, A2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Held {
    /// `LAST`: the value held.
    pub last: f64,
    /// `AGE`: 0 none held, 1 from the last accepted solution, 2 from the previous iteration.
    pub age: u8,
    /// The ambient `STATIC`: an equilibrium solve.
    pub is_static: bool,
}

/// What the function of a `limit` site writes: `x` with its tangent, and `f(x, old)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Limited {
    pub proposed: Dual,
    pub limited: f64,
}

/// What an analog walk runs over.
struct Analog<'a> {
    body: Body<'a>,
    mode: Mode,
    frame: Frame,
    sums: Sums,
    held: Option<Held>,
    limited: Option<Limited>,
    /// The `(RE, IM)` a `Stim` walk wrote at its site.
    phasor: Option<(f64, f64)>,
}

/// One evaluation: the params of the body, and a stack of local frames (a call pushes one).
pub struct Walk<'a> {
    ir: &'a Ir,
    params: Vec<Value>,
    frames: Vec<Vec<Value>>,
    analog: Option<Analog<'a>>,
}

impl<'a> Walk<'a> {
    /// The fault code of a form or a type the walk does not execute.
    pub const UNEXECUTED: u32 = 1;

    /// The fault code of a number or a decision taken from a cell the function does not read.
    pub const UNDECLARED: u32 = 3;

    /// A walk with `params` bound and one frame of `locals` unset (`Unit`) locals.
    pub fn new(ir: &'a Ir, params: Vec<Value>, locals: usize) -> Walk<'a> {
        Walk { ir, params, frames: vec![vec![Value::Unit; locals]], analog: None }
    }

    /// An analog walk of `body` in `mode`, over the reads in `frame` and `params` (one per param
    /// of the body; an unread param is `Unread`).
    pub(crate) fn analog(body: Body<'a>, mode: Mode, frame: Frame, params: Vec<Value>) -> Walk<'a> {
        let branches = body.branches().len();
        let sums = Sums { direct: vec![None; branches], ddt: vec![None; branches] };
        let locals = body.locals().len();
        Walk {
            ir: body.ir(),
            params,
            frames: vec![vec![Value::Unit; locals]],
            analog: Some(Analog { body, mode, frame, sums, held: None, limited: None, phasor: None }),
        }
    }

    /// The `LAST`, `AGE` and `STATIC` a `Limit` walk reads.
    pub(crate) fn hold(&mut self, held: Held) {
        if let Some(a) = self.analog.as_mut() {
            a.held = Some(held);
        }
    }

    /// The fault code that ends a site walk once its site has written: `run` turns it into
    /// success and it never leaves the walk.
    const DONE: u32 = u32::MAX;

    /// Runs the `analog` block once, statement by statement. A site walk stops at its site
    /// (sites sit unconditionally at the top of the block, `Ir::check`); one that ends without
    /// reaching it faults.
    pub(crate) fn run(&mut self) -> Result<(), Fault> {
        let Some(analog) = &self.analog else {
            return Err(Fault::new(Self::UNEXECUTED, "a value walk has no `analog` block to run"));
        };
        let (body, mode) = (analog.body, analog.mode);
        for id in body.analog().unwrap_or(&[]) {
            match self.exec(*id) {
                Ok(()) => {}
                Err(f) if f.code == Self::DONE && self.analog.as_ref().is_some_and(|a| a.limited.is_some() || a.phasor.is_some()) => return Ok(()),
                Err(f) => return Err(f),
            }
        }
        match mode {
            Mode::Terms => Ok(()),
            Mode::Limit(_)
            | Mode::Stim(_)
            | Mode::Check(_)
            | Mode::Cross(_)
            | Mode::Drive(_)
            | Mode::Event(_)
            | Mode::DelayedEvent(_)
            | Mode::Rearm { .. }
            | Mode::Cap(_)
            | Mode::Ttol(_)
            | Mode::Etol(_)
            | Mode::DerivativeAbstol(_) => {
                Err(Fault::new(Self::UNEXECUTED, format!("{}: the walk of {mode:?} ended without reaching its site", body.name())))
            }
        }
    }

    /// What a `Stim` walk wrote at its site: `(RE, IM)`.
    pub(crate) fn phasor(&self) -> Option<(f64, f64)> {
        self.analog.as_ref().and_then(|a| a.phasor)
    }

    /// The site of a `Stim(k)` walk: `mag` and `phase` over plain reals; `RE = mag·cos(phase)`,
    /// `IM = mag·sin(phase)`, and exactly `RE = mag`, `IM = 0` with no phase. Ends the walk.
    fn stim_site(&mut self, mag: ExprId, phase: Option<ExprId>) -> Result<Value, Fault> {
        let mag = self.eval_real(mag)?.v;
        let phasor = match phase {
            Some(p) => {
                let phase = self.eval_real(p)?.v;
                (mag * phase.cos(), mag * phase.sin())
            }
            None => (mag, 0.0),
        };
        if let Some(a) = self.analog.as_mut() {
            a.phasor = Some(phasor);
        }
        Err(Fault::new(Self::DONE, "the site has written"))
    }

    /// What a `Limit` walk wrote at its site.
    pub(crate) fn limited(&self) -> Option<&Limited> {
        self.analog.as_ref().and_then(|a| a.limited.as_ref())
    }

    /// The site of a `Limit(k)` walk (§18 rules 1 and 2, abi.md A2): `x` over duals, `old` the
    /// seed (or `x` with no seed) when `AGE` is 0 or `STATIC` and `AGE` is 1, else `LAST`;
    /// `LIMITED = f(x, old)` over plain reals, raised to the seed when it starts from the seed
    /// and falls below it. Ends the walk.
    fn limit_site(&mut self, id: ExprId, x: ExprId, f: ExprId, seed: Option<ExprId>) -> Result<Value, Fault> {
        let held = match self.analog.as_ref().and_then(|a| a.held) {
            Some(h) => h,
            None => return Err(self.fault(id, "a `limit` walk without its LAST, AGE and STATIC")),
        };
        let x = self.eval_real(x)?;
        let Value::Closure(closure) = self.eval(f)? else {
            return Err(self.fault(f, "the function of a `limit` is not a closure"));
        };
        let seed = match seed {
            Some(s) => Some(self.eval_real(s)?.v),
            None => None,
        };
        let from_seed = held.age == 0 || (held.is_static && held.age == 1);
        let old = if from_seed { seed.unwrap_or(x.v) } else { held.last };
        let mut limited = self.call_closure(closure, &[x.v, old])?;
        if let (true, Some(s)) = (from_seed, seed) {
            if limited < s {
                limited = s;
            }
        }
        if let Some(a) = self.analog.as_mut() {
            a.limited = Some(Limited { proposed: x, limited });
        }
        Err(Fault::new(Self::DONE, "the site has written"))
    }

    /// The sums the walk built, and the frame it read.
    pub(crate) fn finish(self) -> Option<(Sums, Frame)> {
        self.analog.map(|a| (a.sums, a.frame))
    }

    fn origin(origin: &Origin) -> String {
        format!("{}:{}:{}", origin.file, origin.line, origin.col)
    }

    fn fault(&self, id: ExprId, what: &str) -> Fault {
        Fault::new(Self::UNEXECUTED, format!("{what} (at {})", Self::origin(self.ir.expr_origin(id))))
    }

    /// The name of an expression form, for diagnostics.
    fn form(expr: &Expr) -> String {
        match expr {
            // The forms msl-fixes added are named as the source spells them.
            Expr::Method { method, .. } => format!("method `{}`", method.name()),
            Expr::Env(Env::Freq) => "form `freq()`".to_string(),
            Expr::Env(Env::Analysis(kind)) => format!("form `analysis({kind:?})`"),
            Expr::Fired(_) => "form `fired`".to_string(),
            Expr::Unwrap { message: Some(_), .. } => "form `expect`".to_string(),
            Expr::Unwrap { message: None, .. } => "form `unwrap`".to_string(),
            Expr::Level { .. } => "form `level`".to_string(),
            Expr::Merge { .. } => "form `merge`".to_string(),
            Expr::Same { .. } => "form `same`".to_string(),
            Expr::LevelCast { .. } => "form `cast`".to_string(),
            other => format!("form `{}`", Self::form_name(other)),
        }
    }

    /// The variant name of an expression form.
    fn form_name(expr: &Expr) -> &'static str {
        match expr {
            Expr::Real(_) => "Real",
            Expr::Bool(_) => "Bool",
            Expr::Unit => "Unit",
            Expr::Local(_) => "Local",
            Expr::Param(_) => "Param",
            Expr::Unary { .. } => "Unary",
            Expr::Binary { .. } => "Binary",
            Expr::If { .. } => "If",
            Expr::Block { .. } => "Block",
            Expr::Call { .. } => "Call",
            Expr::Math { .. } => "Math",
            Expr::Closure { .. } => "Closure",
            Expr::Probe { .. } => "Probe",
            Expr::Env(_) => "Env",
            Expr::Limit { .. } => "Limit",
            Expr::Stim { .. } => "Stim",
            Expr::Int(_) => "Int",
            Expr::Cast { .. } => "Cast",
            Expr::Method { .. } => "Method",
            Expr::Unknown => "Unknown",
            Expr::KnownOr { .. } => "KnownOr",
            Expr::IsKnown(_) => "IsKnown",
            Expr::High(_) => "High",
            Expr::Carrier(_) => "Carrier",
            Expr::Flow { .. } => "Flow",
            Expr::Derivative(_) => "Derivative",
            Expr::ParamPrevious(_) => "ParamPrevious",
            Expr::Previous(_) => "Previous",
            Expr::Fired(_) => "Fired",
            Expr::Unwrap { .. } => "Unwrap",
            Expr::Level { .. } => "Level",
            Expr::Merge { .. } => "Merge",
            Expr::Same { .. } => "Same",
            Expr::LevelCast { .. } => "LevelCast",
            Expr::Loop { .. } => "Loop",
            Expr::While { .. } => "While",
            Expr::ForRange { .. } => "ForRange",
            Expr::Break(_) => "Break",
            Expr::Continue => "Continue",
        }
    }

    fn locals(&mut self) -> &mut Vec<Value> {
        self.frames.last_mut().expect("a walk always has a frame")
    }

    /// Evaluates `id` and requires a real; `Unread` faults (`UNDECLARED`).
    pub fn eval_real(&mut self, id: ExprId) -> Result<Dual, Fault> {
        match self.eval(id)? {
            Value::Real(d) => Ok(d),
            Value::Unread => Err(self.undeclared(id)),
            other => Err(self.fault(id, &format!("expected a real, found {}", Self::kind(&other)))),
        }
    }

    /// Evaluates `id` and requires a bool; `Unread` faults (`UNDECLARED`).
    pub fn eval_bool(&mut self, id: ExprId) -> Result<bool, Fault> {
        match self.eval(id)? {
            Value::Bool(b) => Ok(b),
            Value::Unread => Err(self.undeclared(id)),
            other => Err(self.fault(id, &format!("expected a bool, found {}", Self::kind(&other)))),
        }
    }

    /// A real or `Unread`; any other value faults.
    fn real_or_unread(&mut self, id: ExprId) -> Result<Option<Dual>, Fault> {
        match self.eval(id)? {
            Value::Real(d) => Ok(Some(d)),
            Value::Unread => Ok(None),
            other => Err(self.fault(id, &format!("expected a real, found {}", Self::kind(&other)))),
        }
    }

    /// A bool or `Unread`; any other value faults.
    fn bool_or_unread(&mut self, id: ExprId) -> Result<Option<bool>, Fault> {
        match self.eval(id)? {
            Value::Bool(b) => Ok(Some(b)),
            Value::Unread => Ok(None),
            other => Err(self.fault(id, &format!("expected a bool, found {}", Self::kind(&other)))),
        }
    }

    fn undeclared(&self, id: ExprId) -> Fault {
        let origin = Self::origin(self.ir.expr_origin(id));
        Fault::new(
            Self::UNDECLARED,
            format!("a value is taken from a cell the function does not read (a wrong dependency set, at {origin})"),
        )
    }

    fn kind(value: &Value) -> &'static str {
        match value {
            Value::Real(_) => "a real",
            Value::Bool(_) => "a bool",
            Value::Unit => "a unit",
            Value::Unread => "a cell the function does not read",
            Value::Closure(_) => "a closure (only `limit` calls one)",
        }
    }

    /// Evaluates one expression.
    pub fn eval(&mut self, id: ExprId) -> Result<Value, Fault> {
        if let Some(v) = self.read_form(id) {
            return Ok(v);
        }
        let ir = self.ir;
        match ir.expr(id) {
            Expr::Real(r) => Ok(Value::Real(Dual::constant(r.get()))),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Unit => Ok(Value::Unit),
            Expr::Local(i) => Ok(self.locals()[*i as usize].clone()),
            Expr::Param(i) => match self.params.get(*i as usize) {
                Some(v) => Ok(v.clone()),
                None => Err(self.fault(id, &format!("param {i} is not bound"))),
            },
            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => Ok(self.real_or_unread(*expr)?.map_or(Value::Unread, |x| Value::Real(-x))),
                UnaryOp::Not => Ok(self.bool_or_unread(*expr)?.map_or(Value::Unread, |b| Value::Bool(!b))),
            },
            Expr::Binary { op, lhs, rhs } => self.binary(id, *op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => match self.bool_or_unread(*cond)? {
                Some(true) => self.eval(*then),
                Some(false) => self.eval(*otherwise),
                None => Ok(Value::Unread),
            },
            Expr::Block { stmts, value } => {
                for s in stmts {
                    self.exec(*s)?;
                }
                match value {
                    Some(v) => self.eval(*v),
                    None => Ok(Value::Unit),
                }
            }
            Expr::Call { function, args } => {
                let mut values = Vec::with_capacity(args.len());
                for a in args {
                    values.push(self.eval(*a)?);
                }
                self.call(*function, values)
            }
            Expr::Math { f, args } => {
                let Some(first) = args.first() else {
                    return Err(self.fault(id, "a math function with no argument"));
                };
                let Some(x) = self.real_or_unread(*first)? else { return Ok(Value::Unread) };
                Ok(Value::Real(match f {
                    MathFn::Exp => x.exp(),
                    MathFn::Ln => x.ln(),
                    MathFn::Sqrt => x.sqrt(),
                    MathFn::Abs => x.abs(),
                }))
            }
            Expr::Closure { .. } => Ok(Value::Closure(id)),
            Expr::Stim { site, mag, phase } if self.mode() == Some(Mode::Stim(*site)) => self.stim_site(*mag, *phase),
            Expr::Limit { site, x, f, seed } if self.mode() == Some(Mode::Limit(*site)) => {
                self.limit_site(id, *x, *f, *seed)
            }
            other => Err(self.fault(id, &format!("the IR {} is not executed by this walk", Self::form(other)))),
        }
    }

    fn mode(&self) -> Option<Mode> {
        self.analog.as_ref().map(|a| a.mode)
    }

    /// The value of an analog read form (probe, environment, a site read in `Terms`), `Unread`
    /// when the function does not read its cell, or `None` when `id` is not one this walk
    /// answers from its frame.
    fn read_form(&self, id: ExprId) -> Option<Value> {
        let analog = self.analog.as_ref()?;
        let f = &analog.frame;
        let read = match self.ir.expr(id) {
            Expr::Probe { plus, minus } => f.probe(*plus, *minus),
            Expr::Env(env) => Frame::read(Some(match env {
                piperine_ir::Env::Temperature => &f.temperature,
                piperine_ir::Env::Gmin => &f.gmin,
                piperine_ir::Env::SourceScale => &f.source_scale,
                // Not a frame cell: `eval` faults on it (`UNEXECUTED`), and `Interpreter::new`
                // rejects a body that holds one.
                piperine_ir::Env::Time | piperine_ir::Env::Static | piperine_ir::Env::Freq | piperine_ir::Env::Analysis(_) => return None,
            })),
            // §18 rule 3, abi.md A2: the held value stands in for `x`; `x`, `f` and the seed are
            // the site function's, not evaluated here.
            // Any other site's function reads the site's value too, never its arguments.
            Expr::Limit { site, .. } if analog.mode != Mode::Limit(*site) => Frame::read(f.limits.get(*site as usize)),
            Expr::Stim { site, .. } if analog.mode != Mode::Stim(*site) => Frame::read(f.stims.get(*site as usize)),
            _ => return None,
        };
        Some(read.map_or(Value::Unread, Value::Real))
    }

    fn binary(&mut self, id: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Value, Fault> {
        match op {
            // An `Unread` left side decides nothing: the right side is not run.
            BinaryOp::And | BinaryOp::Or => {
                let Some(left) = self.bool_or_unread(lhs)? else { return Ok(Value::Unread) };
                if left == (op == BinaryOp::Or) {
                    return Ok(Value::Bool(left));
                }
                Ok(self.bool_or_unread(rhs)?.map_or(Value::Unread, Value::Bool))
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let (l, r) = (self.eval(lhs)?, self.eval(rhs)?);
                let same = match (&l, &r) {
                    (Value::Unread, Value::Real(_) | Value::Bool(_) | Value::Unread)
                    | (Value::Real(_) | Value::Bool(_), Value::Unread) => return Ok(Value::Unread),
                    (Value::Real(a), Value::Real(b)) => a.v == b.v,
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    _ => return Err(self.fault(id, "equality needs two reals or two bools")),
                };
                Ok(Value::Bool(same == (op == BinaryOp::Eq)))
            }
            _ => {
                let (a, b) = (self.real_or_unread(lhs)?, self.real_or_unread(rhs)?);
                let (Some(a), Some(b)) = (a, b) else { return Ok(Value::Unread) };
                Ok(match op {
                    BinaryOp::Add => Value::Real(a + b),
                    BinaryOp::Sub => Value::Real(a - b),
                    BinaryOp::Mul => Value::Real(a * b),
                    BinaryOp::Div => Value::Real(a / b),
                    BinaryOp::Lt => Value::Bool(a.v < b.v),
                    BinaryOp::Le => Value::Bool(a.v <= b.v),
                    BinaryOp::Gt => Value::Bool(a.v > b.v),
                    BinaryOp::Ge => Value::Bool(a.v >= b.v),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::Eq | BinaryOp::Ne => {
                        return Err(self.fault(id, "unreachable operator"));
                    }
                    BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
                        return Err(self.fault(id, &format!("the IR operator `{op:?}` is not executed by this walk")));
                    }
                })
            }
        }
    }

    /// Runs one statement: `let`, expression statements, and in a `Terms` walk a contribution,
    /// each addend added with its sign to its branch's direct or `ddt` sum. Elsewhere a
    /// contribution faults.
    pub fn exec(&mut self, id: StmtId) -> Result<(), Fault> {
        let ir = self.ir;
        match ir.stmt(id) {
            Stmt::Let { local, init } => {
                let v = self.eval(*init)?;
                self.locals()[*local as usize] = v;
                Ok(())
            }
            Stmt::Expr(e) => self.eval(*e).map(|_| ()),
            Stmt::Contribute { branch, addends, .. } if self.mode() == Some(Mode::Terms) => {
                for a in addends {
                    let v = self.eval_real(a.value)?;
                    let v = if a.negated { -v } else { v };
                    let Some(analog) = self.analog.as_mut() else { break };
                    let sums = if a.operator == Operator::Direct { &mut analog.sums.direct } else { &mut analog.sums.ddt };
                    match sums.get_mut(*branch as usize) {
                        Some(slot) => Sums::add(slot, v),
                        None => return Err(self.fault(a.value, &format!("a contribution to branch {branch}, out of range"))),
                    }
                }
                Ok(())
            }
            // A site walk evaluates the addends only to reach a site inside them; what they read
            // past the site's own dependencies is `Unread` and decides nothing.
            Stmt::Contribute { addends, .. } if self.analog.is_some() => {
                for a in addends {
                    self.real_or_unread(a.value)?;
                }
                Ok(())
            }
            Stmt::Contribute { addends, .. } => {
                let at = addends.first().map_or_else(|| "<unknown>".to_string(), |a| Self::origin(ir.expr_origin(a.value)));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Contribute` is not executed by the value walk (at {at})")))
            }
            Stmt::Commit { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Commit` is not executed by this walk (at {at})")))
            }
            Stmt::Emit { .. } => Err(Fault::new(Self::UNEXECUTED, "the IR form `Emit` is not executed by this walk")),
            Stmt::Assign { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Assign` is not executed by this walk (at {at})")))
            }
            Stmt::BoundStep { value, .. } => {
                let at = Self::origin(ir.expr_origin(*value));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `bound_step` is not executed by this walk (at {at})")))
            }
        }
    }

    /// Calls `function` with `args` bound to its leading locals, in a fresh frame.
    pub fn call(&mut self, function: FunctionId, args: Vec<Value>) -> Result<Value, Fault> {
        let f = self.ir.function(function);
        let mut frame = vec![Value::Unit; f.locals().len().max(args.len())];
        for (slot, arg) in frame.iter_mut().zip(args) {
            *slot = arg;
        }
        self.frames.push(frame);
        let result = self.eval(f.body());
        self.frames.pop();
        result
    }

    /// Calls the closure expression `id` with plain reals, in the current frame: the closure sees
    /// the locals of the block that wrote it.
    pub fn call_closure(&mut self, id: ExprId, args: &[f64]) -> Result<f64, Fault> {
        let ir = self.ir;
        let Expr::Closure { params, body } = ir.expr(id) else {
            return Err(self.fault(id, "not a closure"));
        };
        if params.len() != args.len() {
            return Err(self.fault(id, &format!("a closure of {} params called with {}", params.len(), args.len())));
        }
        for (local, value) in params.iter().zip(args) {
            self.locals()[*local as usize] = Value::Real(Dual::constant(*value));
        }
        Ok(self.eval_real(*body)?.v)
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BinaryOp, Env, ExprId, FunctionSpec, Ir, IrBuilder, Local, MathFn, Origin, Stmt, Terminal, UnaryOp, ValueType};

    use super::{Value, Walk};
    use crate::devices::{Devices, Ex};

    /// A one-off IR holding hand-built expressions; the resistor top only completes the design.
    struct Lab {
        b: IrBuilder,
    }

    impl Lab {
        fn new() -> Lab {
            Lab { b: IrBuilder::new() }
        }

        fn ex(&mut self, line: u32, col: u32) -> Ex<'_> {
            Ex::new(&mut self.b, "t.msl", line, col)
        }

        fn finish(mut self) -> Ir {
            let body = Devices::resistor(&mut self.b);
            let top = Devices::alone(&mut self.b, body, "R", 1, vec![1.0]);
            self.b.finish(top)
        }
    }

    fn real(v: Value) -> f64 {
        match v {
            Value::Real(d) => d.v,
            other => panic!("expected a real, found {other:?}"),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-300)
    }

    #[test]
    fn f64_arithmetic_and_methods_match_rust() {
        let mut lab = Lab::new();
        let mut e = lab.ex(1, 1);
        let (a, b, c) = (e.real(1.7), e.real(-0.4), e.real(3.0));
        let sum = e.binary(BinaryOp::Add, a, b);
        let prod = e.binary(BinaryOp::Mul, sum, c);
        let q = e.real(0.7);
        let diff = e.binary(BinaryOp::Sub, prod, q);
        let quo = e.binary(BinaryOp::Div, diff, c);
        let neg = e.unary(UnaryOp::Neg, quo);
        let ex = e.math(MathFn::Exp, neg);
        let ln = e.math(MathFn::Ln, c);
        let sq = e.math(MathFn::Sqrt, c);
        let ab = e.math(MathFn::Abs, neg);
        let lt = e.binary(BinaryOp::Lt, a, c);
        let ge = e.binary(BinaryOp::Ge, a, c);
        let eq = e.binary(BinaryOp::Eq, a, a);
        let ne = e.binary(BinaryOp::Ne, a, b);
        let not = e.unary(UnaryOp::Not, ge);
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![], 0);
        let expect = -(((1.7_f64 + -0.4) * 3.0 - 0.7) / 3.0);
        assert!(close(real(w.eval(neg).unwrap()), expect));
        assert!(close(real(w.eval(ex).unwrap()), expect.exp()));
        assert!(close(real(w.eval(ln).unwrap()), 3.0_f64.ln()));
        assert!(close(real(w.eval(sq).unwrap()), 3.0_f64.sqrt()));
        assert!(close(real(w.eval(ab).unwrap()), expect.abs()));
        assert_eq!(w.eval(lt).unwrap(), Value::Bool(true));
        assert_eq!(w.eval(ge).unwrap(), Value::Bool(false));
        assert_eq!(w.eval(eq).unwrap(), Value::Bool(true));
        assert_eq!(w.eval(ne).unwrap(), Value::Bool(true));
        assert_eq!(w.eval(not).unwrap(), Value::Bool(true));
    }

    #[test]
    fn division_by_zero_is_ieee() {
        let mut lab = Lab::new();
        let mut e = lab.ex(1, 1);
        let (one, zero, minus) = (e.real(1.0), e.real(0.0), e.real(-1.0));
        let pos = e.binary(BinaryOp::Div, one, zero);
        let neg = e.binary(BinaryOp::Div, minus, zero);
        let nan = e.binary(BinaryOp::Div, zero, zero);
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![], 0);
        assert_eq!(real(w.eval(pos).unwrap()), f64::INFINITY);
        assert_eq!(real(w.eval(neg).unwrap()), f64::NEG_INFINITY);
        assert!(real(w.eval(nan).unwrap()).is_nan());
    }

    #[test]
    fn short_circuit_and_if_evaluate_one_side() {
        // The side that must not run is a probe, which this walk faults on.
        let mut lab = Lab::new();
        let mut e = lab.ex(1, 1);
        let (t, f) = (e.boolean(true), e.boolean(false));
        let probe = e.probe(Terminal::Node(0), Terminal::Ground);
        let one = e.real(1.0);
        let skip_and = e.binary(BinaryOp::And, f, probe);
        let skip_or = e.binary(BinaryOp::Or, t, probe);
        let run_and = e.binary(BinaryOp::And, t, probe);
        let taken = e.if_f64(t, one, probe);
        let other = e.if_f64(f, probe, one);
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![], 0);
        assert_eq!(w.eval(skip_and).unwrap(), Value::Bool(false));
        assert_eq!(w.eval(skip_or).unwrap(), Value::Bool(true));
        assert!(w.eval(run_and).is_err(), "`true && x` must evaluate x");
        assert_eq!(real(w.eval(taken).unwrap()), 1.0);
        assert_eq!(real(w.eval(other).unwrap()), 1.0);
    }

    #[test]
    fn an_unread_value_steers_nothing() {
        // Param 0 is a real, param 1 a bool, both unread. `if p0 > 1 { probe } else { 2 }` would
        // take the else arm on a NaN stand-in; on `Unread` it runs neither (the probe arm would
        // fault in this walk) and gives `Unread`, and so does every operator over it, a call's body
        // over its argument among them. A number or a decision taken from it faults.
        let mut lab = Lab::new();
        let mut e = lab.ex(1, 1);
        let p0 = e.param(0);
        let one = e.real(1.0);
        let two = e.real(2.0);
        let probe = e.probe(Terminal::Node(0), Terminal::Ground);
        let above = e.binary(BinaryOp::Gt, p0, one);
        let pick = e.if_f64(above, probe, two);
        let sum = e.binary(BinaryOp::Add, p0, one);
        let ex = e.math(MathFn::Exp, p0);
        let neg = e.unary(UnaryOp::Neg, p0);
        let eq = e.binary(BinaryOp::Eq, p0, one);
        let flag = lab.b.expr(piperine_ir::Expr::Param(1), ValueType::Bool, Origin { file: "t.msl".into(), line: 2, col: 1 });
        let mut e = lab.ex(2, 1);
        let f = e.boolean(false);
        let t = e.boolean(true);
        let and_left = e.binary(BinaryOp::And, flag, probe);
        let or_left = e.binary(BinaryOp::Or, flag, probe);
        let and_right = e.binary(BinaryOp::And, t, flag);
        let and_short = e.binary(BinaryOp::And, f, flag);
        let not = e.unary(UnaryOp::Not, flag);
        let on_flag = e.if_f64(flag, probe, two);
        // fn f(a) { if a > 1 { probe } else { 2 } }, called with the unread param.
        let mut e = lab.ex(3, 1);
        let a = e.local(0);
        let a_above = e.binary(BinaryOp::Gt, a, one);
        let body = e.if_f64(a_above, probe, two);
        let f_a = lab.b.function(FunctionSpec {
            name: "f".into(),
            origin: Origin { file: "t.msl".into(), line: 3, col: 1 },
            params: vec![Local { name: "a".into(), ty: ValueType::F64, mutable: false }],
            ret: ValueType::F64,
            locals: vec![Local { name: "a".into(), ty: ValueType::F64, mutable: false }],
            body,
        });
        let mut e = lab.ex(4, 1);
        let called = e.call(f_a, vec![p0]);
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![Value::Unread, Value::Unread], 0);
        assert!(matches!(ir.expr(called), piperine_ir::Expr::Call { .. }), "the call is an `Expr::Call`");
        for id in [above, pick, sum, ex, neg, eq, and_left, or_left, and_right, not, on_flag, called] {
            assert_eq!(w.eval(id).unwrap(), Value::Unread, "{id:?}");
        }
        assert_eq!(w.eval(and_short).unwrap(), Value::Bool(false), "a decided left side needs no right side");
        assert_eq!(w.eval_real(pick).unwrap_err().code, Walk::UNDECLARED);
        assert_eq!(w.eval_real(called).unwrap_err().code, Walk::UNDECLARED);
        assert_eq!(w.eval_bool(above).unwrap_err().code, Walk::UNDECLARED);
        assert_eq!(w.eval_bool(flag).unwrap_err().code, Walk::UNDECLARED);
    }

    /// `pnjlim` as the standard library writes it, in plain Rust.
    fn pnjlim(new: f64, old: f64, nvt: f64, vcrit: f64) -> f64 {
        if new > vcrit && (new - old).abs() > 2.0 * nvt {
            if old > 0.0 {
                let arg = (new - old) / nvt;
                if arg > 0.0 { old + nvt * (2.0 + (arg - 2.0).ln()) } else { old - nvt * (2.0 + (2.0 - arg).ln()) }
            } else {
                nvt * (new / nvt).ln()
            }
        } else if new < 0.0 {
            let arg = if old > 0.0 { -old - 1.0 } else { 2.0 * old - 1.0 };
            if new < arg { arg } else { new }
        } else {
            new
        }
    }

    #[test]
    fn calls_bind_parameters_and_closures_see_locals() {
        let mut lab = Lab::new();
        let pn = Devices::pnjlim(&mut lab.b);
        // fn sub(a, b) { a - b }: the order of the bound params matters.
        let mut e = lab.ex(2, 1);
        let (a, b) = (e.local(0), e.local(1));
        let body = e.binary(BinaryOp::Sub, a, b);
        let local = |n: &str| Local { name: n.into(), ty: ValueType::F64, mutable: false };
        let sub = lab.b.function(FunctionSpec {
            name: "sub".into(),
            origin: Origin { file: "t.msl".into(), line: 2, col: 1 },
            params: vec![local("a"), local("b")],
            ret: ValueType::F64,
            locals: vec![local("a"), local("b")],
            body,
        });
        let mut e = lab.ex(3, 1);
        let (five, two) = (e.real(5.0), e.real(2.0));
        let call = e.call(sub, vec![five, two]);
        // a closure over local 0, parameter in local 1
        let (l0, l1) = (e.local(0), e.local(1));
        let product = e.binary(BinaryOp::Mul, l0, l1);
        let closure = e.closure(vec![1], product);
        let three = e.real(3.0);
        let let_x = e.b.stmt(Stmt::Let { local: 0, init: three });
        // the six branch points of pnjlim
        let points = [
            (0.9, 0.7), // junction, old > 0, arg > 0
            (0.65, 0.9), // junction, old > 0, arg < 0
            (0.9, -0.1), // junction, old <= 0
            (-2.0, 0.5), // negative, below the floor
            (-0.5, -0.2), // negative, above the floor
            (0.3, 0.1), // untouched
        ];
        let mut calls = Vec::new();
        for (new, old) in points {
            let args: Vec<ExprId> = [new, old, 0.025, 0.6].iter().map(|v| e.real(*v)).collect();
            calls.push(e.call(pn, args));
        }
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![], 2);
        assert_eq!(real(w.eval(call).unwrap()), 3.0);
        w.exec(let_x).unwrap();
        assert_eq!(w.call_closure(closure, &[4.0]).unwrap(), 12.0);
        assert!(w.call_closure(closure, &[]).is_err());
        for ((new, old), id) in points.iter().zip(calls) {
            assert!(close(real(w.eval(id).unwrap()), pnjlim(*new, *old, 0.025, 0.6)), "pnjlim({new}, {old})");
        }
    }

    #[test]
    fn an_unexecuted_form_faults_with_its_origin() {
        let mut lab = Lab::new();
        let mut e = lab.ex(7, 9);
        let probe = e.probe(Terminal::Node(0), Terminal::Ground);
        let mut e = lab.ex(8, 3);
        let temp = e.env(Env::Temperature);
        let mut e = lab.ex(9, 5);
        let body = e.real(1.0);
        let closure = e.closure(vec![], body);
        let ir = lab.finish();
        let mut w = Walk::new(&ir, vec![], 0);
        let fault = w.eval(probe).unwrap_err();
        assert!(fault.message.contains("Probe") && fault.message.contains("t.msl:7:9"), "{}", fault.message);
        let fault = w.eval(temp).unwrap_err();
        assert!(fault.message.contains("Env") && fault.message.contains("t.msl:8:3"), "{}", fault.message);
        // a closure reached as a value outside a `limit` faults with its own origin
        let fault = w.eval_real(closure).unwrap_err();
        assert!(fault.message.contains("closure") && fault.message.contains("t.msl:9:5"), "{}", fault.message);
    }
}
