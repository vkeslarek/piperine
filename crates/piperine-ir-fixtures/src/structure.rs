//! The small bodies the `analog-structure` tests of the plan, the interpreter, the transpiler and the printer
//! share: one body `Sw(a)` with one branch of a chosen kind, a param, an input carrier and an `analog` block the
//! test writes (`Switching`), and one body `Lag` with a param event and a reaction (`ParamEvents`).

use crate::devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Constant, Derivative, Drive, Event, EventSource, Expr, ExprId, InstanceSpec,
    Ir, IrBuilder, Local, NetSpec, NodeRef, NodeRole, Operator, Origin, Param, QuantityRole, Reaction, Real, Site, Stmt, StmtId, Terminal, ValueType,
};

/// What a test writes into the body `Sw` that `Switching` builds.
#[derive(Default)]
pub struct SwitchingWrites {
    /// The statements of the `analog` block.
    pub analog: Vec<StmtId>,
    /// The derivative sites of the body.
    pub derivatives: Vec<Derivative>,
    /// The `limit` sites of the body.
    pub limits: Vec<Site>,
    /// The `ac_stim` sites of the body.
    pub stims: Vec<Site>,
    /// The body's locals: the parameters of a `limit` closure.
    pub locals: Vec<Local>,
}

/// One body `Sw(a)` with the branch `(a, gnd)` of a given kind, one param `tol: f64`, a `Bool`
/// input carrier `en`, and an `analog` block the test writes; placed alone as the design.
pub struct Switching;

impl Switching {
    /// The file every node of `Switching` is attributed to.
    pub const FILE: &'static str = "structure_shape.msl";

    /// The origin every form of `Switching` carries: line 3, column 5 of `structure_shape.msl`.
    pub fn at() -> Origin {
        Origin { file: Self::FILE.into(), line: 3, col: 5 }
    }

    /// The expression helper at a fixed position.
    pub fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 3, 5)
    }

    /// `i(a, gnd) +> value;` or `v(a, gnd) +> value;`.
    pub fn contribute(b: &mut IrBuilder, quantity: QuantityRole, value: f64) -> StmtId {
        let value = Self::ex(b).real(value);
        Self::contribute_expr(b, quantity, Operator::Direct, value)
    }

    /// A contribution of `value` with the given operator.
    pub fn contribute_expr(b: &mut IrBuilder, quantity: QuantityRole, operator: Operator, value: ExprId) -> StmtId {
        b.stmt(Stmt::Contribute { branch: 0, quantity, addends: vec![Addend { operator, negated: false, value }] })
    }

    /// `if cond { then } else { otherwise }` as a statement.
    pub fn branch_on(b: &mut IrBuilder, cond: ExprId, then: Vec<StmtId>, otherwise: Vec<StmtId>) -> StmtId {
        let then = Self::ex(b).block(then, None, ValueType::Unit);
        let otherwise = Self::ex(b).block(otherwise, None, ValueType::Unit);
        let expr = b.expr(Expr::If { cond, then, otherwise }, ValueType::Unit, Self::at());
        b.stmt(Stmt::Expr(expr))
    }

    /// A read of the `bool` carrier `en`.
    pub fn carrier_high(b: &mut IrBuilder) -> ExprId {
        b.expr(Expr::Carrier(0), ValueType::Bool, Self::at())
    }

    /// `<quantity>(a, gnd) +> 2.0 * rate(D<k>);`
    pub fn contribute_twice_the_rate(b: &mut IrBuilder, quantity: QuantityRole, k: u32) -> StmtId {
        let rate = b.expr(Expr::Derivative(k), ValueType::F64, Self::at());
        let mut e = Self::ex(b);
        let two = e.real(2.0);
        let value = e.binary(BinaryOp::Mul, two, rate);
        Self::contribute_expr(b, quantity, Operator::Direct, value)
    }

    /// The derivative site `ddt(v(a, gnd), <abstol>)` named `q`.
    pub fn site(b: &mut IrBuilder, abstol: Option<ExprId>) -> Derivative {
        let state = Self::probe(b);
        Derivative { name: "q".into(), state, abstol, origin: Self::at() }
    }

    /// `v(a, gnd) <op> edge`, a `bool`.
    pub fn compare(b: &mut IrBuilder, op: BinaryOp, edge: f64) -> ExprId {
        let (probe, edge) = (Self::probe(b), Self::ex(b).real(edge));
        Self::ex(b).binary(op, probe, edge)
    }

    /// `v(a, gnd)`.
    pub fn probe(b: &mut IrBuilder) -> ExprId {
        Self::ex(b).probe(Terminal::Node(0), Terminal::Ground)
    }

    /// The design: `kind` is the kind of branch 0; `write` returns what the body holds.
    pub fn ir(kind: BranchKind, write: impl FnOnce(&mut IrBuilder) -> SwitchingWrites) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let origin = Origin { file: Self::FILE.into(), line: 1, col: 1 };
        let SwitchingWrites { analog, derivatives, limits, stims, locals } = write(&mut b);
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Sw".into(),
            origin: origin.clone(),
            params: vec![tolerance],
            nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind }],
            limits,
            stims,
            locals,
            analog: Some(analog),
            children: vec![],
        });
        b.carrier(body, Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: origin.clone(), count: None });
        for site in derivatives {
            b.derivative(body, site);
        }
        let top = b.next_instance_id();
        let net = b.net(NetSpec { path: "Sw.en".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: origin.clone() });
        let params = vec![Constant::Real(Real::new(1e-9))];
        let top = b.instance(InstanceSpec { path: "Sw".into(), name: "Sw".into(), body, parent: None, nodes: vec![NodeRef::Ground], params, origin });
        b.bind_nets(top, vec![net]);
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check(): {ir}");
        ir
    }

    /// `if en { v(a, gnd) +> 1.0; } else { i(a, gnd) +> 2.0; }`: a switch branch.
    pub fn on_the_carrier(b: &mut IrBuilder) -> SwitchingWrites {
        let (cond, potential, flow) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::contribute(b, QuantityRole::Flow, 2.0));
        SwitchingWrites { analog: vec![Switching::branch_on(b, cond, vec![potential], vec![flow])], ..SwitchingWrites::default() }
    }

    /// The two `f64` locals `new` and `old` that `halving_limit` closes over.
    pub fn limit_locals() -> Vec<Local> {
        ["new", "old"].iter().map(|name| Local { name: (*name).into(), ty: ValueType::F64, mutable: false }).collect()
    }

    /// `limit(v(a, gnd), |new, old| old + (new - old) * 0.5)` as site 0, over the locals of
    /// `limit_locals`: it moves half way from the held value to the proposed one.
    pub fn halving_limit(b: &mut IrBuilder) -> ExprId {
        let x = Self::probe(b);
        let mut e = Self::ex(b);
        let (new, old, held, half) = (e.local(0), e.local(1), e.local(1), e.real(0.5));
        let step = e.binary(BinaryOp::Sub, new, old);
        let scaled = e.binary(BinaryOp::Mul, step, half);
        let moved = e.binary(BinaryOp::Add, held, scaled);
        let closure = e.closure(vec![0, 1], moved);
        b.expr(Expr::Limit { site: 0, x, f: closure, seed: None }, ValueType::F64, Self::at())
    }

    /// `limit(v(a, gnd), |..| 0.0)` as site 0.
    pub fn limit_read(b: &mut IrBuilder) -> ExprId {
        let x = Self::probe(b);
        let function = Self::ex(b).real(0.0);
        let closure = b.expr(Expr::Closure { params: vec![], body: function }, ValueType::Fn(vec![], Box::new(ValueType::F64)), Self::at());
        b.expr(Expr::Limit { site: 0, x, f: closure, seed: None }, ValueType::F64, Self::at())
    }
}

