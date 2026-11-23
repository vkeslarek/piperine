//! Scope: the IR forms of `analog-structure`, built by hand through `IrBuilder`, checked,
//! printed and round-tripped through serde (the printer is one way, `tests/print.rs`): the own
//! terminal of a child's own signal (AS-16).

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::{Devices, Ex};
use piperine_ir::{
    Addend, BinaryOp, BodyId, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Child, Condition, Constant, Contribution, Derivative, Drive, EventSource, Expr, ExprId, FunctionSpec, Hoisted, Inflow, Merge, Reach, ReachedItem,
    InstanceSpec, Ir, IrBuilder, NetSpec, NodeRef, NodeRole, NodeSpec, Operator, Origin, Param, QuantityRole, Event, Reaction, Real, Site, Stmt, StmtId, Target, Terminal,
    ValueType,
};

/// The small designs every test of this target builds.
struct Designs;

impl Designs {
    const FILE: &'static str = "structure.msl";

    fn origin(line: u32) -> Origin {
        Origin { file: Self::FILE.into(), line, col: 1 }
    }

    fn node(name: &str, role: NodeRole, nature: piperine_ir::NatureId) -> BodyNode {
        BodyNode { name: name.into(), nature, role, init: None, origin: Self::origin(1) }
    }

    /// `Leaf(p)` with an own `signal s`, placed once by `Top` as `Child { nodes: slots }`:
    /// `slots` is what the parent's authored record binds at the leaf's two slots.
    fn leaf_in_top(slots: Vec<Terminal>) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let leaf = b.body(BodySpec {
            name: "Leaf".into(),
            origin: Self::origin(1),
            params: vec![],
            nodes: vec![Self::node("p", NodeRole::Port, el), Self::node("s", NodeRole::Signal, el)],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let child = Child { name: "leaf".into(), body: leaf, nodes: slots, params: vec![], origin: Self::origin(5) };
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: Self::origin(4),
            params: vec![],
            nodes: vec![Self::node("a", NodeRole::Signal, el)],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![child],
        });
        let top = b.next_instance_id();
        let a = b.node(NodeSpec { path: "Top.a".into(), nature: el, declared_by: top, init: None, origin: Self::origin(4) });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![NodeRef::Node(a)], params: vec![], origin: Self::origin(4) });
        let own = b.node(NodeSpec { path: "Top.leaf.s".into(), nature: el, declared_by: b.next_instance_id(), init: None, origin: Self::origin(1) });
        b.instance(InstanceSpec { path: "Top.leaf".into(), name: "leaf".into(), body: leaf, parent: Some(top), nodes: vec![NodeRef::Node(a), NodeRef::Node(own)], params: vec![], origin: Self::origin(5) });
        b.finish(top)
    }

    /// `Top` with a branch whose end is the own terminal `own0`.
    fn own_in_a_branch() -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let body = b.body(BodySpec {
            name: "Lone".into(),
            origin: Self::origin(1),
            params: vec![],
            nodes: vec![Self::node("a", NodeRole::Signal, el)],
            branches: vec![Branch { plus: Terminal::Own(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.next_instance_id();
        let a = b.node(NodeSpec { path: "Lone.a".into(), nature: el, declared_by: top, init: None, origin: Self::origin(1) });
        let top = b.instance(InstanceSpec { path: "Lone".into(), name: "Lone".into(), body, parent: None, nodes: vec![NodeRef::Node(a)], params: vec![], origin: Self::origin(1) });
        b.finish(top)
    }
}

#[test]
fn an_own_terminal_prints_and_round_trips() {
    let ir = Designs::leaf_in_top(vec![Terminal::Node(0), Terminal::Own(1)]);
    assert_eq!(ir.check(), vec![], "a child that names its own signal passes the check");
    // The authored tree shows the child's own node by name, never as ground (AS-16 AC 1).
    let text = ir.to_string();
    assert!(text.contains("own:s"), "{text}");
    assert!(!text.contains("gnd"), "{text}");
    // The text of the arenas shows the slot too, and the IR survives serde unchanged (AC 2).
    assert!(ir.print_structural().contains("own:s"));
    let back: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serialize")).expect("deserialize");
    assert_eq!(back, ir);
}

#[test]
fn an_own_terminal_outside_a_child_is_broken() {
    let broken = Designs::own_in_a_branch().check();
    assert!(broken.iter().any(|b| b.what.contains("own0") && b.what.contains("child's node slot")), "{broken:?}");
}

#[test]
fn an_own_terminal_that_names_a_port_is_broken() {
    // Slot 0 of `Leaf` is its port `p`: the parent binds it, the child does not own it.
    let broken = Designs::leaf_in_top(vec![Terminal::Own(0), Terminal::Own(1)]).check();
    assert!(broken.iter().any(|b| b.what.contains("is a port") && b.what.contains("own0")), "{broken:?}");
    // An own terminal must also name its own slot.
    let misplaced = Designs::leaf_in_top(vec![Terminal::Node(0), Terminal::Own(0)]).check();
    assert!(misplaced.iter().any(|b| b.what.contains("names its own slot")), "{misplaced:?}");
}

/// What a test writes into the body `Switching` builds.
#[derive(Default)]
struct Written {
    analog: Vec<StmtId>,
    derivatives: Vec<Derivative>,
    limits: Vec<Site>,
    stims: Vec<Site>,
}

/// One body `Sw(a)` with the branch `(a, gnd)` of a given kind, a `Bool` carrier `en` when asked,
/// and an `analog` block the test writes; placed alone as the design.
struct Switching;

impl Switching {
    const FILE: &'static str = "switching.msl";

    /// The expression helper at a fixed position.
    fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 3, 5)
    }

    /// `i(a, gnd) +> value;` or `v(a, gnd) +> value;`.
    fn contribute(b: &mut IrBuilder, quantity: QuantityRole, value: f64) -> StmtId {
        let value = Self::ex(b).real(value);
        b.stmt(Stmt::Contribute { branch: 0, quantity, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] })
    }

    /// A block of statements with no value.
    fn arm(b: &mut IrBuilder, stmts: Vec<StmtId>) -> ExprId {
        Self::ex(b).block(stmts, None, ValueType::Unit)
    }

    /// `if cond { then } else { otherwise }` as a statement.
    fn branch_on(b: &mut IrBuilder, cond: ExprId, then: Vec<StmtId>, otherwise: Vec<StmtId>) -> StmtId {
        let (then, otherwise) = (Self::arm(b, then), Self::arm(b, otherwise));
        let o = Origin { file: Self::FILE.into(), line: 3, col: 5 };
        let expr = b.expr(Expr::If { cond, then, otherwise }, ValueType::Unit, o);
        b.stmt(Stmt::Expr(expr))
    }

    /// `v(a) > 0.5`.
    fn probe_above(b: &mut IrBuilder) -> ExprId {
        let mut e = Self::ex(b);
        let probe = e.probe(Terminal::Node(0), Terminal::Ground);
        let half = e.real(0.5);
        e.binary(BinaryOp::Gt, probe, half)
    }

    /// A read of the `bool` carrier `en`.
    fn carrier_high(b: &mut IrBuilder) -> ExprId {
        let o = Origin { file: Self::FILE.into(), line: 3, col: 5 };
        b.expr(Expr::Carrier(0), ValueType::Bool, o)
    }

    /// The design: `kind` is the kind of branch 0; `write` returns the `analog` block (and may add
    /// a function or a reaction through the builder).
    fn ir(kind: BranchKind, write: impl FnOnce(&mut IrBuilder) -> Vec<StmtId>) -> Ir {
        Self::ir_with_derivatives(kind, |b| (write(b), vec![]))
    }

    /// `ir`, with the derivative sites `write` returns beside the `analog` block. The body has one
    /// param `tol: f64` (index 0).
    fn ir_with_derivatives(kind: BranchKind, write: impl FnOnce(&mut IrBuilder) -> (Vec<StmtId>, Vec<Derivative>)) -> Ir {
        Self::ir_with_sites(kind, |b| {
            let (analog, derivatives) = write(b);
            Written { analog, derivatives, ..Written::default() }
        })
    }

    /// `ir`, with everything `write` returns: the `analog` block, the derivative sites and the
    /// `limit` and `ac_stim` sites.
    fn ir_with_sites(kind: BranchKind, write: impl FnOnce(&mut IrBuilder) -> Written) -> Ir {
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
        b.carrier(body, Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: origin.clone(), count: None });
        for site in derivatives {
            b.derivative(body, site);
        }
        let top = b.next_instance_id();
        let net = b.net(NetSpec { path: "Sw.en".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: origin.clone() });
        let params = vec![Constant::Real(Real::new(1e-9))];
        let top = b.instance(InstanceSpec { path: "Sw".into(), name: "Sw".into(), body, parent: None, nodes: vec![NodeRef::Ground], params, origin });
        b.bind_nets(top, vec![net]);
        b.finish(top)
    }

    /// `i(a, gnd) +> 2.0 * rate(D<k>);`
    fn contribute_twice_the_rate(b: &mut IrBuilder, k: u32) -> StmtId {
        let origin = Origin { file: Self::FILE.into(), line: 3, col: 5 };
        let rate = b.expr(Expr::Derivative(k), ValueType::F64, origin);
        let mut e = Self::ex(b);
        let two = e.real(2.0);
        let value = e.binary(BinaryOp::Mul, two, rate);
        b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] })
    }

    /// The site `ddt(<state>, <abstol>)` named `q`.
    fn site(state: ExprId, abstol: Option<ExprId>) -> Derivative {
        Derivative { name: "q".into(), state, abstol, origin: Origin { file: Self::FILE.into(), line: 3, col: 5 } }
    }

    /// Whether any entry of `broken` says `what`.
    fn names(broken: &[piperine_ir::Broken], what: &str) -> bool {
        broken.iter().any(|entry| entry.what.contains(what))
    }
}

