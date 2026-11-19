//! The shape of each `analog-structure` form: switch branches and conditional contributions
//! (AS-01, AS-03), derivative sites and their `abstol` pins (AS-07), guarded `limit` and
//! `bound_step` sites, param events and reached carriers (AS-06, AS-14, AS-11). Each test builds
//! a small body by hand and reads its shape entry by entry; every expected entry is written from
//! the design (mechanisms 1 to 3) and abi.md, and every shape passes `piperine_abi::validate`.

use piperine_abi::raw::{GROUND, Shape, kind, op, role};
use piperine_ir::{
    Addend, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Constant, Derivative, Expr, ExprId, InstanceSpec, Ir,
    IrBuilder, NetSpec, NodeRef, NodeRole, Operator, Origin, Param, QuantityRole, Real, Site, Stmt, StmtId, Terminal, ValueType,
};
use piperine_plan::{BodyPlan, Mode, Source, Target, Term};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;

use devices::{Devices, Ex};

/// What a test writes into the body `Sw` that `Switching` builds.
#[derive(Default)]
struct Written {
    analog: Vec<StmtId>,
    derivatives: Vec<Derivative>,
    limits: Vec<Site>,
    stims: Vec<Site>,
}

/// One body `Sw(a)` with the branch `(a, gnd)` of a given kind, one param `tol: f64`, a `Bool`
/// input carrier `en`, and an `analog` block the test writes; placed alone as the design.
struct Switching;

impl Switching {
    const FILE: &'static str = "structure_shape.msl";

    fn at() -> Origin {
        Origin { file: Self::FILE.into(), line: 3, col: 5 }
    }

    /// The expression helper at a fixed position.
    fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 3, 5)
    }

    /// `i(a, gnd) +> value;` or `v(a, gnd) +> value;`.
    fn contribute(b: &mut IrBuilder, quantity: QuantityRole, value: f64) -> StmtId {
        let value = Self::ex(b).real(value);
        Self::contribute_expr(b, quantity, Operator::Direct, value)
    }

    /// A contribution of `value` with the given operator.
    fn contribute_expr(b: &mut IrBuilder, quantity: QuantityRole, operator: Operator, value: ExprId) -> StmtId {
        b.stmt(Stmt::Contribute { branch: 0, quantity, addends: vec![Addend { operator, negated: false, value }] })
    }

    /// `if cond { then } else { otherwise }` as a statement.
    fn branch_on(b: &mut IrBuilder, cond: ExprId, then: Vec<StmtId>, otherwise: Vec<StmtId>) -> StmtId {
        let then = Self::ex(b).block(then, None, ValueType::Unit);
        let otherwise = Self::ex(b).block(otherwise, None, ValueType::Unit);
        let expr = b.expr(Expr::If { cond, then, otherwise }, ValueType::Unit, Self::at());
        b.stmt(Stmt::Expr(expr))
    }

    /// `en.high()` of the carrier `en`.
    fn carrier_high(b: &mut IrBuilder) -> ExprId {
        let read = b.expr(Expr::Carrier(0), ValueType::Flagged(Box::new(ValueType::Bool)), Self::at());
        b.expr(Expr::High(read), ValueType::Bool, Self::at())
    }

    /// `v(a, gnd)`.
    fn probe(b: &mut IrBuilder) -> ExprId {
        Self::ex(b).probe(Terminal::Node(0), Terminal::Ground)
    }

    /// The design: `kind` is the kind of branch 0; `write` returns what the body holds.
    fn ir(kind: BranchKind, write: impl FnOnce(&mut IrBuilder) -> Written) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let origin = Origin { file: Self::FILE.into(), line: 1, col: 1 };
        let Written { analog, derivatives, limits, stims } = write(&mut b);
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Sw".into(),
            origin: origin.clone(),
            params: vec![tolerance],
            nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind }],
            limits,
            stims,
            locals: vec![],
            analog: Some(analog),
            children: vec![],
        });
        b.carrier(body, Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: origin.clone() });
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

    /// The plan of the only body of `ir`.
    fn plan(ir: &Ir) -> BodyPlan {
        BodyPlan::new(ir.bodies().next().expect("a body"))
    }
}

/// Entries of a shape, found by name.
struct Find;

