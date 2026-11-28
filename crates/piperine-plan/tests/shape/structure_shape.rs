//! The shape of each `analog-structure` form: switch branches and conditional contributions
//! (AS-01, AS-03), derivative sites and their `abstol` pins (AS-07), guarded `limit` and
//! `bound_step` sites, param events and reached carriers (AS-06, AS-14, AS-11). Each test builds
//! a small body by hand and reads its shape entry by entry; every expected entry is written from
//! the design (mechanisms 1 to 3) and abi.md, and every shape passes `piperine_abi::validate`. A body `check()` refuses is a
//! plan error, not a panic.

use piperine_abi::raw::{Cell, DEFAULT, GROUND, Shape, cell, kind, op, part, role};
use piperine_ir::{
    BodySpec, BranchKind, Carrier, CarrierRole, Child, Expr, InstanceSpec, Ir, IrBuilder, NetSpec, Operator, Origin, QuantityRole, Reach, ReachedItem, Site, Stmt,
    ValueType,
};
use piperine_plan::{BodyPlan, Mode, Source, Target, Term};

use piperine_ir_fixtures::structure;

use structure::{ParamEvents, Switching, SwitchingWrites};

/// Entries of a shape, found by name.
struct Find;

impl Find {
    /// The plan of the only body of `ir`.
    fn plan(ir: &Ir) -> BodyPlan {
        BodyPlan::build(ir.bodies().next().expect("a body")).expect("the body plans")
    }

