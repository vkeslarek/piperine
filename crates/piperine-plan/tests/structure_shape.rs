//! The shape of each `analog-structure` form: switch branches and conditional contributions
//! (AS-01, AS-03), derivative sites and their `abstol` pins (AS-07), guarded `limit` and
//! `bound_step` sites, param events and reached carriers (AS-06, AS-14, AS-11). Each test builds
//! a small body by hand and reads its shape entry by entry; every expected entry is written from
//! the design (mechanisms 1 to 3) and abi.md, and every shape passes `piperine_abi::validate`.

use piperine_abi::raw::{Cell, DEFAULT, GROUND, Shape, cell, kind, op, part, role};
use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Child, Constant, Derivative, Event, EventSource, Expr,
    ExprId, InstanceSpec, Ir, IrBuilder, NetSpec, NodeRef, NodeRole, Operator, Origin, Param, QuantityRole, Reach, ReachedItem, Reaction, Real, Site,
    Stmt, StmtId, Terminal, ValueType,
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

    /// `<quantity>(a, gnd) +> 2.0 * rate(D<k>);`
    fn contribute_twice_the_rate(b: &mut IrBuilder, quantity: QuantityRole, k: u32) -> StmtId {
        let rate = b.expr(Expr::Derivative(k), ValueType::F64, Self::at());
        let mut e = Self::ex(b);
        let two = e.real(2.0);
        let value = e.binary(BinaryOp::Mul, two, rate);
        Self::contribute_expr(b, quantity, Operator::Direct, value)
    }

    /// The derivative site `ddt(v(a, gnd), <abstol>)` named `q`.
    fn site(b: &mut IrBuilder, abstol: Option<ExprId>) -> Derivative {
        let state = Self::probe(b);
        Derivative { name: "q".into(), state, abstol, origin: Self::at() }
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

    /// Function `name`: its reads, its writes and its partials as `(write, read)`.
    fn function(shape: &Shape<'_>, name: &str) -> (Vec<Cell>, Vec<Cell>, Vec<(u32, u32)>) {
        let f = shape.functions.iter().find(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        (f.reads.iter().copied().collect(), f.writes.iter().copied().collect(), f.partials.iter().map(|p| (p.write, p.read)).collect())
    }

    /// The names of the pins of a shape, in order.
    fn pin_names<'s>(shape: &'s Shape<'_>) -> Vec<&'s str> {
        shape.pins.iter().map(|p| p.name.as_str()).collect()
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

/// The cell of a pin's value.
fn value(pin: u32) -> Cell {
    Cell { kind: cell::PIN, index: pin, part: part::VALUE }
}

#[test]
fn a_derivative_site_has_a_rate_pin_and_two_terms() {
    // `v(a, gnd) +> 2.0 * ddt(v(a, gnd))` on a potential branch (design mechanism 3): the pin
    // `d0.rate` follows the branch flow pin, and its two terms follow the branch terms.
    let ir = Switching::ir(BranchKind::Potential, |b| {
        let site = Switching::site(b, None);
        Written { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Potential, 0)], derivatives: vec![site], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(Find::pin_names(shape), ["tol", "a", "b0.flow", "d0.rate", "en"]);
    let (a, flow, rate) = (Find::pin(shape, "a"), Find::pin(shape, "b0.flow"), Find::pin(shape, "d0.rate"));
    let pin = shape.pins.get(rate as usize).expect("the rate pin");
    assert_eq!((pin.kind, pin.role), (kind::CONTINUOUS, role::PRIVATE));
    assert_eq!(
        Find::terms(shape),
        [
            ("b0.flow".into(), op::FLOW, a, GROUND),
            ("b0.eq".into(), op::FLOW, flow, GROUND),
            ("d0.rate".into(), op::FLOW, rate, GROUND),
            ("d0.state".into(), op::DDT, rate, GROUND),
        ]
    );
    assert_eq!(shape.terms.iter().map(|t| t.abstol).collect::<Vec<_>>(), [DEFAULT; 4], "no site tolerance: every term has the default");
    let terms = Find::terms_plan(&plan);
    assert_eq!(terms.writes[2..], [Target::Term(Term::DerivativeRate(0)), Target::Term(Term::DerivativeState(0))]);
    assert_eq!(terms.affine, [true, true, true, false], "the rate term is linear in its pin, the state term is a charge");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_derivative_abstol_names_its_pin() {
    // `ddt(v(a, gnd), tol)`: a private discrete `F64` pin `d0.abstol` after the event pins, written
    // by the function `d0.abstol` from the param cone, and named by the `d0.state` term.
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let tolerance = b.expr(Expr::Param(0), ValueType::F64, Switching::at());
        let site = Switching::site(b, Some(tolerance));
        Written { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0)], derivatives: vec![site], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(Find::pin_names(shape), ["tol", "a", "d0.rate", "en", "d0.abstol"]);
    let abstol = Find::pin(shape, "d0.abstol");
    let pin = shape.pins.get(abstol as usize).expect("the abstol pin");
    assert_eq!((pin.kind, pin.role), (kind::DISCRETE, role::PRIVATE));
    let state = shape.terms.iter().find(|t| t.name.as_str() == "d0.state").expect("the state term");
    assert_eq!(state.abstol, abstol, "the DDT term names the pin");
    let rate = shape.terms.iter().find(|t| t.name.as_str() == "d0.rate").expect("the rate term");
    assert_eq!(rate.abstol, DEFAULT, "only the DDT term has a tolerance");
    let (reads, writes, partials) = Find::function(shape, "d0.abstol");
    assert_eq!((reads, writes, partials), (vec![value(0)], vec![value(abstol)], vec![]), "it reads the param `tol` alone and has no partial");
    let function = plan.functions().iter().find(|f| f.mode == Mode::DerivativeAbstol(0)).expect("the abstol function");
    assert_eq!((function.reads.clone(), function.writes.clone()), (vec![Source::Param(0)], vec![Target::DerivativeAbstol(0)]));
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_derivative_read_is_a_differentiable_pin_read() {
    // `i(a, gnd) +> 2.0 * rate(D0)`: `terms` reads the node `a` (the state's) and the rate pin; both
    // are tangent slots, and the contribution has a partial against the rate (abi.md A1: the
    // rate is an unknown like a flow).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let site = Switching::site(b, None);
        Written { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0)], derivatives: vec![site], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    assert!(Source::Derivative(0).is_differentiable() && Source::Derivative(0).is_pin());
    let terms = Find::terms_plan(&plan);
    assert_eq!(terms.reads, [Source::Node(0), Source::Derivative(0)]);
    assert_eq!(terms.slots, 2);
    // Writes: b0.flow, d0.rate, d0.state. Partials as (write, slot): the flow reads the rate, the
    // rate term is the rate itself, the state term reads the node the site differentiates.
    assert_eq!(terms.partials, [(0, 1), (1, 1), (2, 0)]);
    let (reads, _, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [value(Find::pin(shape, "a")), value(Find::pin(shape, "d0.rate"))]);
    assert_eq!(partials, [(0, 1), (1, 1), (2, 0)]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

/// `limit(v(a, gnd), |..| 0.0)` as site 0.
fn limit_read(b: &mut IrBuilder) -> ExprId {
    let x = Switching::probe(b);
    let function = Switching::ex(b).real(0.0);
    let closure = b.expr(Expr::Closure { params: vec![], body: function }, ValueType::Fn(vec![], Box::new(ValueType::F64)), Switching::at());
    b.expr(Expr::Limit { site: 0, x, f: closure, seed: None }, ValueType::F64, Switching::at())
}

#[test]
fn a_guarded_limit_reads_its_enabled_sources() {
    // `if en.high() { i(a, gnd) +> limit(v(a, gnd), ..); }` with `enabled = en.high()`: the
    // function `limit:lim` reads the carrier `en` its `enabled` reads, after what the site reads
    // itself and before its own `LAST`, `AGE` and the ambient `STATIC`.
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (read, cond, enabled) = (limit_read(b), Switching::carrier_high(b), Switching::carrier_high(b));
        let taken = Switching::contribute_expr(b, QuantityRole::Flow, Operator::Direct, read);
        let site = Site { name: "lim".into(), origin: Switching::at(), enabled: Some(enabled) };
        Written { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], limits: vec![site], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Limit(0)).expect("the limit function");
    assert_eq!(function.reads, [Source::Node(0), Source::Carrier(0), Source::LimitLast(0), Source::LimitAge(0), Source::Static]);
    let (reads, _, _) = Find::function(shape, "limit:lim");
    assert_eq!(reads[1], value(Find::pin(shape, "en")), "the carrier's pin value");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_guarded_cap_reads_its_when_sources() {
    // `bound_step(1n) when en.high();`: the `CAP` watch's function `cap:0` reads the carrier.
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(1e-9), Switching::carrier_high(b));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Cap(0)).expect("the cap function");
    assert_eq!((function.reads.clone(), function.writes.clone()), (vec![Source::Carrier(0)], vec![Target::Cap(0)]));
    let (reads, _, _) = Find::function(shape, "cap:0");
    assert_eq!(reads, [value(Find::pin(shape, "en"))]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

/// A body `Lag` with one param `tol: f64`, an `out` carrier `seen`, the event `param_changed tol`
/// and one reaction on it that commits what a test builds to `seen`.
struct ParamEvents;

impl ParamEvents {
    fn at() -> Origin {
        Origin { file: "param_events.msl".into(), line: 2, col: 3 }
    }

    fn ir(seen_ty: ValueType, value: impl FnOnce(&mut IrBuilder) -> ExprId) -> Ir {
        let mut b = IrBuilder::new();
        let origin = Origin { file: "param_events.msl".into(), line: 1, col: 1 };
        let value = value(&mut b);
        let commit = b.stmt(Stmt::Commit { carrier: 0, value, after: None });
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec { name: "Lag".into(), origin: origin.clone(), params: vec![tolerance], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        b.carrier(body, Carrier { name: "seen".into(), ty: seen_ty.clone(), role: CarrierRole::Out, init: None, origin: origin.clone() });
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
}

#[test]
fn a_param_changed_event_reads_the_params_changed_byte() {
    // `on tol.changed() { seen <- fired(E0); }`: the event function `event:0` reads the param's
    // value and its `CHANGED` byte (abi.md §4.1) and writes the event pin's `CHANGED`.
    let ir = ParamEvents::ir(ValueType::Bool, |b| b.expr(Expr::Fired(0), ValueType::Bool, ParamEvents::at()));
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Event(0)).expect("the event function");
    assert_eq!((function.reads.clone(), function.writes.clone()), (vec![Source::Param(0), Source::ParamChanged(0)], vec![Target::EventPin(0)]));
    let (reads, writes, _) = Find::function(shape, "event:0");
    let event = Find::pin(shape, "event:0");
    assert_eq!(reads, [value(0), Cell { kind: cell::PIN, index: 0, part: part::CHANGED }]);
    assert_eq!(writes, [Cell { kind: cell::PIN, index: event, part: part::CHANGED }]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_param_previous_reads_its_previous_part() {
    // `on tol.changed() { seen <- tol.prev(); }`: the reaction reads the `PREVIOUS` of the pin.
    let ir = ParamEvents::ir(ValueType::F64, |b| b.expr(Expr::ParamPrevious(0), ValueType::F64, ParamEvents::at()));
    let plan = Switching::plan(&ir);
    let shape = plan.shape().shape();
    assert_eq!(plan.reactions()[0].reads, [Source::ParamPrevious(0)]);
    let reaction = shape.reactions.iter().find(|r| r.name.as_str() == "reaction:0").expect("the reaction");
    let reads: Vec<Cell> = reaction.reads.iter().copied().collect();
    assert_eq!(reads, [Cell { kind: cell::PIN, index: 0, part: part::PREVIOUS }]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_reached_carrier_is_a_port_pin() {
    // `Top` reads the `signal q` of its child `leaf` as `seen` (a reached carrier, read only like
    // an `in` port): a `PORT` discrete pin the netlist binds to the child's net.
    let at = |line| Origin { file: "reach.msl".into(), line, col: 1 };
    let mut b = IrBuilder::new();
    let spec = |name: &str, children| BodySpec { name: name.into(), origin: at(1), params: vec![], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children };
    let leaf = b.body(spec("Leaf", vec![]));
    let leaf_q = b.carrier(leaf, Carrier { name: "q".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: at(2) });
    let top_body = b.body(spec("Top", vec![Child { name: "leaf".into(), body: leaf, nodes: vec![], params: vec![], origin: at(5) }]));
    let seen = b.carrier(top_body, Carrier { name: "seen".into(), ty: ValueType::Bool, role: CarrierRole::Reached, init: None, origin: at(4) });
    b.reach(top_body, Reach { child: 0, item: ReachedItem::Carrier { carrier: seen, child_carrier: leaf_q } });
    let top = b.next_instance_id();
    let top_net = b.net(NetSpec { path: "Top.seen".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: at(4) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: at(4) });
    b.bind_nets(top, vec![top_net]);
    let leaf_id = b.next_instance_id();
    let q = b.net(NetSpec { path: "Top.leaf.q".into(), ty: ValueType::Bool, declared_by: leaf_id, init: None, origin: at(2) });
    let leaf_instance = b.instance(InstanceSpec { path: "Top.leaf".into(), name: "leaf".into(), body: leaf, parent: Some(top), nodes: vec![], params: vec![], origin: at(5) });
    b.bind_nets(leaf_instance, vec![q]);
    b.bind_reached_net(top, seen, q);
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "{ir}");
    let top_body = ir.top().body();
    let plan = BodyPlan::new(top_body);
    let shape = plan.shape().shape();
    let pin = shape.pins.get(Find::pin(shape, "seen") as usize).expect("the pin");
    assert_eq!((pin.kind, pin.role), (kind::DISCRETE, role::PORT));
    assert!(BodyPlan::carrier_is_port(top_body, seen));
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}
