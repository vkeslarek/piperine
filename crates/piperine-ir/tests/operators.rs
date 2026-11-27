//! Scope: the IR forms of `analog-operators` (delay sites, noise sources, session parameters,
//! trace sites and symbolic differentiation), built by hand through `IrBuilder`, checked,
//! printed and round-tripped through serde (AO-01, AO-02, AO-03, AO-07, AO-09).

use piperine_ir_fixtures::devices;

#[path = "support/operators.rs"]
#[allow(dead_code)]
mod fixtures;

use fixtures::{Operators, Written};

/// A design after a serde round trip.
struct RoundTrip;

impl RoundTrip {
    /// The text of `ir` after a serde round trip, which must equal `ir`.
    fn of(ir: &Ir) -> Ir {
        serde_json::from_str(&serde_json::to_string(ir).expect("serialize")).expect("deserialize")
    }
}

use devices::Devices;
use piperine_ir::{
    BodySpec, Branch, BranchKind, Constant, EmitField, Expr, ExprId, FunctionSpec, Ir, IrBuilder, InstanceSpec, NodeRef, NodeRole, Param, Real, Stmt, Target, Terminal, ValueType,
    BodyNode, BodyId, PartialDerivative, BinaryOp as Op2, Differentiation, Env, MathFn, Method, NotDifferentiable, Variable, UnaryOp, Place, FunctionId, Local,
};

#[test]
fn a_delay_site_is_built_checked_printed_and_round_tripped() {
    // `i(a, gnd) +> 0.5 * delay(v(a), td, 2e-9)`: one site D0 read once.
    let ir = Operators::ir(|b| {
        let site = Operators::delay_site(b, Some(2e-9));
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("delay0(probe(a, gnd), td, 2e-9) \"d\""), "{text}");
    assert!(text.contains("(0.5 * delay0.value)"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.delays().len(), 1);
    assert_eq!(RoundTrip::of(&ir), ir);
    // The site's three parts and the read are dependencies, as a derivative's are.
    assert_eq!(body.dependencies(Target::DelayInput(0)).nodes, vec![0]);
    assert_eq!(body.dependencies(Target::DelayTime(0)).params, vec![0]);
    assert!(body.dependencies(Target::DelayMaximum(0)).nodes.is_empty());
    assert_eq!(body.delay_reads(Target::Flow(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)).delays, vec![0]);
    // The output is an unknown of the system: it reaches a sum affinely.
    assert!(body.is_affine(0));
}

#[test]
fn a_delay_read_out_of_range_or_in_a_function_is_broken() {
    // `delay1` with one site, and `delay0` in a body with none.
    let one_site = Operators::ir(|b| {
        let site = Operators::delay_site(b, None);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 1)], delays: vec![site], ..Written::default() }
    });
    assert!(Operators::names(&one_site.check(), "delay read of site 1, out of range (1 sites)"), "{:?}", one_site.check());
    let none = Operators::ir(|b| Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![], ..Written::default() });
    assert!(Operators::names(&none.check(), "delay read of site 0, out of range (0 sites)"), "{:?}", none.check());
    // A function that reads a delay: a function has no body to own the site.
    let in_function = Operators::ir(|b| {
        let read = b.expr(Expr::Delay(0), ValueType::F64, Operators::at());
        b.function(FunctionSpec { name: "late".into(), origin: Operators::at(), params: vec![], ret: ValueType::F64, locals: vec![], body: read });
        let site = Operators::delay_site(b, None);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    });
    assert!(Operators::names(&in_function.check(), "a delay read outside a body"), "{:?}", in_function.check());
}


