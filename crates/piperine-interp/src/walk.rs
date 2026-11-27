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
    Body, ExprId, Ir, Terminal, 
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
    /// Whether the run set its temperature (`temperature()` is `Some`): a flag, not a number.
    pub temperature_given: Option<bool>,
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
    /// Adds `v` into the slot, or fills it.
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
            | Mode::DerivativeAbstol(_)
            | Mode::DelayMaximum(_)
            | Mode::Noise(_) => {
                Err(Fault::new(Self::UNEXECUTED, format!("{}: the walk of {mode:?} ended without reaching its site", body.name())))
            }
        }
    }

    /// The sums the walk built, and the frame it read.
    pub(crate) fn finish(self) -> Option<(Sums, Frame)> {
        self.analog.map(|a| (a.sums, a.frame))
    }
}

mod diagnostics;
mod expressions;
mod sites;
mod statements;


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
