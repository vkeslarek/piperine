//! The five M1 device bodies, the functions they call and the divider design, built by hand
//! through `IrBuilder` exactly as `tests/e2e/twins/devices.msl` and `divider.msl` read, in source
//! order. Shared by `piperine-interp`'s tests through `#[path]`.

use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Child, Constant, Env, Expr,
    ExprId, FunctionId, FunctionSpec, InstanceId, InstanceSpec, IrBuilder, Local, MathFn,
    NatureId, NodeRef, NodeRole, NodeSpec, Operator, Origin, Param, Quantity, QuantityRole, Real,
    Site, Stmt, Terminal, UnaryOp, ValueType,
};

/// Expression construction at one source position; types follow the operator.
pub struct Ex<'b> {
    pub b: &'b mut IrBuilder,
    pub file: &'static str,
    pub line: u32,
    pub col: u32,
}

impl<'b> Ex<'b> {
    pub fn new(b: &'b mut IrBuilder, file: &'static str, line: u32, col: u32) -> Self {
        Ex { b, file, line, col }
    }

    fn origin(&self) -> Origin {
        Origin { file: self.file.to_string(), line: self.line, col: self.col }
    }

    pub fn at(&mut self, line: u32, col: u32) -> &mut Self {
        self.line = line;
        self.col = col;
        self
    }

    pub fn real(&mut self, v: f64) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Real(Real::new(v)), ValueType::F64, o)
    }

    pub fn boolean(&mut self, v: bool) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Bool(v), ValueType::Bool, o)
    }

    pub fn unit(&mut self) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Unit, ValueType::Unit, o)
    }

    pub fn local(&mut self, i: u32) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Local(i), ValueType::F64, o)
    }

    pub fn param(&mut self, i: u32) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Param(i), ValueType::F64, o)
    }

    pub fn unary(&mut self, op: UnaryOp, expr: ExprId) -> ExprId {
        let o = self.origin();
        let ty = if op == UnaryOp::Neg { ValueType::F64 } else { ValueType::Bool };
        self.b.expr(Expr::Unary { op, expr }, ty, o)
    }

    pub fn binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> ExprId {
        let o = self.origin();
        let ty = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => ValueType::F64,
            _ => ValueType::Bool,
        };
        self.b.expr(Expr::Binary { op, lhs, rhs }, ty, o)
    }

    /// An `if` of `f64` arms.
    pub fn if_f64(&mut self, cond: ExprId, then: ExprId, otherwise: ExprId) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::If { cond, then, otherwise }, ValueType::F64, o)
    }

    pub fn block(&mut self, stmts: Vec<piperine_ir::StmtId>, value: Option<ExprId>, ty: ValueType) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Block { stmts, value }, ty, o)
    }

    pub fn call(&mut self, function: FunctionId, args: Vec<ExprId>) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Call { function, args }, ValueType::F64, o)
    }

    pub fn math(&mut self, f: MathFn, arg: ExprId) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Math { f, args: vec![arg] }, ValueType::F64, o)
    }

    pub fn closure(&mut self, params: Vec<u32>, body: ExprId) -> ExprId {
        let o = self.origin();
        let ty = ValueType::Fn(vec![ValueType::F64; params.len()], Box::new(ValueType::F64));
        self.b.expr(Expr::Closure { params, body }, ty, o)
    }

    pub fn probe(&mut self, plus: Terminal, minus: Terminal) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Probe { plus, minus }, ValueType::F64, o)
    }

    pub fn env(&mut self, env: Env) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Env(env), ValueType::F64, o)
    }

    pub fn limit(&mut self, site: u32, x: ExprId, f: ExprId, seed: Option<ExprId>) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Limit { site, x, f, seed }, ValueType::F64, o)
    }

    pub fn stim(&mut self, site: u32, mag: ExprId, phase: Option<ExprId>) -> ExprId {
        let o = self.origin();
        self.b.expr(Expr::Stim { site, mag, phase }, ValueType::F64, o)
    }
}

/// The hand-built library.
pub struct Devices;

impl Devices {
    const FILE: &'static str = "devices.msl";

