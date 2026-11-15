//! The value walk: evaluation of IR value code over [`Dual`] numbers. Constants, locals, params,
//! operators (`&&`/`||` short-circuit), `if`, blocks, `let`, calls, `Math`, and closures called
//! with plain reals. A form it does not execute here (probes, environment reads, `limit`,
//! `ac_stim` sites, contributions) is a `Fault` naming the form and its origin: the analog
//! modes of the interpreter add those.

use piperine_abi::device::Fault;
use piperine_ir::{BinaryOp, Expr, ExprId, FunctionId, Ir, MathFn, Origin, Stmt, StmtId, UnaryOp};

use crate::dual::Dual;

/// What an expression evaluates to.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Real(Dual),
    Bool(bool),
    Unit,
    /// A closure expression; called with plain reals through [`Walk::call_closure`].
    Closure(ExprId),
}

/// One evaluation: the params of the body, and a stack of local frames (a call pushes one).
pub struct Walk<'a> {
    ir: &'a Ir,
    params: Vec<Value>,
    frames: Vec<Vec<Value>>,
}

impl<'a> Walk<'a> {
    /// The fault code of a form or a type the walk does not execute.
    pub const UNEXECUTED: u32 = 1;

    /// A walk with `params` bound and one frame of `locals` unset (`Unit`) locals.
    pub fn new(ir: &'a Ir, params: Vec<Value>, locals: usize) -> Walk<'a> {
        Walk { ir, params, frames: vec![vec![Value::Unit; locals]] }
    }

    fn origin(origin: &Origin) -> String {
        format!("{}:{}:{}", origin.file, origin.line, origin.col)
    }

    fn fault(&self, id: ExprId, what: &str) -> Fault {
        Fault::new(Self::UNEXECUTED, format!("{what} (at {})", Self::origin(self.ir.expr_origin(id))))
    }

    /// The name of an expression form, for diagnostics.
    fn form(expr: &Expr) -> &'static str {
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
        }
    }

    fn frame(&mut self) -> &mut Vec<Value> {
        self.frames.last_mut().expect("a walk always has a frame")
    }

    /// Evaluates `id` and requires a real.
    pub fn eval_real(&mut self, id: ExprId) -> Result<Dual, Fault> {
        match self.eval(id)? {
            Value::Real(d) => Ok(d),
            other => Err(self.fault(id, &format!("expected a real, found {}", Self::kind(&other)))),
        }
    }

    /// Evaluates `id` and requires a bool.
    pub fn eval_bool(&mut self, id: ExprId) -> Result<bool, Fault> {
        match self.eval(id)? {
            Value::Bool(b) => Ok(b),
            other => Err(self.fault(id, &format!("expected a bool, found {}", Self::kind(&other)))),
        }
    }

    fn kind(value: &Value) -> &'static str {
        match value {
            Value::Real(_) => "a real",
            Value::Bool(_) => "a bool",
            Value::Unit => "a unit",
            Value::Closure(_) => "a closure (only `limit` calls one)",
        }
    }

    /// Evaluates one expression.
    pub fn eval(&mut self, id: ExprId) -> Result<Value, Fault> {
        let ir = self.ir;
        match ir.expr(id) {
            Expr::Real(r) => Ok(Value::Real(Dual::constant(r.get()))),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Unit => Ok(Value::Unit),
            Expr::Local(i) => Ok(self.frame()[*i as usize].clone()),
            Expr::Param(i) => match self.params.get(*i as usize) {
                Some(v) => Ok(v.clone()),
                None => Err(self.fault(id, &format!("param {i} is not bound"))),
            },
            Expr::Unary { op, expr } => match op {
                UnaryOp::Neg => Ok(Value::Real(-self.eval_real(*expr)?)),
                UnaryOp::Not => Ok(Value::Bool(!self.eval_bool(*expr)?)),
            },
            Expr::Binary { op, lhs, rhs } => self.binary(id, *op, *lhs, *rhs),
            Expr::If { cond, then, otherwise } => {
                let taken = if self.eval_bool(*cond)? { *then } else { *otherwise };
                self.eval(taken)
            }
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
                let x = self.eval_real(*first)?;
                Ok(Value::Real(match f {
                    MathFn::Exp => x.exp(),
                    MathFn::Ln => x.ln(),
                    MathFn::Sqrt => x.sqrt(),
                    MathFn::Abs => x.abs(),
                }))
            }
            Expr::Closure { .. } => Ok(Value::Closure(id)),
            other => Err(self.fault(id, &format!("the IR form `{}` is not executed by the value walk", Self::form(other)))),
        }
    }

    fn binary(&mut self, id: ExprId, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> Result<Value, Fault> {
        match op {
            BinaryOp::And => {
                let left = self.eval_bool(lhs)?;
                Ok(Value::Bool(left && self.eval_bool(rhs)?))
            }
            BinaryOp::Or => {
                let left = self.eval_bool(lhs)?;
                Ok(Value::Bool(left || self.eval_bool(rhs)?))
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let (l, r) = (self.eval(lhs)?, self.eval(rhs)?);
                let same = match (&l, &r) {
                    (Value::Real(a), Value::Real(b)) => a.v == b.v,
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    _ => return Err(self.fault(id, "equality needs two reals or two bools")),
                };
                Ok(Value::Bool(same == (op == BinaryOp::Eq)))
            }
            _ => {
                let (a, b) = (self.eval_real(lhs)?, self.eval_real(rhs)?);
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
                })
            }
        }
    }

    /// Runs one statement: `let` and expression statements. A contribution is not a value form
    /// and faults here.
    pub fn exec(&mut self, id: StmtId) -> Result<(), Fault> {
        let ir = self.ir;
        match ir.stmt(id) {
            Stmt::Let { local, init } => {
                let v = self.eval(*init)?;
                self.frame()[*local as usize] = v;
                Ok(())
            }
            Stmt::Expr(e) => self.eval(*e).map(|_| ()),
            Stmt::Contribute { addends, .. } => {
                let at = addends.first().map_or_else(|| "<unknown>".to_string(), |a| Self::origin(ir.expr_origin(a.value)));
                Err(Fault::new(Self::UNEXECUTED, format!("the IR form `Contribute` is not executed by the value walk (at {at})")))
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
            self.frame()[*local as usize] = Value::Real(Dual::constant(*value));
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
        let local = |n: &str| Local { name: n.into(), ty: ValueType::F64 };
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