#[test]
fn a_contribution_in_an_if_arm_passes_check() {
    // `if v(a) > 0.5 { i(a, gnd) +> 1.0; } else { { i(a, gnd) +> 2.0; } }`: one under the `if`, one in
    // a plain block nested in its other arm (AS-01 AC 1, AC 3).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let nested = Switching::contribute(b, QuantityRole::Flow, 2.0);
        let plain = Switching::arm(b, vec![nested]);
        let plain = b.stmt(Stmt::Expr(plain));
        let (cond, taken) = (Switching::probe_above(b), Switching::contribute(b, QuantityRole::Flow, 1.0));
        vec![Switching::branch_on(b, cond, vec![taken], vec![plain])]
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
}

#[test]
fn a_contribution_in_a_function_or_a_reaction_is_broken() {
    // A function whose body block contributes.
    let in_function = Switching::ir(BranchKind::Flow, |b| {
        let inside = Switching::contribute(b, QuantityRole::Flow, 1.0);
        let body = Switching::arm(b, vec![inside]);
        b.function(FunctionSpec { name: "stamp".into(), origin: Origin { file: Switching::FILE.into(), line: 9, col: 1 }, params: vec![], ret: ValueType::Unit, locals: vec![], body });
        vec![Switching::contribute(b, QuantityRole::Flow, 1.0)]
    });
    assert!(Switching::names(&in_function.check(), "contribution outside the analog block of a body"), "{:?}", in_function.check());

    // A closure inside the analog block: the closure body block contributes.
    let in_closure = Switching::ir(BranchKind::Flow, |b| {
        let inside = Switching::contribute(b, QuantityRole::Flow, 1.0);
        let body = Switching::arm(b, vec![inside]);
        let o = Origin { file: Switching::FILE.into(), line: 3, col: 5 };
        let closure = b.expr(Expr::Closure { params: vec![], body }, ValueType::Fn(vec![], Box::new(ValueType::Unit)), o);
        vec![b.stmt(Stmt::Expr(closure))]
    });
    assert!(Switching::names(&in_closure.check(), "contribution inside a closure"), "{:?}", in_closure.check());

    // A reaction (`on en.changed()`): contributions belong to the analog block.
    let mut b = IrBuilder::new();
    let el = Devices::electrical(&mut b);
    let origin = Origin { file: Switching::FILE.into(), line: 1, col: 1 };
    let inside = Switching::contribute(&mut b, QuantityRole::Flow, 1.0);
    let body = b.body(BodySpec {
        name: "Sw".into(),
        origin: origin.clone(),
        params: vec![],
        nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: vec![],
    });
    b.carrier(body, Carrier { name: "en".into(), ty: ValueType::Bool, role: CarrierRole::In, init: None, origin: origin.clone(), count: None });
    let event = b.event(body, piperine_ir::Event { source: piperine_ir::EventSource::Changed(0), guard: None, origin: origin.clone() });
    b.reaction(body, Reaction { on: vec![event], stmts: vec![inside], locals: vec![], origin: origin.clone() });
    let top = b.next_instance_id();
    let net = b.net(NetSpec { path: "Sw.en".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: origin.clone() });
    let top = b.instance(InstanceSpec { path: "Sw".into(), name: "Sw".into(), body, parent: None, nodes: vec![NodeRef::Ground], params: vec![], origin });
    b.bind_nets(top, vec![net]);
    let in_reaction = b.finish(top);
    assert!(Switching::names(&in_reaction.check(), "contribution outside the analog block of a body"), "{:?}", in_reaction.check());
}