#[test]
fn a_noise_source_is_built_checked_printed_and_round_tripped() {
    // `i(a, gnd) +> noise(4e-21, "thermal")`: one source N0 read once.
    let ir = Operators::ir(|b| {
        let source = Operators::noise_site(b, Some("thermal"));
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("noise0(4e-21, \"thermal\") \"n\""), "{text}");
    assert!(text.contains("noise0.value"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.noise_sources().len(), 1);
    assert_eq!(RoundTrip::of(&ir), ir);
    assert!(body.dependencies(Target::NoiseDensity(0)).nodes.is_empty());
    assert_eq!(body.noise_reads(Target::Flow(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)).noise_sources, vec![0]);
    // An unnamed source prints its group as `none` and round-trips too.
    let unnamed = Operators::ir(|b| {
        let source = Operators::noise_site(b, None);
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert!(unnamed.to_string().contains("noise0(4e-21, none)"), "{unnamed}");
    assert_eq!(RoundTrip::of(&unnamed), unnamed);
}

#[test]
fn a_noise_read_out_of_range_or_in_a_function_is_broken() {
    let none = Operators::ir(|b| Written { analog: vec![Operators::contribute_the_noise(b, 0)], ..Written::default() });
    assert!(Operators::names(&none.check(), "noise read of source 0, out of range (0 sources)"), "{:?}", none.check());
    let in_function = Operators::ir(|b| {
        let read = b.expr(Expr::Noise(0), ValueType::F64, Operators::at());
        b.function(FunctionSpec { name: "hiss".into(), origin: Operators::at(), params: vec![], ret: ValueType::F64, locals: vec![], body: read });
        let source = Operators::noise_site(b, None);
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    });
    assert!(Operators::names(&in_function.check(), "a noise read outside a body"), "{:?}", in_function.check());
}


#[test]
fn a_session_parameter_is_built_checked_printed_and_round_tripped() {
    // `i(a, gnd) +> simparam("gmin").unwrap_or_default()`: one parameter S0, an `Option<f64>` read
    // through its `Value` part.
    let ir = Operators::ir(|b| Written { analog: vec![Operators::contribute_the_session_parameter(b, 0)], session_parameters: vec![Operators::gmin()], ..Written::default() });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("session S0 \"gmin\""), "{text}");
    assert!(text.contains("simparam(\"gmin\")"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.session_parameters().len(), 1);
    assert_eq!(RoundTrip::of(&ir), ir);
    // A discrete read: a constant of the sum, listed among the dependencies.
    assert_eq!(body.session_parameter_reads(Target::Flow(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)).session_parameters, vec![0]);
    assert!(body.is_affine(0));
}

#[test]
fn a_session_parameter_read_out_of_range_or_typed_as_a_real_is_broken() {
    let none = Operators::ir(|b| Written { analog: vec![Operators::contribute_the_session_parameter(b, 0)], ..Written::default() });
    assert!(Operators::names(&none.check(), "session parameter read 0, out of range (0 parameters)"), "{:?}", none.check());
    // the read is an `Option<f64>`: an `f64` type on it is a mismatch
    let typed = Operators::ir(|b| {
        let read = b.expr(Expr::SessionParameter { parameter: 0 }, ValueType::F64, Operators::at());
        Written { analog: vec![Operators::contribute(b, read)], session_parameters: vec![Operators::gmin()], ..Written::default() }
    });
    assert!(!typed.check().is_empty(), "{typed}");
}

#[test]
fn a_trace_site_is_built_checked_printed_and_round_tripped() {
    let ir = Operators::tracing();
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("trace0 \"newton\""), "{text}");
    assert!(text.contains("trace0 {f0 = probe(a, gnd), who = \"diode\"}"), "{text}");
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.traces().len(), 1);
    assert_eq!(RoundTrip::of(&ir), ir);
    // A trace reads what its fields read, and writes no row.
    assert_eq!(body.dependencies(Target::Trace(0)).nodes, vec![0]);
}

#[test]
fn a_trace_out_of_range_in_a_function_or_mistyped_is_broken() {
    // A trace of a site the body does not have.
    let none = Operators::ir(|b| Written { analog: vec![Operators::trace_the_probe(b, 0)], streams: vec![Operators::newton()], ..Written::default() });
    assert!(Operators::names(&none.check(), "trace of site 0, out of range (0 sites)"), "{:?}", none.check());
    // A trace in a function: the analog block alone writes one.
    let in_function = Operators::ir(|b| {
        let trace = Operators::trace_the_probe(b, 0);
        let unit = Operators::ex(b).unit();
        let body = Operators::ex(b).block(vec![trace], Some(unit), ValueType::Unit);
        b.function(FunctionSpec { name: "log".into(), origin: Operators::at(), params: vec![], ret: ValueType::Unit, locals: vec![], body });
        Written { analog: vec![], streams: vec![Operators::newton()], traces: vec![Operators::trace_site()], ..Written::default() }
    });
    assert!(Operators::names(&in_function.check(), "a trace outside the analog block of a body"), "{:?}", in_function.check());
    // A field of the wrong type: `x: f64` written a `bool`.
    let mistyped = Operators::ir(|b| {
        let flag = Operators::ex(b).boolean(true);
        let trace = b.stmt(Stmt::Trace { site: 0, fields: vec![EmitField::Value { field: 0, value: flag }] });
        Written { analog: vec![trace], streams: vec![Operators::newton()], traces: vec![Operators::trace_site()], ..Written::default() }
    });
    assert!(Operators::names(&mistyped.check(), "the value of a trace field"), "{:?}", mistyped.check());
}

/// Symbolic differentiation (AO-01): each derivative is built through `Differentiation`, put in
/// an `analog` block and evaluated by a small reference evaluator against its closed form.
struct Calculus;

/// The point the evaluator reads: the potential of node `a` (`x`) and of node `b`, the param `k`
/// and the flow of branch 0.
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    other: f64,
    k: f64,
    flow: f64,
}

/// A reference evaluator of the `f64` forms differentiation produces and reads.
struct Evaluator<'a> {
    ir: &'a Ir,
    point: Point,
}