    fn origin(line: u32, col: u32) -> Origin {
        Origin { file: Self::FILE.to_string(), line, col }
    }

    fn param(name: &str, default: Option<f64>, check: Option<ExprId>, line: u32) -> Param {
        Param {
            name: name.to_string(),
            ty: ValueType::F64,
            default: default.map(|v| Constant::Real(Real::new(v))),
            check,
            structural: false,
            origin: Self::origin(line, 5),
        }
    }

    fn port(name: &str, nature: NatureId, line: u32, col: u32) -> BodyNode {
        BodyNode { name: name.to_string(), nature, role: NodeRole::Port, init: None, origin: Self::origin(line, col) }
    }

    /// `where <param> <op> 0.0` over param `index`.
    fn positive(b: &mut IrBuilder, op: BinaryOp, index: u32, line: u32, col: u32) -> ExprId {
        let mut e = Ex::new(b, Self::FILE, line, col);
        let p = e.param(index);
        let zero = e.real(0.0);
        e.binary(op, p, zero)
    }

    /// The `Electrical` nature of `std/carriers.msl`, added once per builder.
    pub fn electrical(b: &mut IrBuilder) -> NatureId {
        if let Some(id) = b.find_nature("Electrical") {
            return id;
        }
        let voltage = Quantity { name: "Voltage".into(), access: "v".into(), units: "V".into(), abstol: Real::new(1e-6) };
        let current = Quantity { name: "Current".into(), access: "i".into(), units: "A".into(), abstol: Real::new(1e-12) };
        b.nature("Electrical", voltage, Some(current))
    }

    /// `std/analog.msl`: `pub fn vt() -> f64 { K_B * temperature() / Q }`, constants folded.
    pub fn vt(b: &mut IrBuilder) -> FunctionId {
        if let Some(id) = b.find_function("vt") {
            return id;
        }
        let mut e = Ex::new(b, "analog.msl", 10, 24);
        let kb = e.real(1.380649e-23);
        let t = e.env(Env::Temperature);
        let num = e.binary(BinaryOp::Mul, kb, t);
        let q = e.real(1.602176634e-19);
        let body = e.binary(BinaryOp::Div, num, q);
        b.function(FunctionSpec {
            name: "vt".into(),
            origin: Origin { file: "analog.msl".into(), line: 10, col: 1 },
            params: vec![],
            ret: ValueType::F64,
            locals: vec![],
            body,
        })
    }