#[test]
fn a_switch_branch_takes_both_quantities() {
    // `if v(a) > 0.5 { v(a, gnd) +> 1.0; } else { i(a, gnd) +> 2.0; }` on one branch (§11 rule 2).
    let ir = Switching::ir(BranchKind::Switch, |b| {
        let (cond, potential, flow) = (Switching::probe_above(b), Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::contribute(b, QuantityRole::Flow, 2.0));
        vec![Switching::branch_on(b, cond, vec![potential], vec![flow])]
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    assert!(ir.to_string().contains("switch"), "the printed branch names its kind");
}

#[test]
fn a_flow_branch_refuses_a_potential_contribution() {
    let flow_with_potential = Switching::ir(BranchKind::Flow, |b| vec![Switching::contribute(b, QuantityRole::Potential, 1.0)]);
    assert!(Switching::names(&flow_with_potential.check(), "does not match the kind of branch 0"), "{:?}", flow_with_potential.check());
    let potential_with_flow = Switching::ir(BranchKind::Potential, |b| vec![Switching::contribute(b, QuantityRole::Flow, 1.0)]);
    assert!(Switching::names(&potential_with_flow.check(), "does not match the kind of branch 0"), "{:?}", potential_with_flow.check());
}

#[test]
fn contributions_lists_each_with_its_enclosing_conditions() {
    // c1 = `v(a) > 0.5`, c2 = `en`:
    //   `if c1 { A } else { if c2 { B } }  { C }  D`
    let mut conditions: Vec<ExprId> = Vec::new();
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (c1, c2) = (Switching::probe_above(b), Switching::carrier_high(b));
        conditions = vec![c1, c2];
        let (a, bb, c, d) = (
            Switching::contribute(b, QuantityRole::Flow, 1.0),
            Switching::contribute(b, QuantityRole::Flow, 2.0),
            Switching::contribute(b, QuantityRole::Flow, 3.0),
            Switching::contribute(b, QuantityRole::Flow, 4.0),
        );
        let inner = Switching::branch_on(b, c2, vec![bb], vec![]);
        let outer = Switching::branch_on(b, c1, vec![a], vec![inner]);
        let plain = Switching::arm(b, vec![c]);
        let plain = b.stmt(Stmt::Expr(plain));
        vec![outer, plain, d]
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let listed: Vec<Contribution<'_>> = ir.bodies().next().expect("a body").contributions();
    let (c1, c2) = (conditions[0], conditions[1]);
    let under: Vec<Vec<Condition>> = listed.iter().map(|c| c.conditions.clone()).collect();
    // Source order A, B, C, D: A under c1 holding, B under c1 not holding then c2 holding, C and D free.
    assert_eq!(
        under,
        vec![vec![Condition { expr: c1, holds: true }], vec![Condition { expr: c1, holds: false }, Condition { expr: c2, holds: true }], vec![], vec![]]
    );
    assert!(listed.iter().all(|c| c.branch == 0 && c.quantity == QuantityRole::Flow));
}

#[test]
fn a_nested_contribution_reads_its_conditions_in_its_branch_dependencies() {
    // `if v(a) > 0.5 { i(a, gnd) +> 1.0; }`: the branch reads the node its condition probes, and
    // is no longer affine; `if en { i(a, gnd) +> 1.0; }` reads the carrier and stays affine.
    let on_probe = Switching::ir(BranchKind::Flow, |b| {
        let (cond, taken) = (Switching::probe_above(b), Switching::contribute(b, QuantityRole::Flow, 1.0));
        vec![Switching::branch_on(b, cond, vec![taken], vec![])]
    });
    let body = on_probe.bodies().next().expect("a body");
    assert_eq!(body.dependencies(Target::Flow(0)).nodes, vec![0]);
    assert!(!body.is_affine(0));

    let on_carrier = Switching::ir(BranchKind::Flow, |b| {
        let (cond, taken) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Flow, 1.0));
        vec![Switching::branch_on(b, cond, vec![taken], vec![])]
    });
    let body = on_carrier.bodies().next().expect("a body");
    assert_eq!(body.carrier_reads(Target::Flow(0)), vec![0]);
    assert!(body.dependencies(Target::Flow(0)).nodes.is_empty());
    assert!(body.is_affine(0));
}