impl<'a> Evaluator<'a> {
    fn potential(&self, terminal: Terminal) -> f64 {
        match terminal {
            Terminal::Node(0) => self.point.x,
            Terminal::Node(_) => self.point.other,
            Terminal::Ground | Terminal::Own(_) => 0.0,
        }
    }

    /// The value of `id`; `locals` holds the locals of the function being evaluated.
    fn eval(&self, id: ExprId, locals: &mut Vec<f64>) -> f64 {
        match self.ir.expr(id) {
            Expr::Real(r) => r.get(),
            Expr::Int(v) => *v as f64,
            Expr::Bool(b) => f64::from(*b),
            Expr::Param(_) => self.point.k,
            Expr::Flow { .. } => self.point.flow,
            Expr::Probe { plus, minus } => self.potential(*plus) - self.potential(*minus),
            Expr::Local(i) => locals[*i as usize],
            Expr::Unary { op: UnaryOp::Neg, expr } => -self.eval(*expr, locals),
            Expr::Unary { op: UnaryOp::Not, expr } => f64::from(self.eval(*expr, locals) == 0.0),
            Expr::Binary { op, lhs, rhs } => {
                let (a, b) = (self.eval(*lhs, locals), self.eval(*rhs, locals));
                match op {
                    Op2::Add => a + b,
                    Op2::Sub => a - b,
                    Op2::Mul => a * b,
                    Op2::Div => a / b,
                    Op2::Lt => f64::from(a < b),
                    Op2::Le => f64::from(a <= b),
                    Op2::Gt => f64::from(a > b),
                    Op2::Ge => f64::from(a >= b),
                    other => panic!("evaluator has no rule for {other:?}"),
                }
            }
            Expr::If { cond, then, otherwise } => {
                if self.eval(*cond, locals) != 0.0 {
                    self.eval(*then, locals)
                } else {
                    self.eval(*otherwise, locals)
                }
            }
            Expr::Math { f, args } => {
                let x = self.eval(args[0], locals);
                match f {
                    MathFn::Exp => x.exp(),
                    MathFn::Ln => x.ln(),
                    MathFn::Sqrt => x.sqrt(),
                    MathFn::Abs => x.abs(),
                }
            }
            Expr::Method { method, receiver, args } => {
                let x = self.eval(*receiver, locals);
                let rest: Vec<f64> = args.iter().map(|a| self.eval(*a, locals)).collect();
                match method {
                    Method::Sin => x.sin(),
                    Method::Cos => x.cos(),
                    Method::Tan => x.tan(),
                    Method::Asin => x.asin(),
                    Method::Acos => x.acos(),
                    Method::Atan => x.atan(),
                    Method::Atan2 => x.atan2(rest[0]),
                    Method::Sinh => x.sinh(),
                    Method::Cosh => x.cosh(),
                    Method::Tanh => x.tanh(),
                    Method::Powf => x.powf(rest[0]),
                    Method::Powi => x.powi(rest[0] as i32),
                    Method::Log10 => x.log10(),
                    Method::Log2 => x.log2(),
                    Method::Hypot => x.hypot(rest[0]),
                    Method::Min => x.min(rest[0]),
                    Method::Max => x.max(rest[0]),
                    Method::Clamp => x.clamp(rest[0], rest[1]),
                    Method::Floor => x.floor(),
                    Method::Signum => x.signum(),
                    other => panic!("evaluator has no rule for {other:?}"),
                }
            }
            Expr::Cast { expr, .. } => self.eval(*expr, locals),
            Expr::Limit { x, .. } => self.eval(*x, locals),
            Expr::Block { stmts, value } => {
                for stmt in stmts {
                    if let Stmt::Let { local, init } = self.ir.stmt(*stmt) {
                        let v = self.eval(*init, locals);
                        locals[*local as usize] = v;
                    }
                }
                value.map_or(0.0, |v| self.eval(v, locals))
            }
            Expr::Call { function, args } => {
                let values: Vec<f64> = args.iter().map(|a| self.eval(*a, locals)).collect();
                let function = self.ir.function(*function);
                let mut own = vec![0.0; function.locals().len()];
                own[..values.len()].copy_from_slice(&values);
                self.eval(function.body(), &mut own)
            }
            other => panic!("evaluator has no rule for {other:?}"),
        }
    }
}