/// A body `Lag` with one param `tol: f64`, an `out` carrier `seen`, the event `param_changed tol`
/// and one reaction on it that commits what a test builds to `seen`.
pub struct ParamEvents;

impl ParamEvents {
    /// The origin of the event and the reaction: line 2, column 3 of `param_events.msl`.
    pub fn at() -> Origin {
        Origin { file: "param_events.msl".into(), line: 2, col: 3 }
    }

    /// The design: the body `Lag` whose reaction commits what `value` builds, to a `seen` carrier of type `seen_ty`.
    pub fn ir(seen_ty: ValueType, value: impl FnOnce(&mut IrBuilder) -> ExprId) -> Ir {
        let mut b = IrBuilder::new();
        let origin = Origin { file: "param_events.msl".into(), line: 1, col: 1 };
        let value = value(&mut b);
        let commit = b.stmt(Stmt::Commit { carrier: 0, value, after: None, target: None });
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec { name: "Lag".into(), origin: origin.clone(), params: vec![tolerance], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        b.carrier(body, Carrier { name: "seen".into(), ty: seen_ty.clone(), role: CarrierRole::Out, init: None, origin: origin.clone(), count: None });
        let event = b.event(body, Event { source: EventSource::ParamChanged(0), guard: None, origin: Self::at() });
        b.reaction(body, Reaction { on: vec![event], stmts: vec![commit], locals: vec![], origin: Self::at() });
        let top = b.next_instance_id();
        let net = b.net(NetSpec { path: "Lag.seen".into(), ty: seen_ty, declared_by: top, init: None, origin: origin.clone() });
        let top = b.instance(InstanceSpec { path: "Lag".into(), name: "Lag".into(), body, parent: None, nodes: vec![], params: vec![Constant::Real(Real::new(1e-9))], origin });
        b.bind_nets(top, vec![net]);
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check(): {ir}");
        ir
    }

    /// `Lag` with the drive `seen = tol.prev();` and no event: the value a drive writes is the
    /// previous value of the param `tol`.
    pub fn previous_drive() -> Ir {
        let mut b = IrBuilder::new();
        let origin = Origin { file: "param_events.msl".into(), line: 1, col: 1 };
        let value = b.expr(Expr::ParamPrevious(0), ValueType::F64, Self::at());
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec { name: "Lag".into(), origin: origin.clone(), params: vec![tolerance], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        b.carrier(body, Carrier { name: "seen".into(), ty: ValueType::F64, role: CarrierRole::Out, init: None, origin: origin.clone(), count: None });
        b.drive(body, Drive { carrier: 0, value, after: None, origin: Self::at(), element: None, path: vec![] });
        let top = b.next_instance_id();
        let net = b.net(NetSpec { path: "Lag.seen".into(), ty: ValueType::F64, declared_by: top, init: None, origin: origin.clone() });
        let top = b.instance(InstanceSpec { path: "Lag".into(), name: "Lag".into(), body, parent: None, nodes: vec![], params: vec![Constant::Real(Real::new(1e-9))], origin });
        b.bind_nets(top, vec![net]);
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check(): {ir}");
        ir
    }
}