    /// `std/analog.msl`: `pnjlim(new, old, nvt, vcrit)`. Locals 0..4 are the params; 4 and 5 the
    /// two `arg` lets.
    pub fn pnjlim(b: &mut IrBuilder) -> FunctionId {
        if let Some(id) = b.find_function("pnjlim") {
            return id;
        }
        let mut e = Ex::new(b, "analog.msl", 22, 8);
        let (new, old, nvt, vcrit) = (0u32, 1u32, 2u32, 3u32);
        // new > vcrit && (new - old).abs() > 2.0 * nvt
        let n = e.local(new);
        let vc = e.local(vcrit);
        let c1 = e.binary(BinaryOp::Gt, n, vc);
        let n = e.local(new);
        let o = e.local(old);
        let diff = e.binary(BinaryOp::Sub, n, o);
        let abs = e.math(MathFn::Abs, diff);
        let two = e.real(2.0);
        let nv = e.local(nvt);
        let lim = e.binary(BinaryOp::Mul, two, nv);
        let c2 = e.binary(BinaryOp::Gt, abs, lim);
        let cond = e.binary(BinaryOp::And, c1, c2);

        // old > 0.0
        e.at(23, 12);
        let o = e.local(old);
        let z = e.real(0.0);
        let old_pos = e.binary(BinaryOp::Gt, o, z);
        // let arg = (new - old) / nvt;
        e.at(24, 17);
        let n = e.local(new);
        let o = e.local(old);
        let d = e.binary(BinaryOp::Sub, n, o);
        let nv = e.local(nvt);
        let arg_init = e.binary(BinaryOp::Div, d, nv);
        let let_arg = e.b.stmt(Stmt::Let { local: 4, init: arg_init });
        let mut e = Ex::new(e.b, "analog.msl", 25, 17);
        // if arg > 0.0 { old + nvt * (2.0 + (arg - 2.0).ln()) } else { old - nvt * (2.0 + (2.0 - arg).ln()) }
        let a = e.local(4);
        let z = e.real(0.0);
        let arg_pos = e.binary(BinaryOp::Gt, a, z);
        let o = e.local(old);
        let nv = e.local(nvt);
        let two = e.real(2.0);
        let a = e.local(4);
        let two2 = e.real(2.0);
        let am2 = e.binary(BinaryOp::Sub, a, two2);
        let ln = e.math(MathFn::Ln, am2);
        let inner = e.binary(BinaryOp::Add, two, ln);
        let scaled = e.binary(BinaryOp::Mul, nv, inner);
        let up = e.binary(BinaryOp::Add, o, scaled);
        let o = e.local(old);
        let nv = e.local(nvt);
        let two = e.real(2.0);
        let two2 = e.real(2.0);
        let a = e.local(4);
        let m = e.binary(BinaryOp::Sub, two2, a);
        let ln = e.math(MathFn::Ln, m);
        let inner = e.binary(BinaryOp::Add, two, ln);
        let scaled = e.binary(BinaryOp::Mul, nv, inner);
        let down = e.binary(BinaryOp::Sub, o, scaled);
        let by_arg = e.if_f64(arg_pos, up, down);
        let positive_branch = e.block(vec![let_arg], Some(by_arg), ValueType::F64);
        // else { nvt * (new / nvt).ln() }
        let nv = e.local(nvt);
        let n = e.local(new);
        let nv2 = e.local(nvt);
        let q = e.binary(BinaryOp::Div, n, nv2);
        let ln = e.math(MathFn::Ln, q);
        let nonpos_branch = e.binary(BinaryOp::Mul, nv, ln);
        let junction = e.if_f64(old_pos, positive_branch, nonpos_branch);

        // else if new < 0.0 { let arg = if old > 0.0 { -old - 1.0 } else { 2.0 * old - 1.0 }; if new < arg { arg } else { new } }
        e.at(27, 15);
        let n = e.local(new);
        let z = e.real(0.0);
        let new_neg = e.binary(BinaryOp::Lt, n, z);
        e.at(28, 19);
        let o = e.local(old);
        let z = e.real(0.0);
        let old_pos = e.binary(BinaryOp::Gt, o, z);
        let o = e.local(old);
        let neg = e.unary(UnaryOp::Neg, o);
        let one = e.real(1.0);
        let lhs = e.binary(BinaryOp::Sub, neg, one);
        let two = e.real(2.0);
        let o = e.local(old);
        let twice = e.binary(BinaryOp::Mul, two, o);
        let one = e.real(1.0);
        let rhs = e.binary(BinaryOp::Sub, twice, one);
        let arg_init = e.if_f64(old_pos, lhs, rhs);
        let let_arg = e.b.stmt(Stmt::Let { local: 5, init: arg_init });
        let mut e = Ex::new(e.b, "analog.msl", 29, 12);
        let n = e.local(new);
        let a = e.local(5);
        let below = e.binary(BinaryOp::Lt, n, a);
        let a = e.local(5);
        let n = e.local(new);
        let clamp = e.if_f64(below, a, n);
        let negative_branch = e.block(vec![let_arg], Some(clamp), ValueType::F64);
        let n = e.local(new);
        let tail = e.if_f64(new_neg, negative_branch, n);
        let body = e.if_f64(cond, junction, tail);
        let local = |name: &str| Local { name: name.to_string(), ty: ValueType::F64 };
        b.function(FunctionSpec {
            name: "pnjlim".into(),
            origin: Origin { file: "analog.msl".into(), line: 21, col: 1 },
            params: vec![local("new"), local("old"), local("nvt"), local("vcrit")],
            ret: ValueType::F64,
            locals: vec![local("new"), local("old"), local("nvt"), local("vcrit"), local("arg"), local("arg")],
            body,
        })
    }