impl Calculus {
    /// The design with `expr` as the value of a contribution, so `ir.check()` reads it, plus the
    /// evaluator's reading of `expr` at `point`.
    fn value_of(b: IrBuilder, expr: ExprId, point: Point) -> f64 {
        let ir = Calculus::finished(b, expr);
        Evaluator { ir: &ir, point }.eval(expr, &mut Vec::new())
    }

    /// Finishes `b` with one body whose `analog` block contributes `expr`.
    fn finished(mut b: IrBuilder, expr: ExprId) -> Ir {
        let el = Devices::electrical(&mut b);
        let origin = Operators::at();
        let k = Param { name: "k".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1.5))), check: None, structural: false, origin: origin.clone() };
        let contribute = Operators::contribute(&mut b, expr);
        let node = |name: &str| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Calculus".into(),
            origin: origin.clone(),
            params: vec![k],
            nodes: vec![node("a"), node("b")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![contribute]),
            children: vec![],
        });
        let params = vec![Constant::Real(Real::new(1.5))];
        let top = b.instance(InstanceSpec { path: "Calculus".into(), name: "Calculus".into(), body, parent: None, nodes: vec![NodeRef::Ground, NodeRef::Ground], params, origin });
        b.finish(top)
    }

    /// `∂(build)/∂v(a, gnd)` evaluated at `point`: the derivative, and `ir.check()` of the design
    /// that contains it.
    fn derivative_at(variable: Variable, point: Point, build: impl FnOnce(&mut IrBuilder) -> ExprId) -> f64 {
        let mut b = IrBuilder::new();
        let expr = build(&mut b);
        let derivative = Differentiation::new(&mut b, BodyId(0), variable).of(expr).expect("differentiable");
        let ir = Calculus::finished(b, derivative);
        assert_eq!(ir.check(), vec![], "{ir}");
        Evaluator { ir: &ir, point }.eval(derivative, &mut Vec::new())
    }

    /// The potential variable of the probe `v(a, gnd)`.
    fn wrt_a() -> Variable {
        Variable::Potential { plus: Terminal::Node(0), minus: Terminal::Ground }
    }

    fn at(x: f64) -> Point {
        Point { x, other: 0.3, k: 1.5, flow: 0.0 }
    }

    /// The probe `v(a, gnd)`.
    fn x(b: &mut IrBuilder) -> ExprId {
        Operators::ex(b).probe(Terminal::Node(0), Terminal::Ground)
    }

    /// The param `k`.
    fn k(b: &mut IrBuilder) -> ExprId {
        b.expr(Expr::Param(0), ValueType::F64, Operators::at())
    }

    fn lit(b: &mut IrBuilder, v: f64) -> ExprId {
        Operators::ex(b).real(v)
    }

    fn math(b: &mut IrBuilder, f: MathFn, arg: ExprId) -> ExprId {
        Operators::ex(b).math(f, arg)
    }

    fn binary(b: &mut IrBuilder, op: Op2, l: ExprId, r: ExprId) -> ExprId {
        Operators::ex(b).binary(op, l, r)
    }

    fn method(b: &mut IrBuilder, method: Method, receiver: ExprId, args: Vec<ExprId>) -> ExprId {
        b.expr(Expr::Method { method, receiver, args }, ValueType::F64, Operators::at())
    }

    /// Whether `actual` equals `expected` to a relative 1e-12.
    fn agree(row: &str, actual: f64, expected: f64) {
        let bound = 1e-12 * expected.abs().max(1.0);
        assert!((actual - expected).abs() <= bound, "{row}: derivative {actual}, closed form {expected}");
    }
}