#[test]
fn a_derivative_site_prints_and_round_trips() {
    // `i(a, gnd) +> 2.0 * ddt(v(a), tol)`: the `ddt` is off a whole addend, so a site D0.
    let ir = Switching::ir_with_derivatives(BranchKind::Flow, |b| {
        let state = Switching::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let abstol = b.expr(Expr::Param(0), ValueType::F64, Origin { file: Switching::FILE.into(), line: 3, col: 5 });
        (vec![Switching::contribute_twice_the_rate(b, 0)], vec![Switching::site(state, Some(abstol))])
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    assert!(text.contains("derivative D0 \"q\" state probe(a, gnd) abstol tol"), "{text}");
    assert!(text.contains("b0.flow +> direct (2.0 * rate(D0))"), "{text}");
    assert_eq!(ir.bodies().next().expect("a body").derivatives().len(), 1);
    let back: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serialize")).expect("deserialize");
    assert_eq!(back, ir);
}

#[test]
fn a_derivative_read_out_of_range_is_broken() {
    // `rate(D1)` with one site, and `rate(D0)` in a body with none.
    let one_site = Switching::ir_with_derivatives(BranchKind::Flow, |b| {
        let state = Switching::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        (vec![Switching::contribute_twice_the_rate(b, 1)], vec![Switching::site(state, None)])
    });
    assert!(Switching::names(&one_site.check(), "derivative read of site 1, out of range (1 sites)"), "{:?}", one_site.check());
    let none = Switching::ir(BranchKind::Flow, |b| vec![Switching::contribute_twice_the_rate(b, 0)]);
    assert!(Switching::names(&none.check(), "derivative read of site 0, out of range (0 sites)"), "{:?}", none.check());
}

#[test]
fn a_derivative_abstol_that_reads_a_probe_is_broken() {
    // `ddt(v(a), v(a))`: a tolerance is a constant or a param expression.
    let ir = Switching::ir_with_derivatives(BranchKind::Flow, |b| {
        let state = Switching::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let abstol = Switching::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        (vec![Switching::contribute_twice_the_rate(b, 0)], vec![Switching::site(state, Some(abstol))])
    });
    assert!(Switching::names(&ir.check(), "the `abstol` of a derivative site reads an input"), "{:?}", ir.check());
}

#[test]
fn a_derivative_state_reads_are_its_dependencies() {
    // D0: `ddt(v(a), tol)`: the state reads node 0, the tolerance reads param 0, and
    // the contribution that reads `rate(D0)` lists the site.
    let ir = Switching::ir_with_derivatives(BranchKind::Flow, |b| {
        let state = Switching::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let abstol = b.expr(Expr::Param(0), ValueType::F64, Origin { file: Switching::FILE.into(), line: 3, col: 5 });
        (vec![Switching::contribute_twice_the_rate(b, 0)], vec![Switching::site(state, Some(abstol))])
    });
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.dependencies(Target::Derivative(0)).nodes, vec![0]);
    assert!(body.dependencies(Target::Derivative(0)).params.is_empty());
    assert_eq!(body.dependencies(Target::DerivativeAbstol(0)).params, vec![0]);
    assert!(body.dependencies(Target::DerivativeAbstol(0)).nodes.is_empty());
    assert_eq!(body.derivative_reads(Target::Flow(0)), vec![0]);
    // The rate is an unknown like a flow: it reaches the sum affinely.
    assert!(body.is_affine(0));
}

/// Guarded `limit`, `ac_stim` and `bound_step` sites.
impl Switching {
    fn at() -> Origin {
        Origin { file: Self::FILE.into(), line: 3, col: 5 }
    }

    fn named(name: &str, enabled: Option<ExprId>) -> Site {
        Site { name: name.into(), origin: Self::at(), enabled }
    }

    /// `limit(v(a), |..| 0.0)` of site 0.
    fn limit_read(b: &mut IrBuilder) -> ExprId {
        let x = Self::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let function = Self::ex(b).real(0.0);
        let closure = b.expr(Expr::Closure { params: vec![], body: function }, ValueType::Fn(vec![], Box::new(ValueType::F64)), Self::at());
        b.expr(Expr::Limit { site: 0, x, f: closure, seed: None }, ValueType::F64, Self::at())
    }

    /// `ac_stim(1.0)` of site 0.
    fn stim_read(b: &mut IrBuilder) -> ExprId {
        let magnitude = Self::ex(b).real(1.0);
        b.expr(Expr::Stim { site: 0, mag: magnitude, phase: None }, ValueType::F64, Self::at())
    }

    /// `i(a, gnd) +> <value>;`
    fn contribute_expr(b: &mut IrBuilder, value: ExprId) -> StmtId {
        b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] })
    }

    /// `if en { i(a, gnd) +> <read>; }` where `read` builds the contributed value.
    fn read_under_a_condition(b: &mut IrBuilder, read: ExprId) -> StmtId {
        let (cond, taken) = (Self::carrier_high(b), Self::contribute_expr(b, read));
        Self::branch_on(b, cond, vec![taken], vec![])
    }
}