    /// `pub module Resistor(p, n) { r: f64 where r > 0.0 }` and `i(p, n) +> v(p, n) / r`.
    pub fn resistor(b: &mut IrBuilder) -> BodyId {
        let el = Self::electrical(b);
        let check = Self::positive(b, BinaryOp::Gt, 0, 6, 17);
        let mut e = Ex::new(b, Self::FILE, 8, 32);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let r = e.param(0);
        let value = e.binary(BinaryOp::Div, v, r);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        b.body(BodySpec {
            name: "Resistor".into(),
            origin: Self::origin(5, 1),
            params: vec![Self::param("r", None, Some(check), 6)],
            nodes: vec![Self::port("p", el, 5, 21), Self::port("n", el, 5, 36)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        })
    }

    /// `i(p, n) +> ddt(c * v(p, n))`.
    pub fn capacitor(b: &mut IrBuilder) -> BodyId {
        let el = Self::electrical(b);
        let check = Self::positive(b, BinaryOp::Ge, 0, 12, 17);
        let mut e = Ex::new(b, Self::FILE, 14, 40);
        let c = e.param(0);
        let v = e.probe(Terminal::Node(0), Terminal::Node(1));
        let value = e.binary(BinaryOp::Mul, c, v);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Ddt, negated: false, value }],
        });
        b.body(BodySpec {
            name: "Capacitor".into(),
            origin: Self::origin(11, 1),
            params: vec![Self::param("c", None, Some(check), 12)],
            nodes: vec![Self::port("p", el, 11, 22), Self::port("n", el, 11, 37)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        })
    }

    /// A source: `<q>(p, n) +> dc * source_scale() + ac_stim(ac)`.
    fn source(b: &mut IrBuilder, name: &str, line: u32, kind: BranchKind) -> BodyId {
        let el = Self::electrical(b);
        let col = 20;
        let mut e = Ex::new(b, Self::FILE, line + 4, 30);
        let dc = e.param(0);
        let scale = e.env(Env::SourceScale);
        let scaled = e.binary(BinaryOp::Mul, dc, scale);
        let ac = e.param(1);
        let stim = e.stim(0, ac, None);
        let value = e.binary(BinaryOp::Add, scaled, stim);
        let quantity = if kind == BranchKind::Flow { QuantityRole::Flow } else { QuantityRole::Potential };
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        b.body(BodySpec {
            name: name.to_string(),
            origin: Self::origin(line, 1),
            params: vec![
                Self::param("dc", Some(0.0), None, line + 1),
                Self::param("ac", Some(0.0), None, line + 2),
            ],
            nodes: vec![Self::port("p", el, line, col), Self::port("n", el, line, col + 15)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind }],
            limits: vec![],
            stims: vec![Site { name: "ac".into(), origin: Self::origin(line + 4, 62), enabled: None }],
            locals: vec![],
            analog: Some(vec![stmt]),
            children: vec![],
        })
    }

    /// `v(p, n) +> dc * source_scale() + ac_stim(ac)`.
    pub fn vsource(b: &mut IrBuilder) -> BodyId {
        Self::source(b, "Vsource", 17, BranchKind::Potential)
    }

    /// `i(p, n) +> dc * source_scale() + ac_stim(ac)`.
    pub fn isource(b: &mut IrBuilder) -> BodyId {
        Self::source(b, "Isource", 24, BranchKind::Flow)
    }

    /// The junction diode with its junction shunt. The contribution is two addends: the direct
    /// `id + gmin() * vd` and `ddt(cj0 * vd)`. Locals: nvt 0, vcrit 1, vd 2, id 3, new 4, old 5.
    pub fn diode(b: &mut IrBuilder) -> BodyId {
        let el = Self::electrical(b);
        let vt = Self::vt(b);
        let pnjlim = Self::pnjlim(b);
        let (is, n, cj0) = (0u32, 1u32, 2u32);
        let (nvt, vcrit, vd, id) = (0u32, 1u32, 2u32, 3u32);
        let checks = [
            Self::positive(b, BinaryOp::Gt, is, 32, 17),
            Self::positive(b, BinaryOp::Gt, n, 33, 17),
            Self::positive(b, BinaryOp::Ge, cj0, 34, 18),
        ];
        // let nvt = n * vt();
        let mut e = Ex::new(b, Self::FILE, 37, 17);
        let p = e.param(n);
        let call = e.call(vt, vec![]);
        let init = e.binary(BinaryOp::Mul, p, call);
        let let_nvt = b.stmt(Stmt::Let { local: nvt, init });
        // let vcrit = nvt * (nvt / (2.0_f64.sqrt() * is)).ln();
        let mut e = Ex::new(b, Self::FILE, 38, 17);
        let l = e.local(nvt);
        let l2 = e.local(nvt);
        let two = e.real(2.0);
        let root = e.math(MathFn::Sqrt, two);
        let p = e.param(is);
        let denom = e.binary(BinaryOp::Mul, root, p);
        let ratio = e.binary(BinaryOp::Div, l2, denom);
        let ln = e.math(MathFn::Ln, ratio);
        let init = e.binary(BinaryOp::Mul, l, ln);
        let let_vcrit = b.stmt(Stmt::Let { local: vcrit, init });
        // let vd = limit(v(a, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);
        let mut e = Ex::new(b, Self::FILE, 39, 28);
        let x = e.probe(Terminal::Node(0), Terminal::Node(1));
        let (a_new, a_old, a_nvt, a_vcrit) = (e.local(4), e.local(5), e.local(nvt), e.local(vcrit));
        let call = e.call(pnjlim, vec![a_new, a_old, a_nvt, a_vcrit]);
        let f = e.closure(vec![4, 5], call);
        let seed = e.local(vcrit);
        let init = e.limit(0, x, f, Some(seed));
        let let_vd = b.stmt(Stmt::Let { local: vd, init });
        // let id = is * ((vd / nvt).exp() - 1.0);
        let mut e = Ex::new(b, Self::FILE, 40, 17);
        let p = e.param(is);
        let v = e.local(vd);
        let nv = e.local(nvt);
        let ratio = e.binary(BinaryOp::Div, v, nv);
        let ex = e.math(MathFn::Exp, ratio);
        let one = e.real(1.0);
        let minus = e.binary(BinaryOp::Sub, ex, one);
        let init = e.binary(BinaryOp::Mul, p, minus);
        let let_id = b.stmt(Stmt::Let { local: id, init });
        // i(a, k) +> id + gmin() * vd + ddt(cj0 * vd);
        let mut e = Ex::new(b, Self::FILE, 41, 17);
        let i = e.local(id);
        let g = e.env(Env::Gmin);
        let v = e.local(vd);
        let shunt = e.binary(BinaryOp::Mul, g, v);
        let direct = e.binary(BinaryOp::Add, i, shunt);
        let c = e.param(cj0);
        let v = e.local(vd);
        let charge = e.binary(BinaryOp::Mul, c, v);
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![
                Addend { operator: Operator::Direct, negated: false, value: direct },
                Addend { operator: Operator::Ddt, negated: false, value: charge },
            ],
        });
        let local = |name: &str| Local { name: name.to_string(), ty: ValueType::F64 };
        b.body(BodySpec {
            name: "Diode".into(),
            origin: Self::origin(31, 1),
            params: vec![
                Self::param("is", Some(1e-14), Some(checks[0]), 32),
                Self::param("n", Some(1.0), Some(checks[1]), 33),
                Self::param("cj0", Some(0.0), Some(checks[2]), 34),
            ],
            nodes: vec![Self::port("a", el, 31, 18), Self::port("k", el, 31, 33)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![Site { name: "vd".into(), origin: Self::origin(39, 17), enabled: None }],
            stims: vec![],
            locals: vec![local("nvt"), local("vcrit"), local("vd"), local("id"), local("new"), local("old")],
            analog: Some(vec![let_nvt, let_vcrit, let_vd, let_id, contribute]),
            children: vec![],
        })
    }

    /// The body alone as the design: a top instance named after it, both ports on ground, the
    /// given param values. For the two-port devices only.
    pub fn alone(b: &mut IrBuilder, body: BodyId, name: &str, line: u32, params: Vec<f64>) -> InstanceId {
        b.instance(InstanceSpec {
            path: name.to_string(),
            name: name.to_string(),
            body,
            parent: None,
            nodes: vec![NodeRef::Ground, NodeRef::Ground],
            params: params.into_iter().map(|v| Constant::Real(Real::new(v))).collect(),
            origin: Self::origin(line, 1),
        })
    }

    fn everything_origin() -> Origin {
        Origin { file: "all.msl".into(), line: 1, col: 1 }
    }

    /// A body that constructs every `Expr`, `Stmt`, `Addend`, `Terminal` and `Site` variant the
    /// five devices leave out.
    pub fn everything(b: &mut IrBuilder) -> BodyId {
        let el = Self::electrical(b);
        let f64_local = |name: &str| Local { name: name.to_string(), ty: ValueType::F64 };
        // fn id(x: f64) -> f64 { x }
        let mut e = Ex::new(b, "all.msl", 2, 1);
        let x = e.local(0);
        let function = b.function(FunctionSpec {
            name: "identity".into(),
            origin: Self::everything_origin(),
            params: vec![f64_local("x")],
            ret: ValueType::F64,
            locals: vec![f64_local("x")],
            body: x,
        });
        let mut e = Ex::new(b, "all.msl", 3, 1);
        let one = e.real(1.0);
        let two = e.real(2.0);
        let sum = e.binary(BinaryOp::Add, one, two);
        let let_x = b.stmt(Stmt::Let { local: 0, init: sum });
        let mut e = Ex::new(b, "all.msl", 4, 1);
        // a block with a let and a value, inside a statement
        let t = e.boolean(true);
        let not = e.unary(UnaryOp::Not, t);
        let unit = e.unit();
        let inner_let = e.local(0);
        let blk_let = e.b.stmt(Stmt::Let { local: 1, init: inner_let });
        let mut e = Ex::new(b, "all.msl", 5, 1);
        let y = e.local(1);
        let neg = e.unary(UnaryOp::Neg, y);
        let abs = e.math(MathFn::Abs, neg);
        let block = e.block(vec![blk_let], Some(abs), ValueType::F64);
        let block_stmt = b.stmt(Stmt::Expr(block));
        let unit_stmt = b.stmt(Stmt::Expr(unit));
        let mut e = Ex::new(b, "all.msl", 6, 1);
        let tt = e.boolean(true);
        let cond = e.binary(BinaryOp::Or, not, tt);
        let a1 = e.real(1.0);
        let a2 = e.real(2.0);
        let pick = e.if_f64(cond, a1, a2);
        let ln = e.math(MathFn::Ln, pick);
        let ex = e.math(MathFn::Exp, ln);
        let sq = e.math(MathFn::Sqrt, ex);
        let call_arg = e.local(0);
        let called = e.call(function, vec![call_arg]);
        let temp = e.env(Env::Temperature);
        let probe = e.probe(Terminal::Node(0), Terminal::Ground);
        let seed = e.real(0.0);
        let body_local = e.local(2);
        let closure = e.closure(vec![2], body_local);
        let limited = e.limit(0, probe, closure, Some(seed));
        let mag = e.real(1.0);
        let phase = e.real(90.0);
        let stim = e.stim(0, mag, Some(phase));
        let s1 = e.binary(BinaryOp::Add, sq, called);
        let s2 = e.binary(BinaryOp::Add, s1, temp);
        let s3 = e.binary(BinaryOp::Add, s2, limited);
        let total = e.binary(BinaryOp::Add, s3, stim);
        let contribute = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Potential,
            addends: vec![Addend { operator: Operator::Ddt, negated: true, value: total }],
        });
        b.body(BodySpec {
            name: "Everything".into(),
            origin: Self::everything_origin(),
            params: vec![
                Param {
                    name: "flag".into(),
                    ty: ValueType::Bool,
                    default: Some(Constant::Bool(true)),
                    check: None,
                    structural: true,
                    origin: Self::everything_origin(),
                },
            ],
            nodes: vec![
                BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: Self::everything_origin() },
                BodyNode { name: "s".into(), nature: el, role: NodeRole::Signal, init: Some(Real::new(0.5)), origin: Self::everything_origin() },
            ],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Potential }],
            limits: vec![Site { name: "lim".into(), origin: Self::everything_origin(), enabled: None }],
            stims: vec![Site { name: "stim".into(), origin: Self::everything_origin(), enabled: None }],
            locals: vec![f64_local("x"), f64_local("y"), f64_local("t")],
            analog: Some(vec![let_x, block_stmt, unit_stmt, contribute]),
            children: vec![],
        })
    }

    fn child(name: &str, body: BodyId, nodes: Vec<Terminal>, params: Vec<(u32, f64)>, line: u32) -> Child {
        Child {
            name: name.to_string(),
            body,
            nodes,
            params: params.into_iter().map(|(i, v)| (i, Constant::Real(Real::new(v)))).collect(),
            origin: Origin { file: "divider.msl".into(), line, col: 5 },
        }
    }

    /// Twin of `tests/e2e/twins/divider.msl`: the top `Divider` with signals `vin` and `mid`, and
    /// `v1`, `r1`, `r2`. Paths `Divider`, `Divider.v1`, `Divider.r1`, `Divider.r2`, `Divider.vin`,
    /// `Divider.mid`.
    pub fn divider(b: &mut IrBuilder) -> InstanceId {
        let el = Self::electrical(b);
        let vsource = Self::vsource(b);
        let resistor = Self::resistor(b);
        let at = |line: u32, col: u32| Origin { file: "divider.msl".into(), line, col };
        let signal = |name: &str, line: u32| BodyNode {
            name: name.to_string(),
            nature: el,
            role: NodeRole::Signal,
            init: None,
            origin: at(line, 5),
        };
        let (vin, mid, gnd) = (Terminal::Node(0), Terminal::Node(1), Terminal::Ground);
        let body = b.body(BodySpec {
            name: "Divider".into(),
            origin: at(5, 1),
            params: vec![],
            nodes: vec![signal("vin", 6), signal("mid", 7)],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![
                Self::child("v1", vsource, vec![vin, gnd], vec![(0, 10.0)], 8),
                Self::child("r1", resistor, vec![vin, mid], vec![(0, 1000.0)], 9),
                Self::child("r2", resistor, vec![mid, gnd], vec![(0, 3000.0)], 10),
            ],
        });
        // Pre-order: the top instance is the next id, then its children. Its nodes are added
        // first because the top binds them.
        let top = b.next_instance_id();
        let node = |path: &str, line: u32| NodeSpec {
            path: path.to_string(),
            nature: el,
            declared_by: top,
            init: None,
            origin: at(line, 5),
        };
        let n_vin = b.node(node("Divider.vin", 6));
        let n_mid = b.node(node("Divider.mid", 7));
        let top = b.instance(InstanceSpec {
            path: "Divider".into(),
            name: "Divider".into(),
            body,
            parent: None,
            nodes: vec![NodeRef::Node(n_vin), NodeRef::Node(n_mid)],
            params: vec![],
            origin: at(5, 1),
        });
        let real = |v: f64| Constant::Real(Real::new(v));
        let placements = [
            ("v1", vsource, [NodeRef::Node(n_vin), NodeRef::Ground], vec![real(10.0), real(0.0)], 8),
            ("r1", resistor, [NodeRef::Node(n_vin), NodeRef::Node(n_mid)], vec![real(1000.0)], 9),
            ("r2", resistor, [NodeRef::Node(n_mid), NodeRef::Ground], vec![real(3000.0)], 10),
        ];
        for (name, body, nodes, params, line) in placements {
            b.instance(InstanceSpec {
                path: format!("Divider.{name}"),
                name: name.to_string(),
                body,
                parent: Some(top),
                nodes: nodes.to_vec(),
                params,
                origin: at(line, 5),
            });
        }
        top
    }
}