#[test]
fn each_form_differentiates_by_the_rule_of_its_row() {
    let k = 1.5f64;
    // (row, x, build over x and k, the closed form of the derivative)
    type Build = fn(&mut IrBuilder) -> ExprId;
    let rows: Vec<(&str, f64, Build, fn(f64, f64) -> f64)> = vec![
        ("x*x", 0.7, |b| { let (x, y) = (Calculus::x(b), Calculus::x(b)); Calculus::binary(b, Op2::Mul, x, y) }, |x, _| 2.0 * x),
        ("x*k + k", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); let p = Calculus::binary(b, Op2::Mul, x, k); let k2 = Calculus::k(b); Calculus::binary(b, Op2::Add, p, k2) }, |_, k| k),
        ("x - k", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::binary(b, Op2::Sub, x, k) }, |_, _| 1.0),
        ("x / k", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::binary(b, Op2::Div, x, k) }, |_, k| 1.0 / k),
        ("k / x", 0.7, |b| { let (k, x) = (Calculus::k(b), Calculus::x(b)); Calculus::binary(b, Op2::Div, k, x) }, |x, k| -k / (x * x)),
        ("x / (x + k)", 0.7, |b| { let (x, y, k) = (Calculus::x(b), Calculus::x(b), Calculus::k(b)); let s = Calculus::binary(b, Op2::Add, y, k); Calculus::binary(b, Op2::Div, x, s) }, |x, k| k / ((x + k) * (x + k))),
        ("-x", 0.7, |b| { let x = Calculus::x(b); b.expr(Expr::Unary { op: UnaryOp::Neg, expr: x }, ValueType::F64, Operators::at()) }, |_, _| -1.0),
        ("exp(k*x)", 0.7, |b| { let (k, x) = (Calculus::k(b), Calculus::x(b)); let p = Calculus::binary(b, Op2::Mul, k, x); Calculus::math(b, MathFn::Exp, p) }, |x, k| k * (k * x).exp()),
        ("ln(x)", 0.7, |b| { let x = Calculus::x(b); Calculus::math(b, MathFn::Ln, x) }, |x, _| 1.0 / x),
        ("sqrt(x)", 0.7, |b| { let x = Calculus::x(b); Calculus::math(b, MathFn::Sqrt, x) }, |x, _| 0.5 / x.sqrt()),
        ("abs(x), x > 0", 0.7, |b| { let x = Calculus::x(b); Calculus::math(b, MathFn::Abs, x) }, |_, _| 1.0),
        ("abs(x), x < 0", -0.7, |b| { let x = Calculus::x(b); Calculus::math(b, MathFn::Abs, x) }, |_, _| -1.0),
        ("sin", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Sin, x, vec![]) }, |x, _| x.cos()),
        ("cos", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Cos, x, vec![]) }, |x, _| -x.sin()),
        ("tan", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Tan, x, vec![]) }, |x, _| 1.0 / (x.cos() * x.cos())),
        ("asin", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Asin, x, vec![]) }, |x, _| 1.0 / (1.0 - x * x).sqrt()),
        ("acos", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Acos, x, vec![]) }, |x, _| -1.0 / (1.0 - x * x).sqrt()),
        ("atan", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Atan, x, vec![]) }, |x, _| 1.0 / (1.0 + x * x)),
        ("atan2(x, k)", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Atan2, x, vec![k]) }, |x, k| k / (x * x + k * k)),
        ("atan2(k, x)", 0.7, |b| { let (k, x) = (Calculus::k(b), Calculus::x(b)); Calculus::method(b, Method::Atan2, k, vec![x]) }, |x, k| -k / (x * x + k * k)),
        ("sinh", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Sinh, x, vec![]) }, |x, _| x.cosh()),
        ("cosh", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Cosh, x, vec![]) }, |x, _| x.sinh()),
        ("tanh", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Tanh, x, vec![]) }, |x, _| 1.0 - x.tanh() * x.tanh()),
        ("log10", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Log10, x, vec![]) }, |x, _| 1.0 / (x * std::f64::consts::LN_10)),
        ("log2", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Log2, x, vec![]) }, |x, _| 1.0 / (x * std::f64::consts::LN_2)),
        ("hypot(x, k)", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Hypot, x, vec![k]) }, |x, k| x / x.hypot(k)),
        ("powf(x, 3)", 0.7, |b| { let (x, e) = (Calculus::x(b), Calculus::lit(b, 3.0)); Calculus::method(b, Method::Powf, x, vec![e]) }, |x, _| 3.0 * x * x),
        ("powf(k, x)", 0.7, |b| { let (k, x) = (Calculus::k(b), Calculus::x(b)); Calculus::method(b, Method::Powf, k, vec![x]) }, |x, k| k.powf(x) * k.ln()),
        ("powf(x, x)", 0.7, |b| { let (x, y) = (Calculus::x(b), Calculus::x(b)); Calculus::method(b, Method::Powf, x, vec![y]) }, |x, _| x.powf(x) * (x.ln() + 1.0)),
        ("min(x, k), below", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Min, x, vec![k]) }, |_, _| 1.0),
        ("min(x, k), above", 2.5, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Min, x, vec![k]) }, |_, _| 0.0),
        ("max(x, k), below", 0.7, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Max, x, vec![k]) }, |_, _| 0.0),
        ("max(x, k), above", 2.5, |b| { let (x, k) = (Calculus::x(b), Calculus::k(b)); Calculus::method(b, Method::Max, x, vec![k]) }, |_, _| 1.0),
        ("clamp(x, -1, 1), inside", 0.7, |b| { let (x, lo, hi) = (Calculus::x(b), Calculus::lit(b, -1.0), Calculus::lit(b, 1.0)); Calculus::method(b, Method::Clamp, x, vec![lo, hi]) }, |_, _| 1.0),
        ("clamp(x, -1, 1), above", 2.5, |b| { let (x, lo, hi) = (Calculus::x(b), Calculus::lit(b, -1.0), Calculus::lit(b, 1.0)); Calculus::method(b, Method::Clamp, x, vec![lo, hi]) }, |_, _| 0.0),
        ("floor(x)", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Floor, x, vec![]) }, |_, _| 0.0),
        ("signum(x)", 0.7, |b| { let x = Calculus::x(b); Calculus::method(b, Method::Signum, x, vec![]) }, |_, _| 0.0),
        ("if x > 0 { x*x } else { -x }, then", 0.7, |b| { let (c, p, q) = (Calculus::x(b), Calculus::x(b), Calculus::x(b)); let z = Calculus::lit(b, 0.0); let cond = Calculus::binary(b, Op2::Gt, c, z); let sq = Calculus::binary(b, Op2::Mul, p, q); let r = Calculus::x(b); let neg = b.expr(Expr::Unary { op: UnaryOp::Neg, expr: r }, ValueType::F64, Operators::at()); Operators::ex(b).if_f64(cond, sq, neg) }, |x, _| 2.0 * x),
        ("if x > 0 { x*x } else { -x }, else", -0.7, |b| { let (c, p, q) = (Calculus::x(b), Calculus::x(b), Calculus::x(b)); let z = Calculus::lit(b, 0.0); let cond = Calculus::binary(b, Op2::Gt, c, z); let sq = Calculus::binary(b, Op2::Mul, p, q); let r = Calculus::x(b); let neg = b.expr(Expr::Unary { op: UnaryOp::Neg, expr: r }, ValueType::F64, Operators::at()); Operators::ex(b).if_f64(cond, sq, neg) }, |_, _| -1.0),
    ];
    for (row, x, build, closed) in rows {
        Calculus::agree(row, Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(x), build), closed(x, k));
    }
    // powi(x, 3) with an integer exponent: 3·x²
    let powi = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| {
        let x = Calculus::x(b);
        let three = b.expr(Expr::Int(3), ValueType::Int(32), Operators::at());
        Calculus::method(b, Method::Powi, x, vec![three])
    });
    Calculus::agree("powi(x, 3)", powi, 3.0 * 0.7 * 0.7);
}