#[test]
fn a_limit_under_a_condition_needs_enabled() {
    // `if en { i(a, gnd) +> limit(..); }`: without `enabled` the site is broken, with
    // `enabled = en` the same body passes and the site reads the carrier.
    let unguarded = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let read = Switching::limit_read(b);
        Written { analog: vec![Switching::read_under_a_condition(b, read)], limits: vec![Switching::named("lim", None)], ..Written::default() }
    });
    assert!(Switching::names(&unguarded.check(), "limit site 0 under an `if` or a short-circuit operator has no `enabled`"), "{:?}", unguarded.check());

    let guarded = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let (read, enabled) = (Switching::limit_read(b), Switching::carrier_high(b));
        Written { analog: vec![Switching::read_under_a_condition(b, read)], limits: vec![Switching::named("lim", Some(enabled))], ..Written::default() }
    });
    assert_eq!(guarded.check(), vec![], "{}", guarded);
    let body = guarded.bodies().next().expect("a body");
    assert_eq!(body.carrier_reads(Target::Limit(0)), vec![0], "the site reads what enables it");
    assert!(guarded.to_string().contains("limit L0 \"lim\" enabled en"), "{}", guarded);
}

#[test]
fn a_top_level_limit_has_no_enabled() {
    // `i(a, gnd) +> limit(..)` at the top of the block, on a site that claims an `enabled`.
    let ir = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let (read, enabled) = (Switching::limit_read(b), Switching::carrier_high(b));
        Written { analog: vec![Switching::contribute_expr(b, read)], limits: vec![Switching::named("lim", Some(enabled))], ..Written::default() }
    });
    assert!(Switching::names(&ir.check(), "limit site 0 has an `enabled` but is read outside every condition"), "{:?}", ir.check());
}

#[test]
fn an_ac_stim_under_a_condition_passes_check() {
    // `if en { i(a, gnd) +> ac_stim(1.0); }` (AS-06 AC 4): no `enabled` is needed.
    let ir = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let read = Switching::stim_read(b);
        Written { analog: vec![Switching::read_under_a_condition(b, read)], stims: vec![Switching::named("ac", None)], ..Written::default() }
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
}