impl Find {
    fn pin(shape: &Shape<'_>, name: &str) -> u32 {
        shape.pins.iter().position(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`")) as u32
    }

    /// The terms of a shape as `(name, op, plus, minus)`.
    fn terms(shape: &Shape<'_>) -> Vec<(String, u32, u32, u32)> {
        shape.terms.iter().map(|t| (t.name.as_str().to_string(), t.op, t.plus, t.minus)).collect()
    }

    /// The plan of the `terms` function.
    fn terms_plan(plan: &BodyPlan) -> &piperine_plan::FunctionPlan {
        plan.functions().iter().find(|f| f.mode == Mode::Terms).expect("a terms function")
    }
}

/// `if en.high() { v(a, gnd) +> 1.0; } else { i(a, gnd) +> 2.0; }`: a switch branch.
fn switching_on_the_carrier(b: &mut IrBuilder) -> Written {
    let (cond, potential, flow) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::contribute(b, QuantityRole::Flow, 2.0));
    Written { analog: vec![Switching::branch_on(b, cond, vec![potential], vec![flow])], ..Written::default() }
}

#[test]
fn a_switch_branch_has_a_flow_pin_and_an_equation_term() {
    // Design mechanism 2: a switch branch has the shape of a potential branch: the private flow
    // pin `b0.flow`, the term `b0.flow` = FLOW {a, gnd}, and the term `b0.eq` = FLOW {b0.flow, gnd}.
    let ir = Switching::ir(BranchKind::Switch, switching_on_the_carrier);
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    let flow = Find::pin(shape, "b0.flow");
    let a = Find::pin(shape, "a");
    let pin = shape.pins.get(flow as usize).expect("the pin");
    assert_eq!((pin.kind, pin.role), (kind::CONTINUOUS, role::PRIVATE));
    assert_eq!(Find::terms(shape), [("b0.flow".into(), op::FLOW, a, GROUND), ("b0.eq".into(), op::FLOW, flow, GROUND)]);
    let terms = Find::terms_plan(&plan);
    assert_eq!(terms.writes, [Target::Term(Term::Current(0)), Target::Term(Term::Eq(0))]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_conditional_flow_contribution_reads_its_condition() {
    // `if en.high() { i(a, gnd) +> v(a, gnd); }` on a flow branch: the `b0.flow` term reads the
    // carrier the condition reads (`terms` reads the sources of every enclosing condition).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (cond, probe) = (Switching::carrier_high(b), Switching::probe(b));
        let taken = Switching::contribute_expr(b, QuantityRole::Flow, Operator::Direct, probe);
        Written { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(Find::terms(shape), [("b0.flow".into(), op::FLOW, Find::pin(shape, "a"), GROUND)]);
    let terms = Find::terms_plan(&plan);
    assert_eq!(terms.reads, [Source::Node(0), Source::Carrier(0)], "the node it probes, then the carrier its condition reads");
    assert_eq!(terms.slots, 1, "only the node is a differentiable read");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_switch_equation_is_not_affine() {
    // The condition reads a carrier only (Constant in the potentials), yet the equation a switch
    // branch selects per solve is never affine (design mechanism 2). A potential branch with an
    // unconditional constant contribution stays affine, as today.
    let switch = Switching::ir(BranchKind::Switch, switching_on_the_carrier);
    assert_eq!(Find::terms_plan(&Switching::plan(&switch)).affine, [true, false], "b0.flow is linear in the potential, b0.eq is not");
    let potential = Switching::ir(BranchKind::Potential, |b| Written { analog: vec![Switching::contribute(b, QuantityRole::Potential, 1.0)], ..Written::default() });
    assert_eq!(Find::terms_plan(&Switching::plan(&potential)).affine, [true, true]);
}

#[test]
fn a_body_without_new_forms_keeps_its_shape() {
    // A potential branch with a `ddt` addend: the pins and the terms are those of abi.md A1, in
    // the order they had before this feature (`b0.flow`, `b0.eq`, `b0.charge`).
    let ir = Switching::ir(BranchKind::Potential, |b| {
        let (potential, charge) = (Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::probe(b));
        let charge = Switching::contribute_expr(b, QuantityRole::Potential, Operator::Ddt, charge);
        Written { analog: vec![potential, charge], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    let (a, flow) = (Find::pin(shape, "a"), Find::pin(shape, "b0.flow"));
    let names: Vec<&str> = shape.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["tol", "a", "b0.flow", "en"]);
    assert_eq!(
        Find::terms(shape),
        [("b0.flow".into(), op::FLOW, a, GROUND), ("b0.eq".into(), op::FLOW, flow, GROUND), ("b0.charge".into(), op::DDT, flow, GROUND)]
    );
    assert_eq!(Find::terms_plan(&plan).affine, [true, true, false]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}