#[test]
fn the_reversed_probe_and_a_flow_have_their_own_rule() {
    // d v(gnd, a) / d v(a, gnd) = -1: the reversed probe of the same ends.
    let reversed = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| Operators::ex(b).probe(Terminal::Ground, Terminal::Node(0)));
    assert_eq!(reversed, -1.0);
    // d (3 * i(b0)) / d i(b0) = 3, and the flow of another branch is held constant.
    let own = Calculus::derivative_at(Variable::Flow { branch: 0 }, Calculus::at(0.7), |b| {
        let (flow, three) = (b.expr(Expr::Flow { branch: 0 }, ValueType::F64, Operators::at()), Calculus::lit(b, 3.0));
        Calculus::binary(b, Op2::Mul, three, flow)
    });
    assert_eq!(own, 3.0);
    let other = Calculus::derivative_at(Variable::Flow { branch: 0 }, Calculus::at(0.7), |b| b.expr(Expr::Flow { branch: 1 }, ValueType::F64, Operators::at()));
    assert_eq!(other, 0.0);
    // a probe is not a flow, and a flow is not a probe
    let probe_of_flow = Calculus::derivative_at(Variable::Flow { branch: 0 }, Calculus::at(0.7), Calculus::x);
    assert_eq!(probe_of_flow, 0.0);
}