#[test]
fn a_bound_step_with_when_reads_its_condition() {
    // `bound_step(1n) when en` at the top of the block: the cap site reads the carrier.
    let capped = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(1e-9), Switching::carrier_high(b));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    assert_eq!(capped.check(), vec![], "{}", capped);
    let body = capped.bodies().next().expect("a body");
    assert_eq!(body.carrier_reads(Target::BoundStep(0)), vec![0]);
    assert!(capped.to_string().contains("bound_step(1e-9) when en"), "{}", capped);

    // A `when` that is not a `bool` is broken.
    let wrong = Switching::ir_with_sites(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(1e-9), Switching::ex(b).real(1.0));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    assert!(Switching::names(&wrong.check(), "the `when` of a `bound_step`"), "{:?}", wrong.check());
}

/// Designs that place children, to merge their nodes or reach into them.
struct Placements;

impl Placements {
    fn at(line: u32) -> Origin {
        Origin { file: "placements.msl".into(), line, col: 1 }
    }

    fn node(name: &str, role: NodeRole, nature: piperine_ir::NatureId) -> BodyNode {
        BodyNode { name: name.into(), nature, role, init: None, origin: Self::at(1) }
    }

    fn body(name: &str, nodes: Vec<BodyNode>, children: Vec<Child>) -> BodySpec {
        BodySpec { name: name.into(), origin: Self::at(1), params: vec![], nodes, branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children }
    }

    fn child(name: &str, body: BodyId, nodes: Vec<Terminal>) -> Child {
        Child { name: name.into(), body, nodes, params: vec![], origin: Self::at(5) }
    }

    /// `Lone(p, n)` whose branch `(p, n)` was dropped as a merge, placed by `Top` as `p1(x, y)` and
    /// `p2(y, z)`. `merge` says which `merge_nodes` calls placement makes: it receives the builder
    /// and the node ids of `x`, `y`, `z`. The two `Lone` instances are the last two instances.
    fn lones(merge: impl FnOnce(&mut IrBuilder, [piperine_ir::NodeId; 3])) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let lone = b.body(Self::body("Lone", vec![Self::node("p", NodeRole::Port, el), Self::node("n", NodeRole::Port, el)], vec![]));
        b.merge(lone, Merge { plus: Terminal::Node(0), minus: Terminal::Node(1) });
        let top_body = b.body(Self::body(
            "Top",
            vec![Self::node("x", NodeRole::Signal, el), Self::node("y", NodeRole::Signal, el), Self::node("z", NodeRole::Signal, el)],
            vec![Self::child("p1", lone, vec![Terminal::Node(0), Terminal::Node(1)]), Self::child("p2", lone, vec![Terminal::Node(1), Terminal::Node(2)])],
        ));
        let top = b.next_instance_id();
        let nodes: Vec<piperine_ir::NodeId> = ["x", "y", "z"].iter().map(|n| b.node(NodeSpec { path: format!("Top.{n}"), nature: el, declared_by: top, init: None, origin: Self::at(1) })).collect();
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: nodes.iter().map(|n| NodeRef::Node(*n)).collect(), params: vec![], origin: Self::at(4) });
        for (name, ends) in [("p1", [nodes[0], nodes[1]]), ("p2", [nodes[1], nodes[2]])] {
            b.instance(InstanceSpec { path: format!("Top.{name}"), name: name.into(), body: lone, parent: Some(top), nodes: ends.iter().map(|n| NodeRef::Node(*n)).collect(), params: vec![], origin: Self::at(5) });
        }
        merge(&mut b, [nodes[0], nodes[1], nodes[2]]);
        b.finish(top)
    }

    /// `Top` reads `mid`, the `signal s` of its child `leaf`; `bind` says whether placement binds the
    /// reached node. With `drive` the body also defines a reached carrier, which is broken.
    fn reaching(bind: bool, drive: bool) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let leaf = b.body(Self::body("Leaf", vec![Self::node("p", NodeRole::Port, el), Self::node("s", NodeRole::Signal, el)], vec![]));
        let leaf_q = b.carrier(leaf, Carrier { name: "q".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: Self::at(2), count: None });
        let top_body = b.body(Self::body(
            "Top",
            vec![Self::node("a", NodeRole::Signal, el), Self::node("mid", NodeRole::Reached, el)],
            vec![Self::child("leaf", leaf, vec![Terminal::Node(0), Terminal::Own(1)])],
        ));
        let seen = b.carrier(top_body, Carrier { name: "seen".into(), ty: ValueType::Bool, role: CarrierRole::Reached, init: None, origin: Self::at(4), count: None });
        b.reach(top_body, Reach { child: 0, item: ReachedItem::Node { node: 1, child_node: 1 } });
        b.reach(top_body, Reach { child: 0, item: ReachedItem::Carrier { carrier: seen, child_carrier: leaf_q } });
        if drive {
            let value = Ex::new(&mut b, "placements.msl", 6, 1).boolean(true);
            b.drive(top_body, Drive { carrier: seen, value, after: None, origin: Self::at(6), element: None, path: vec![] });
        }
        let top = b.next_instance_id();
        let a = b.node(NodeSpec { path: "Top.a".into(), nature: el, declared_by: top, init: None, origin: Self::at(4) });
        let top_net = b.net(NetSpec { path: "Top.seen".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: Self::at(4) });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![NodeRef::Node(a), NodeRef::Ground], params: vec![], origin: Self::at(4) });
        b.bind_nets(top, vec![top_net]);
        let leaf_id = b.next_instance_id();
        let s = b.node(NodeSpec { path: "Top.leaf.s".into(), nature: el, declared_by: leaf_id, init: None, origin: Self::at(2) });
        let q = b.net(NetSpec { path: "Top.leaf.q".into(), ty: ValueType::Bool, declared_by: leaf_id, init: None, origin: Self::at(2) });
        let leaf_instance = b.instance(InstanceSpec { path: "Top.leaf".into(), name: "leaf".into(), body: leaf, parent: Some(top), nodes: vec![NodeRef::Node(a), NodeRef::Node(s)], params: vec![], origin: Self::at(5) });
        b.bind_nets(leaf_instance, vec![q]);
        if bind {
            b.bind_reached_node(top, 1, NodeRef::Node(s));
            b.bind_reached_net(top, seen, q);
        }
        b.finish(top)
    }
}