    /// The cell of a pin's value.
    fn value(pin: u32) -> Cell {
        Cell { kind: cell::PIN, index: pin, part: part::VALUE }
    }

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

#[test]
fn a_switch_branch_has_a_flow_pin_and_an_equation_term() {
    // Design mechanism 2: a switch branch has the shape of a potential branch: the private flow
    // pin `b0.flow`, the term `b0.flow` = FLOW {a, gnd}, and the term `b0.eq` = FLOW {b0.flow, gnd}.
    let ir = Switching::ir(BranchKind::Switch, Switching::on_the_carrier);
    let plan = Find::plan(&ir);
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
        SwitchingWrites { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
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
    let switch = Switching::ir(BranchKind::Switch, Switching::on_the_carrier);
    assert_eq!(Find::terms_plan(&Find::plan(&switch)).affine, [true, false], "b0.flow is linear in the potential, b0.eq is not");
    let potential = Switching::ir(BranchKind::Potential, |b| SwitchingWrites { analog: vec![Switching::contribute(b, QuantityRole::Potential, 1.0)], ..SwitchingWrites::default() });
    assert_eq!(Find::terms_plan(&Find::plan(&potential)).affine, [true, true]);
}

#[test]
fn a_body_without_new_forms_keeps_its_shape() {
    // A potential branch with a `ddt` addend: the pins and the terms are those of abi.md A1, in
    // the order they had before this feature (`b0.flow`, `b0.eq`, `b0.charge`).
    let ir = Switching::ir(BranchKind::Potential, |b| {
        let (potential, charge) = (Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::probe(b));
        let charge = Switching::contribute_expr(b, QuantityRole::Potential, Operator::Ddt, charge);
        SwitchingWrites { analog: vec![potential, charge], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
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

#[test]
fn a_derivative_site_has_a_rate_pin_and_two_terms() {
    // `v(a, gnd) +> 2.0 * ddt(v(a, gnd))` on a potential branch (design mechanism 3): the pin
    // `d0.rate` follows the branch flow pin, and its two terms follow the branch terms.
    let ir = Switching::ir(BranchKind::Potential, |b| {
        let site = Switching::site(b, None);
        SwitchingWrites { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Potential, 0)], derivatives: vec![site], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
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
        SwitchingWrites { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0)], derivatives: vec![site], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
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
    assert_eq!((reads, writes, partials), (vec![Find::value(0)], vec![Find::value(abstol)], vec![]), "it reads the param `tol` alone and has no partial");
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
        SwitchingWrites { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0)], derivatives: vec![site], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    assert!(Source::Derivative(0).is_differentiable() && Source::Derivative(0).is_pin());
    let terms = Find::terms_plan(&plan);
    assert_eq!(terms.reads, [Source::Node(0), Source::Derivative(0)]);
    assert_eq!(terms.slots, 2);
    // Writes: b0.flow, d0.rate, d0.state. Partials as (write, slot): the flow reads the rate, the
    // rate term is the rate itself, the state term reads the node the site differentiates.
    assert_eq!(terms.partials, [(0, 1), (1, 1), (2, 0)]);
    let (reads, _, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [Find::value(Find::pin(shape, "a")), Find::value(Find::pin(shape, "d0.rate"))]);
    assert_eq!(partials, [(0, 1), (1, 1), (2, 0)]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_guarded_limit_reads_its_enabled_sources() {
    // `if en.high() { i(a, gnd) +> limit(v(a, gnd), ..); }` with `enabled = en.high()`: the
    // function `limit:lim` reads the carrier `en` its `enabled` reads, after what the site reads
    // itself and before its own `LAST`, `AGE` and the ambient `STATIC`.
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (read, cond, enabled) = (Switching::limit_read(b), Switching::carrier_high(b), Switching::carrier_high(b));
        let taken = Switching::contribute_expr(b, QuantityRole::Flow, Operator::Direct, read);
        let site = Site { name: "lim".into(), origin: Switching::at(), enabled: Some(enabled) };
        SwitchingWrites { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], limits: vec![site], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Limit(0)).expect("the limit function");
    assert_eq!(function.reads, [Source::Node(0), Source::Carrier(0), Source::LimitLast(0), Source::LimitAge(0), Source::Static]);
    let (reads, _, _) = Find::function(shape, "limit:lim");
    assert_eq!(reads[1], Find::value(Find::pin(shape, "en")), "the carrier's pin value");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_guarded_cap_reads_its_when_sources() {
    // `bound_step(1n) when en.high();`: the `CAP` watch's function `cap:0` reads the carrier.
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(1e-9), Switching::carrier_high(b));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        SwitchingWrites { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..SwitchingWrites::default() }
    });
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Cap(0)).expect("the cap function");
    assert_eq!((function.reads.clone(), function.writes.clone()), (vec![Source::Carrier(0)], vec![Target::Cap(0)]));
    let (reads, _, _) = Find::function(shape, "cap:0");
    assert_eq!(reads, [Find::value(Find::pin(shape, "en"))]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_param_changed_event_reads_the_params_changed_byte() {
    // `on tol.changed() { seen <- fired(E0); }`: the event function `event:0` reads the param's
    // value and its `CHANGED` byte (abi.md §4.1) and writes the event pin's `CHANGED`.
    let ir = ParamEvents::ir(ValueType::Bool, |b| b.expr(Expr::Fired(0), ValueType::Bool, ParamEvents::at()));
    let plan = Find::plan(&ir);
    let shape = plan.shape().shape();
    let function = plan.functions().iter().find(|f| f.mode == Mode::Event(0)).expect("the event function");
    assert_eq!((function.reads.clone(), function.writes.clone()), (vec![Source::Param(0), Source::ParamChanged(0)], vec![Target::EventPin(0)]));
    let (reads, writes, _) = Find::function(shape, "event:0");
    let event = Find::pin(shape, "event:0");
    assert_eq!(reads, [Find::value(0), Cell { kind: cell::PIN, index: 0, part: part::CHANGED }]);
    assert_eq!(writes, [Cell { kind: cell::PIN, index: event, part: part::CHANGED }]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_param_previous_reads_its_previous_part() {
    // `on tol.changed() { seen <- tol.prev(); }`: the reaction reads the `PREVIOUS` of the pin.
    let ir = ParamEvents::ir(ValueType::F64, |b| b.expr(Expr::ParamPrevious(0), ValueType::F64, ParamEvents::at()));
    let plan = Find::plan(&ir);
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
    let leaf_q = b.carrier(leaf, Carrier { name: "q".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: at(2), count: None });
    let top_body = b.body(spec("Top", vec![Child { name: "leaf".into(), body: leaf, nodes: vec![], params: vec![], origin: at(5) }]));
    let seen = b.carrier(top_body, Carrier { name: "seen".into(), ty: ValueType::Bool, role: CarrierRole::Reached, init: None, origin: at(4), count: None });
    b.reach(top_body, Reach { child: 0, item: ReachedItem::Carrier { carrier: seen, child_carrier: leaf_q } });
    let top = b.next_instance_id();
    let top_net = b.net(NetSpec { path: "Top.seen".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: at(4) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: at(4) });
    b.bind_nets(top, vec![top_net]).unwrap();
    let leaf_id = b.next_instance_id();
    let q = b.net(NetSpec { path: "Top.leaf.q".into(), ty: ValueType::Bool, declared_by: leaf_id, init: None, origin: at(2) });
    let leaf_instance = b.instance(InstanceSpec { path: "Top.leaf".into(), name: "leaf".into(), body: leaf, parent: Some(top), nodes: vec![], params: vec![], origin: at(5) });
    b.bind_nets(leaf_instance, vec![q]).unwrap();
    b.bind_reached_net(top, seen, q).unwrap();
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "{ir}");
    let top_body = ir.top().body();
    let plan = BodyPlan::build(top_body).expect("the body plans");
    let shape = plan.shape().shape();
    let pin = shape.pins.get(Find::pin(shape, "seen") as usize).expect("the pin");
    assert_eq!((pin.kind, pin.role), (kind::DISCRETE, role::PORT));
    assert!(BodyPlan::carrier_is_port(top_body, seen));
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

/// A body that `Ir::check` refuses, planned without being checked: the plan's error names the body, where the plan used to panic.
mod unchecked_body {
    use piperine_ir::{BodySpec, Branch, BranchKind, Constant, InstanceSpec, Ir, IrBuilder, Origin, Terminal};
    use piperine_plan::BodyPlan;

    /// A body whose one branch ends on an own terminal, which belongs in a child's node slot only.
    struct OwnTerminalBody;

    impl OwnTerminalBody {
        /// The IR of `Top`, which `check()` refuses.
        fn ir() -> Ir {
            let origin = Origin { file: "unchecked.msl".into(), line: 1, col: 1 };
            let mut builder = IrBuilder::new();
            let body = builder.body(BodySpec {
                name: "Top".into(),
                origin: origin.clone(),
                params: vec![],
                nodes: vec![],
                branches: vec![Branch { plus: Terminal::Own(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
                limits: vec![],
                stims: vec![],
                locals: vec![],
                analog: None,
                children: vec![],
            });
            let top = builder.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: Vec::<Constant>::new(), origin });
            builder.finish(top)
        }
    }

    #[test]
    fn an_own_terminal_on_a_branch_is_a_plan_error_not_a_panic() {
        let ir = OwnTerminalBody::ir();
        assert!(!ir.check().is_empty(), "the body is one check() refuses");
        let refused = BodyPlan::build(ir.bodies().next().expect("a body")).expect_err("an own terminal has no pin");
        assert_eq!(refused.body, "Top");
        assert!(refused.to_string().contains("own terminal"), "{refused}");
    }
}