#[test]
fn probes_of_other_branches_are_held_constant() {
    // d v(a, b) / d v(a, gnd) = 0 although v(a, b) moves with node a: another branch (§14.2.3).
    let other = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| Operators::ex(b).probe(Terminal::Node(0), Terminal::Node(1)));
    assert_eq!(other, 0.0);
    // d (v(b, gnd) * v(a, gnd)) / d v(a, gnd) = v(b, gnd): the other probe stays a constant factor.
    let held = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| {
        let (other, x) = (Operators::ex(b).probe(Terminal::Node(1), Terminal::Ground), Calculus::x(b));
        Calculus::binary(b, Op2::Mul, other, x)
    });
    Calculus::agree("v(b)*v(a)", held, 0.3);
}

#[test]
fn limit_is_transparent_to_ddx() {
    // d limit(x*x, f) / dx = 2x (§18 rule 3): the limiter does not change the derivative.
    let through = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| {
        let (p, q, f) = (Calculus::x(b), Calculus::x(b), Calculus::k(b));
        let square = Calculus::binary(b, Op2::Mul, p, q);
        b.expr(Expr::Limit { site: 0, x: square, f, seed: None }, ValueType::F64, Operators::at())
    });
    Calculus::agree("limit", through, 1.4);
}

#[test]
fn results_of_other_instances_are_held_constant() {
    // a site's result, an environment read and a session parameter do not move with v(a, gnd)
    for (row, make) in [
        ("delay", Expr::Delay(0)),
        ("noise", Expr::Noise(0)),
        ("rate", Expr::Derivative(0)),
        ("env", Expr::Env(Env::Temperature)),
        ("simparam", Expr::SessionParameter { parameter: 0 }),
    ] {
        let mut b = IrBuilder::new();
        let read = b.expr(make, ValueType::F64, Operators::at());
        let derivative = Differentiation::new(&mut b, BodyId(0), Calculus::wrt_a()).of(read).expect("differentiable");
        assert!(matches!(b.expr_kind(derivative), Some(Expr::Real(r)) if r.get() == 0.0), "{row}");
    }
}

#[test]
fn a_call_a_block_with_lets_and_a_second_derivative_differentiate() {
    // g(u, w) = u * u * w; d g(x, k) / dx = 2·x·k, through ∂g/∂u of the derived function
    let call = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| {
        let mut e = Operators::ex(b);
        let (u1, u2, w) = (e.local(0), e.local(0), e.local(1));
        let square = e.binary(Op2::Mul, u1, u2);
        let body = e.binary(Op2::Mul, square, w);
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let g = b.function(FunctionSpec { name: "g".into(), origin: Operators::at(), params: vec![local("u"), local("w")], ret: ValueType::F64, locals: vec![local("u"), local("w")], body });
        let (x, k) = (Calculus::x(b), Calculus::k(b));
        Operators::ex(b).call(g, vec![x, k])
    });
    Calculus::agree("g(x, k)", call, 2.0 * 0.7 * 1.5);
    // h(u) = { let t = u * u; t * t }: d h(x) / dx = 4·x³, forward through the `let`
    let block = Calculus::derivative_at(Calculus::wrt_a(), Calculus::at(0.7), |b| {
        let mut e = Operators::ex(b);
        let (u1, u2) = (e.local(0), e.local(0));
        let init = e.binary(Op2::Mul, u1, u2);
        let (t1, t2) = (e.local(1), e.local(1));
        let value = e.binary(Op2::Mul, t1, t2);
        let bind = b.stmt(Stmt::Let { local: 1, init });
        let body = Operators::ex(b).block(vec![bind], Some(value), ValueType::F64);
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let h = b.function(FunctionSpec { name: "h".into(), origin: Operators::at(), params: vec![local("u")], ret: ValueType::F64, locals: vec![local("u"), local("t")], body });
        let x = Calculus::x(b);
        Operators::ex(b).call(h, vec![x])
    });
    Calculus::agree("h(x)", block, 4.0 * 0.7f64.powi(3));
    // a second derivative needs no rule of its own: d²(x³)/dx² = 6x
    let mut b = IrBuilder::new();
    let (p, q, r) = (Calculus::x(&mut b), Calculus::x(&mut b), Calculus::x(&mut b));
    let square = Calculus::binary(&mut b, Op2::Mul, p, q);
    let cube = Calculus::binary(&mut b, Op2::Mul, square, r);
    let first = Differentiation::new(&mut b, BodyId(0), Calculus::wrt_a()).of(cube).expect("first");
    let second = Differentiation::new(&mut b, BodyId(0), Calculus::wrt_a()).of(first).expect("second");
    Calculus::agree("x^3''", Calculus::value_of(b, second, Calculus::at(0.7)), 6.0 * 0.7);
}