#[test]
fn merging_two_nodes_leaves_one_and_rewrites_every_instance() {
    // p1 merges x and y, p2 merges y and z: the three nodes are one, and the arena holds it only.
    let ir = Placements::lones(|b, [x, y, z]| {
        b.merge_nodes(NodeRef::Node(x), y);
        b.merge_nodes(NodeRef::Node(y), z);
    });
    assert_eq!(ir.check(), vec![], "{}", ir);
    assert_eq!(ir.nodes().count(), 1, "one node left");
    let kept = ir.nodes().next().expect("a node").id();
    for instance in ir.instances() {
        assert!(instance.bindings().iter().all(|n| *n == NodeRef::Node(kept)), "{}: {:?}", instance.path(), instance.bindings());
    }
}

#[test]
fn merging_a_node_into_ground_removes_it() {
    // Every node merged into ground (each `Lone` then binds both ends to ground): the nodes are
    // gone from the arena, and every ref reads ground.
    let all = Placements::lones(|b, [x, y, z]| {
        for node in [x, y, z] {
            b.merge_nodes(NodeRef::Ground, node);
        }
    });
    assert_eq!(all.check(), vec![], "{}", all);
    assert_eq!(all.nodes().count(), 0, "every node merged into ground is gone");
    assert!(all.instances().all(|i| i.bindings().iter().all(|n| *n == NodeRef::Ground)));
}

#[test]
fn an_instance_that_binds_a_merge_to_two_nodes_is_broken() {
    // No `merge_nodes` call: p1 binds the ends of its merged branch to x and y.
    let ir = Placements::lones(|_, _| {});
    let broken = ir.check();
    assert_eq!(broken.iter().filter(|b| b.what.contains("binds the two ends of a merged branch to different nodes")).count(), 2, "{broken:?}");
}

#[test]
fn a_reached_node_is_bound_to_the_childs_node() {
    let bound = Placements::reaching(true, false);
    assert_eq!(bound.check(), vec![], "{}", bound);
    let top = bound.instances().next().expect("the top");
    let leaf = top.children().next().expect("the child");
    assert_eq!(top.bindings()[1], leaf.bindings()[1], "`mid` is the child's `s`");
    assert!(bound.to_string().contains("reach node mid = leaf.s"), "{}", bound);
    // Left unbound, `mid` is a placeholder: the check names it.
    let unbound = Placements::reaching(false, false).check();
    assert!(Switching::names(&unbound, "reached node 1 is not bound to node 1 of child Top.leaf"), "{unbound:?}");
    assert!(Switching::names(&unbound, "reached carrier 0 is not bound to carrier 0 of child Top.leaf"), "{unbound:?}");
}

#[test]
fn a_write_to_a_reached_carrier_is_broken() {
    let broken = Placements::reaching(true, true).check();
    assert!(Switching::names(&broken, "drives the reached carrier seen: a child's carrier is read only"), "{broken:?}");
}

#[test]
fn the_structure_records_print_and_round_trip() {
    // A body with an inflow ammeter `(p, p.inflow)`, a hoisted module and a merge.
    let mut b = IrBuilder::new();
    let el = Devices::electrical(&mut b);
    let mut spec = Placements::body("Rec", vec![Placements::node("p", NodeRole::Port, el), Placements::node("p.inflow", NodeRole::Signal, el)], vec![]);
    spec.branches = vec![Branch { plus: Terminal::Node(1), minus: Terminal::Node(0), kind: BranchKind::Potential }];
    let body = b.body(spec);
    b.inflow(body, Inflow { port: 0, branch: 0 });
    b.hoisted(body, Hoisted { name: "lag".into(), module: "Lag".into(), origin: Placements::at(7) });
    b.merge(body, Merge { plus: Terminal::Node(0), minus: Terminal::Node(1) });
    let top = b.instance(InstanceSpec { path: "Rec".into(), name: "Rec".into(), body, parent: None, nodes: vec![NodeRef::Ground, NodeRef::Ground], params: vec![], origin: Placements::at(1) });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "{}", ir);
    let text = ir.to_string();
    for line in ["hoisted lag: Lag", "merge (p, p.inflow)", "inflow p measured by b0"] {
        assert!(text.contains(line), "{line} in {text}");
    }
    let back: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serialize")).expect("deserialize");
    assert_eq!(back, ir);
    assert_eq!(back.bodies().next().expect("a body").inflows().len(), 1);
}

/// A body `Lag` with one param `tol: f64`, an `out` carrier `seen`, and one reaction that commits
/// what the test builds.
struct ParamEvents;

impl ParamEvents {
    fn at() -> Origin {
        Origin { file: "param_events.msl".into(), line: 2, col: 3 }
    }

    /// `source` is the event E0 the reaction waits on; `value` builds what it commits to `seen`
    /// (a carrier of type `seen_ty`).
    fn ir(source: EventSource, seen_ty: ValueType, value: impl FnOnce(&mut IrBuilder) -> ExprId) -> Ir {
        let mut b = IrBuilder::new();
        let origin = Origin { file: "param_events.msl".into(), line: 1, col: 1 };
        let value = value(&mut b);
        let commit = b.stmt(Stmt::Commit { carrier: 0, value, after: None, target: None });
        let tolerance = Param { name: "tol".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec { name: "Lag".into(), origin: origin.clone(), params: vec![tolerance], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        b.carrier(body, Carrier { name: "seen".into(), ty: seen_ty.clone(), role: CarrierRole::Out, init: None, origin: origin.clone(), count: None });
        let event = b.event(body, Event { source, guard: None, origin: Self::at() });
        b.reaction(body, Reaction { on: vec![event], stmts: vec![commit], locals: vec![], origin: Self::at() });
        let top = b.next_instance_id();
        let net = b.net(NetSpec { path: "Lag.seen".into(), ty: seen_ty, declared_by: top, init: None, origin: origin.clone() });
        let top = b.instance(InstanceSpec { path: "Lag".into(), name: "Lag".into(), body, parent: None, nodes: vec![], params: vec![Constant::Real(Real::new(1e-9))], origin });
        b.bind_nets(top, vec![net]);
        b.finish(top)
    }

    fn fired(b: &mut IrBuilder) -> ExprId {
        b.expr(Expr::Fired(0), ValueType::Bool, Self::at())
    }

    fn previous(b: &mut IrBuilder, ty: ValueType) -> ExprId {
        b.expr(Expr::ParamPrevious(0), ty, Self::at())
    }
}

#[test]
fn a_param_changed_event_prints_and_round_trips() {
    // `on tol.changed() { seen <- fired(E0); }`
    let ir = ParamEvents::ir(EventSource::ParamChanged(0), ValueType::Bool, ParamEvents::fired);
    assert_eq!(ir.check(), vec![], "{}", ir);
    assert!(ir.to_string().contains("event E0 param_changed tol"), "{}", ir);
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.param_changes(Target::Reaction(0)), vec![0], "the reaction reads the changed byte of `tol`");
    assert!(body.param_previous_reads(Target::Reaction(0)).is_empty());
    let back: Ir = serde_json::from_str(&serde_json::to_string(&ir).expect("serialize")).expect("deserialize");
    assert_eq!(back, ir);
}

#[test]
fn a_param_previous_read_is_typed_as_the_param() {
    // `seen <- tol.prev();` is an `f64`, the param's own type, and the target lists the param.
    let ir = ParamEvents::ir(EventSource::ParamChanged(0), ValueType::F64, |b| ParamEvents::previous(b, ValueType::F64));
    assert_eq!(ir.check(), vec![], "{}", ir);
    assert!(ir.to_string().contains("seen <- tol.prev()"), "{}", ir);
    let body = ir.bodies().next().expect("a body");
    assert_eq!(body.param_previous_reads(Target::Reaction(0)), vec![0]);
    assert!(body.param_changes(Target::Reaction(0)).is_empty());
    // A `prev()` of another type is not what the form gives.
    let wrong = ParamEvents::ir(EventSource::ParamChanged(0), ValueType::F64, |b| ParamEvents::previous(b, ValueType::Bool));
    assert!(Switching::names(&wrong.check(), "expression is typed bool, its form gives f64"), "{:?}", wrong.check());
}

#[test]
fn a_param_event_out_of_range_is_broken() {
    let event = ParamEvents::ir(EventSource::ParamChanged(3), ValueType::Bool, ParamEvents::fired);
    assert!(Switching::names(&event.check(), "`changed` of param 3, out of range (1 params)"), "{:?}", event.check());
    let read = ParamEvents::ir(EventSource::ParamChanged(0), ValueType::F64, |b| {
        b.expr(Expr::ParamPrevious(2), ValueType::F64, ParamEvents::at())
    });
    assert!(Switching::names(&read.check(), "previous value of param 2, out of range (1 params)"), "{:?}", read.check());
}