#[test]
fn a_form_with_no_rule_is_not_differentiable_by_name() {
    let origin = Operators::at();
    let refused = |build: &dyn Fn(&mut IrBuilder) -> ExprId| -> NotDifferentiable {
        let mut b = IrBuilder::new();
        let expr = build(&mut b);
        Differentiation::new(&mut b, BodyId(0), Calculus::wrt_a()).of(expr).expect_err("refused")
    };
    // a loop that yields an `f64`
    let in_loop = refused(&|b| b.expr(Expr::Loop { body: vec![] }, ValueType::F64, Operators::at()));
    assert_eq!(in_loop.form, "a loop");
    assert_eq!(in_loop.origin, origin);
    // a call through a function value
    let through_value = refused(&|b| {
        let callee = b.expr(Expr::FunctionRef(FunctionId(0)), ValueType::Fn(vec![], Box::new(ValueType::F64)), Operators::at());
        b.expr(Expr::CallValue { callee, args: vec![] }, ValueType::F64, Operators::at())
    });
    assert_eq!(through_value.form, "a closure or a call through a function value");
    // a local in the standalone expression of a body
    let local = refused(&|b| Operators::ex(b).local(0));
    assert_eq!(local.form, "a local outside a function");
    // an assignment inside a function the call reaches
    let assignment = refused(&|b| {
        let mut e = Operators::ex(b);
        let init = e.real(0.0);
        let bind = b.stmt(Stmt::Let { local: 1, init });
        let value = Operators::ex(b).real(1.0);
        let assign = b.stmt(Stmt::Assign { place: Place { local: 1, path: vec![] }, op: None, value });
        let result = Operators::ex(b).local(1);
        let body = Operators::ex(b).block(vec![bind, assign], Some(result), ValueType::F64);
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: true };
        let f = b.function(FunctionSpec { name: "bump".into(), origin: Operators::at(), params: vec![local("u")], ret: ValueType::F64, locals: vec![local("u"), local("t")], body });
        let x = Calculus::x(b);
        Operators::ex(b).call(f, vec![x])
    });
    assert_eq!(assignment.form, "an assignment");
    // a recursive function: f(u) = f(u)
    let recursion = refused(&|b| {
        let again_arg = Operators::ex(b).local(0);
        let again = Operators::ex(b).call(FunctionId(0), vec![again_arg]);
        let local = |name: &str| Local { name: name.into(), ty: ValueType::F64, mutable: false };
        let f = b.function(FunctionSpec { name: "forever".into(), origin: Operators::at(), params: vec![local("u")], ret: ValueType::F64, locals: vec![local("u")], body: again });
        let x = Calculus::x(b);
        Operators::ex(b).call(f, vec![x])
    });
    assert_eq!(recursion.form, "a recursive function");
}

#[test]
fn a_partial_derivative_is_recorded_printed_and_round_tripped() {
    // The derivative itself is an ordinary expression; the record says where `ddx` stood.
    let mut b = IrBuilder::new();
    let el = Devices::electrical(&mut b);
    let origin = Operators::at();
    let value = Operators::ex(&mut b).real(1.0);
    let analog = vec![Operators::contribute(&mut b, value)];
    let body = b.body(BodySpec {
        name: "Op".into(),
        origin: origin.clone(),
        params: vec![],
        nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: Some(analog),
        children: vec![],
    });
    assert_eq!(b.partial_derivative(body, PartialDerivative { name: "v(a, gnd)".into(), origin: origin.clone() }), 0);
    let top = b.instance(InstanceSpec { path: "Op".into(), name: "Op".into(), body, parent: None, nodes: vec![NodeRef::Ground], params: vec![], origin });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "{ir}");
    assert!(ir.to_string().contains("ddx0 \"v(a, gnd)\""), "{ir}");
    assert_eq!(ir.bodies().next().expect("a body").partial_derivatives().len(), 1);
    assert_eq!(RoundTrip::of(&ir), ir);
}
