//! Scope: the storage of `piperine-ir` (IR-01), the node set (IR-07), the handles (IR-02) and the
//! body and instance tables (IR-08), all on hand-built IRs.

use std::mem::size_of;

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::Devices;
use piperine_ir::{
    BinaryOp, Body, BodyId, BodyNode, BodySpec, Branch, BranchKind, NodeRef, Site, Constant, Env, Expr, ExprId, Function,
    FunctionId, Instance, InstanceId, InstanceSpec, Ir, IrBuilder, MathFn, Nature, NatureId, Node,
    NodeId, NodeRole, NodeSpec, Operator, Origin, Quantity, QuantityRole, Real, Stmt, StmtId,
    Terminal, UnaryOp, ValueType,
};

#[test]
fn ids_are_u32_newtypes_one_per_arena() {
    assert_eq!(size_of::<NatureId>(), 4);
    assert_eq!(size_of::<BodyId>(), 4);
    assert_eq!(size_of::<FunctionId>(), 4);
    assert_eq!(size_of::<ExprId>(), 4);
    assert_eq!(size_of::<StmtId>(), 4);
    assert_eq!(size_of::<InstanceId>(), 4);
    assert_eq!(size_of::<NodeId>(), 4);
    // Seven distinct types: each constructs from a bare u32 and indexes its arena.
    assert_eq!(NatureId(1).index(), 1);
    assert_eq!(BodyId(2).index(), 2);
    assert_eq!(FunctionId(3).index(), 3);
    assert_eq!(ExprId(4).index(), 4);
    assert_eq!(StmtId(5).index(), 5);
    assert_eq!(InstanceId(6).index(), 6);
    assert_eq!(NodeId(7).index(), 7);
}

/// A static check: the trait bounds are compile-time facts of the type.
struct Bounds<T: Send + Sync>(std::marker::PhantomData<T>);

#[test]
fn ir_is_send_and_sync() {
    let _ = Bounds::<Ir>(std::marker::PhantomData);
}

/// Every Rust source file of the crate, as text.
struct Sources;

impl Sources {
    fn all() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("read src") {
            let path = entry.expect("dir entry").path();
            let text = std::fs::read_to_string(&path).expect("read a source");
            out.push((path.display().to_string(), text));
        }
        out
    }
}

#[test]
fn a_finished_ir_has_no_mutating_operation() {
    // No `&mut self` method on `Ir` or on a handle: the builder (and the printer's own buffer)
    // are the only types that mutate.
    let read_only = ["Ir ", "Ir{", "Body<", "Function<", "Instance<", "Node<", "Nature<"];
    for (path, text) in Sources::all() {
        let mut guarded = false;
        for (n, line) in text.lines().enumerate() {
            if line.starts_with("impl") {
                let target = line.rsplit(" for ").next().unwrap_or(line);
                let target = target.trim_start_matches("impl").trim_start_matches("<'a>").trim();
                guarded = read_only.iter().any(|t| target.starts_with(t.trim_end()) && !target.starts_with("IrBuilder"));
            }
            if line.contains("&mut self") && guarded {
                panic!("{path}:{}: a mutating method on the IR or a handle: {line}", n + 1);
            }
        }
    }
}

fn origin() -> Origin {
    Origin { file: "t.msl".into(), line: 1, col: 1 }
}

/// One fixed sequence of builder calls.
struct Sequence;

impl Sequence {
    fn build() -> Ir {
        let mut b = IrBuilder::new();
        let nature = b.nature(
            "Electrical",
            Quantity { name: "voltage".into(), access: "v".into(), units: "V".into(), abstol: Real::new(1e-6) },
            None,
        );
        let body = b.body(BodySpec {
            name: "Empty".into(),
            origin: origin(),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.instance(InstanceSpec {
            path: "Empty".into(),
            name: "Empty".into(),
            body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: origin(),
        });
        b.node(NodeSpec { path: "Empty.n".into(), nature, declared_by: top, init: None, origin: origin() });
        b.finish(top)
    }
}

#[test]
fn equal_builder_sequences_give_equal_irs() {
    assert_eq!(Sequence::build(), Sequence::build());
    assert_eq!(Sequence::build().to_string(), Sequence::build().to_string());
}

/// The five device bodies, the divider on top of them, and the `Everything` body that uses each
/// form the devices do not.
struct Library;

impl Library {
    /// All of it, finished, with the divider as top.
    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        Devices::everything(&mut b);
        Devices::capacitor(&mut b);
        Devices::isource(&mut b);
        Devices::diode(&mut b);
        let top = Devices::divider(&mut b);
        b.finish(top)
    }

    fn body<'a>(ir: &'a Ir, name: &str) -> Body<'a> {
        ir.bodies().find(|b| b.name() == name).expect("body present")
    }
}

/// A body with three branches, two `limit` and two `ac_stim` sites and two signals, placed as the
/// top; the design declares its two nodes, the second with an initial potential.
struct Lists;

impl Lists {
    fn origin(line: u32) -> Origin {
        Origin { file: "lists.msl".into(), line, col: 1 }
    }

    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let signal = |name: &str, init: Option<f64>, line: u32| BodyNode {
            name: name.into(),
            nature: el,
            role: NodeRole::Signal,
            init: init.map(Real::new),
            origin: Lists::origin(line),
        };
        let site = |name: &str, line: u32| Site { name: name.into(), origin: Lists::origin(line), enabled: None };
        let (x, y, gnd) = (Terminal::Node(0), Terminal::Node(1), Terminal::Ground);
        let body = b.body(BodySpec {
            name: "Lists".into(),
            origin: Lists::origin(1),
            params: vec![],
            nodes: vec![signal("x", None, 2), signal("y", Some(0.25), 3)],
            branches: vec![
                Branch { plus: y, minus: gnd, kind: BranchKind::Flow },
                Branch { plus: x, minus: y, kind: BranchKind::Potential },
                Branch { plus: x, minus: gnd, kind: BranchKind::Flow },
            ],
            limits: vec![site("lb", 4), site("la", 5)],
            stims: vec![site("sb", 6), site("sa", 7)],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.next_instance_id();
        let n_x = b.node(NodeSpec { path: "Lists.x".into(), nature: el, declared_by: top, init: None, origin: Lists::origin(2) });
        let n_y = b.node(NodeSpec {
            path: "Lists.y".into(),
            nature: el,
            declared_by: top,
            init: Some(Real::new(0.25)),
            origin: Lists::origin(3),
        });
        let top = b.instance(InstanceSpec {
            path: "Lists".into(),
            name: "Lists".into(),
            body,
            parent: None,
            nodes: vec![NodeRef::Node(n_x), NodeRef::Node(n_y)],
            params: vec![],
            origin: Lists::origin(1),
        });
        b.finish(top)
    }
}

/// Every expression reachable from a body's `analog` block, in a depth-first walk.
struct Reach;

impl Reach {
    fn body(body: Body<'_>) -> Vec<ExprId> {
        let mut out = Vec::new();
        for &stmt in body.analog().unwrap_or(&[]) {
            Reach::stmt(body.ir(), stmt, &mut out);
        }
        out
    }

    fn stmt(ir: &Ir, id: StmtId, out: &mut Vec<ExprId>) {
        match ir.stmt(id) {
            Stmt::Let { init, .. } => Reach::expr(ir, *init, out),
            Stmt::Expr(e) | Stmt::BoundStep { value: e, .. } => Reach::expr(ir, *e, out),
            Stmt::Contribute { addends, .. } => {
                for a in addends {
                    Reach::expr(ir, a.value, out);
                }
            }
            Stmt::Commit { value, after, .. } => {
                Reach::expr(ir, *value, out);
                if let Some(a) = after {
                    Reach::expr(ir, *a, out);
                }
            }
            Stmt::Emit { fields, .. } => {
                for f in fields {
                    if let piperine_ir::EmitField::Value { value, .. } = f {
                        Reach::expr(ir, *value, out);
                    }
                }
            }
        }
    }

    fn expr(ir: &Ir, id: ExprId, out: &mut Vec<ExprId>) {
        out.push(id);
        match ir.expr(id) {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) => {}
            Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::Unknown | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::ParamPrevious(_) => {}
            Expr::Unwrap { is, value, .. } => {
                Reach::expr(ir, *is, out);
                Reach::expr(ir, *value, out);
            }
            Expr::Level { value: a, index: b, .. } | Expr::Merge { a, b } | Expr::Same { a, b } => {
                Reach::expr(ir, *a, out);
                Reach::expr(ir, *b, out);
            }
            Expr::LevelCast { expr, .. } => Reach::expr(ir, *expr, out),
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => Reach::expr(ir, *expr, out),
            Expr::KnownOr { value, default } => {
                Reach::expr(ir, *value, out);
                Reach::expr(ir, *default, out);
            }
            Expr::Method { receiver, args, .. } => {
                Reach::expr(ir, *receiver, out);
                for a in args {
                    Reach::expr(ir, *a, out);
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                Reach::expr(ir, *lhs, out);
                Reach::expr(ir, *rhs, out);
            }
            Expr::If { cond, then, otherwise } => {
                for e in [cond, then, otherwise] {
                    Reach::expr(ir, *e, out);
                }
            }
            Expr::Block { stmts, value } => {
                for s in stmts {
                    Reach::stmt(ir, *s, out);
                }
                if let Some(v) = value {
                    Reach::expr(ir, *v, out);
                }
            }
            Expr::Call { args, .. } | Expr::Math { args, .. } => {
                for a in args {
                    Reach::expr(ir, *a, out);
                }
            }
            Expr::Closure { body, .. } => Reach::expr(ir, *body, out),
            Expr::Limit { x, f, seed, .. } => {
                Reach::expr(ir, *x, out);
                Reach::expr(ir, *f, out);
                if let Some(s) = seed {
                    Reach::expr(ir, *s, out);
                }
            }
            Expr::Stim { mag, phase, .. } => {
                Reach::expr(ir, *mag, out);
                if let Some(p) = phase {
                    Reach::expr(ir, *p, out);
                }
            }
        }
    }

    /// The kinds reachable from `body`.
    fn kinds(body: Body<'_>) -> Vec<Expr> {
        Reach::body(body).into_iter().map(|id| body.ir().expr(id).clone()).collect()
    }
}

#[test]
fn value_code_of_the_subset_is_representable() {
    let ir = Library::ir();
    let kinds = Reach::kinds(Library::body(&ir, "Everything"));
    let has = |f: fn(&Expr) -> bool| kinds.iter().any(f);
    assert!(has(|e| matches!(e, Expr::Real(_))));
    assert!(has(|e| matches!(e, Expr::Bool(true))));
    assert!(has(|e| matches!(e, Expr::Local(_))));
    assert!(has(|e| matches!(e, Expr::Unary { op: UnaryOp::Neg, .. })));
    assert!(has(|e| matches!(e, Expr::Unary { op: UnaryOp::Not, .. })));
    assert!(has(|e| matches!(e, Expr::Binary { op: BinaryOp::Or, .. })));
    assert!(has(|e| matches!(e, Expr::If { .. })));
    assert!(has(|e| matches!(e, Expr::Block { stmts, value: Some(_) } if stmts.len() == 1)));
    assert!(has(|e| matches!(e, Expr::Call { args, .. } if args.len() == 1)));
    for f in [MathFn::Exp, MathFn::Ln, MathFn::Sqrt, MathFn::Abs] {
        assert!(has(|e| matches!(e, Expr::Math { .. })));
        assert!(kinds.iter().any(|e| matches!(e, Expr::Math { f: g, .. } if *g == f)), "{f:?}");
    }
    assert!(has(|e| matches!(e, Expr::Closure { params, .. } if params == &vec![2])));
    // The devices bring Param, Unit (as a statement) and the rest.
    let diode = Reach::kinds(Library::body(&ir, "Diode"));
    assert!(diode.iter().any(|e| matches!(e, Expr::Param(_))));
    // `Unit` is reachable as a statement expression of the Everything body.
    let body = Library::body(&ir, "Everything");
    let stmts = body.analog().expect("analog");
    assert!(stmts.iter().any(|s| matches!(ir.stmt(*s), Stmt::Expr(e) if matches!(ir.expr(*e), Expr::Unit))));
    assert!(stmts.iter().any(|s| matches!(ir.stmt(*s), Stmt::Let { .. })));
    assert!(stmts.iter().any(|s| matches!(ir.stmt(*s), Stmt::Contribute { .. })));
}

#[test]
fn a_body_holds_params_nodes_branches_and_sites() {
    let ir = Library::ir();
    let diode = Library::body(&ir, "Diode");
    let names: Vec<&str> = diode.params().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["is", "n", "cj0"]);
    let is = &diode.params()[0];
    assert_eq!(is.ty, ValueType::F64);
    assert_eq!(is.default, Some(Constant::Real(Real::new(1e-14))));
    assert!(is.check.is_some());
    assert!(!is.structural);
    let flag = &Library::body(&ir, "Everything").params()[0];
    assert!(flag.structural);
    assert_eq!(flag.default, Some(Constant::Bool(true)));
    assert_eq!(flag.ty, ValueType::Bool);
    assert!(Library::body(&ir, "Resistor").params()[0].default.is_none());

    let nodes = diode.nodes();
    assert_eq!((nodes[0].name.as_str(), nodes[0].role), ("a", NodeRole::Port));
    assert_eq!((nodes[1].name.as_str(), nodes[1].role), ("k", NodeRole::Port));
    let everything = Library::body(&ir, "Everything");
    assert_eq!(everything.nodes()[0].role, NodeRole::Port);
    assert_eq!(everything.nodes()[1].role, NodeRole::Signal);
    assert_eq!(everything.nodes()[1].init, Some(Real::new(0.5)));

    assert_eq!(diode.branches().len(), 1);
    assert_eq!(diode.branches()[0].kind, BranchKind::Flow);
    assert_eq!(diode.limits().len(), 1);
    assert_eq!(diode.limits()[0].name, "vd");
    assert_eq!(Library::body(&ir, "Vsource").stims().len(), 1);
    assert_eq!(diode.analog().expect("analog").len(), 5);
    assert_eq!(diode.origin(), &Origin { file: "devices.msl".into(), line: 31, col: 1 });
}

#[test]
fn a_probe_spans_two_terminals() {
    let ir = Library::ir();
    let probes = |name: &str| -> Vec<(Terminal, Terminal)> {
        Reach::kinds(Library::body(&ir, name))
            .into_iter()
            .filter_map(|e| match e {
                Expr::Probe { plus, minus } => Some((plus, minus)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(probes("Resistor"), [(Terminal::Node(0), Terminal::Node(1))]);
    assert_eq!(probes("Everything"), [(Terminal::Node(0), Terminal::Ground)]);
}

#[test]
fn environment_reads_are_nodes() {
    let ir = Library::ir();
    let envs = |name: &str| -> Vec<Env> {
        Reach::kinds(Library::body(&ir, name))
            .into_iter()
            .filter_map(|e| match e {
                Expr::Env(env) => Some(env),
                _ => None,
            })
            .collect()
    };
    assert_eq!(envs("Vsource"), [Env::SourceScale]);
    assert_eq!(envs("Diode"), [Env::Gmin]);
    assert_eq!(envs("Everything"), [Env::Temperature]);
}

#[test]
fn limit_and_stim_sites_are_indexed() {
    let ir = Library::ir();
    let diode = Reach::kinds(Library::body(&ir, "Diode"));
    let limit = diode.iter().find_map(|e| match e {
        Expr::Limit { site, seed, .. } => Some((*site, seed.is_some())),
        _ => None,
    });
    assert_eq!(limit, Some((0, true)));
    let vsource = Reach::kinds(Library::body(&ir, "Vsource"));
    let stim = vsource.iter().find_map(|e| match e {
        Expr::Stim { site, phase, .. } => Some((*site, phase.is_some())),
        _ => None,
    });
    assert_eq!(stim, Some((0, false)));
    let everything = Reach::kinds(Library::body(&ir, "Everything"));
    assert!(everything.iter().any(|e| matches!(e, Expr::Stim { site: 0, phase: Some(_), .. })));
    // Every used index addresses the body's own site list.
    assert_eq!(Library::body(&ir, "Diode").limits().len(), 1);
    assert_eq!(Library::body(&ir, "Everything").stims().len(), 1);
}

#[test]
fn a_contribution_is_addends_direct_or_ddt() {
    let ir = Library::ir();
    let contribution = |name: &str| -> (u32, QuantityRole, Vec<(Operator, bool)>) {
        let body = Library::body(&ir, name);
        let stmt = body.analog().expect("analog").iter().find_map(|s| match ir.stmt(*s) {
            Stmt::Contribute { branch, quantity, addends } => Some((
                *branch,
                *quantity,
                addends.iter().map(|a| (a.operator, a.negated)).collect(),
            )),
            _ => None,
        });
        stmt.expect("a contribution")
    };
    assert_eq!(
        contribution("Diode"),
        (0, QuantityRole::Flow, vec![(Operator::Direct, false), (Operator::Ddt, false)])
    );
    assert_eq!(contribution("Capacitor"), (0, QuantityRole::Flow, vec![(Operator::Ddt, false)]));
    assert_eq!(contribution("Vsource"), (0, QuantityRole::Potential, vec![(Operator::Direct, false)]));
    assert_eq!(contribution("Everything"), (0, QuantityRole::Potential, vec![(Operator::Ddt, true)]));
}

#[test]
fn a_nature_carries_abstol_and_units() {
    let ir = Library::ir();
    let electrical = ir.natures().next().expect("a nature");
    assert_eq!(electrical.name(), "Electrical");
    let v = electrical.potential();
    assert_eq!((v.access.as_str(), v.units.as_str(), v.abstol), ("v", "V", Real::new(1e-6)));
    let i = electrical.flow().expect("flow");
    assert_eq!((i.access.as_str(), i.units.as_str(), i.abstol), ("i", "A", Real::new(1e-12)));
    assert_eq!(ir.natures().count(), 1);
}

#[test]
fn every_expression_records_its_type() {
    let ir = Library::ir();
    for body in ir.bodies() {
        for id in Reach::body(body) {
            let ty = ir.expr_type(id);
            let ok = match ir.expr(id) {
                Expr::Real(_) | Expr::Probe { .. } | Expr::Env(_) | Expr::Math { .. } => *ty == ValueType::F64,
                Expr::Limit { .. } | Expr::Stim { .. } | Expr::Call { .. } => *ty == ValueType::F64,
                Expr::Bool(_) | Expr::Unary { op: UnaryOp::Not, .. } => *ty == ValueType::Bool,
                Expr::Unit => *ty == ValueType::Unit,
                Expr::Closure { params, .. } => {
                    *ty == ValueType::Fn(vec![ValueType::F64; params.len()], Box::new(ValueType::F64))
                }
                Expr::Binary { op, .. } => match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => *ty == ValueType::F64,
                    _ => *ty == ValueType::Bool,
                },
                _ => true,
            };
            assert!(ok, "{}: {:?} typed {ty:?}", body.name(), ir.expr(id));
        }
    }
}

#[test]
fn a_body_without_analog_has_no_branches() {
    let ir = Library::ir();
    let divider = Library::body(&ir, "Divider");
    assert!(divider.analog().is_none());
    assert!(divider.branches().is_empty());
    assert!(divider.limits().is_empty());
    assert!(divider.stims().is_empty());
}

/// A handle is `Copy`: a static fact of the type.
struct CopyBound<T: Copy>(std::marker::PhantomData<T>);

#[test]
fn handles_are_copy() {
    let _ = CopyBound::<Body<'static>>(std::marker::PhantomData);
    let _ = CopyBound::<Function<'static>>(std::marker::PhantomData);
    let _ = CopyBound::<Instance<'static>>(std::marker::PhantomData);
    let _ = CopyBound::<Node<'static>>(std::marker::PhantomData);
    let _ = CopyBound::<Nature<'static>>(std::marker::PhantomData);
    let ir = Library::ir();
    let top = ir.top();
    let again = top; // used after the copy
    assert_eq!(top.path(), again.path());
}

#[test]
fn contained_items_come_in_declaration_order() {
    let ir = Library::ir();
    let diode = Library::body(&ir, "Diode");
    let params: Vec<&str> = diode.params().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(params, ["is", "n", "cj0"]);
    let nodes: Vec<&str> = diode.nodes().iter().map(|n| n.name.as_str()).collect();
    assert_eq!(nodes, ["a", "k"]);
    let divider = Library::body(&ir, "Divider");
    let children: Vec<&str> = divider.children().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(children, ["v1", "r1", "r2"]);
    let kids: Vec<&str> = ir.top().children().map(|i| i.name()).collect();
    assert_eq!(kids, ["v1", "r1", "r2"]);
    // Branches and sites on lists of more than one item, declared out of name order.
    let ir = Lists::ir();
    let body = ir.top().body();
    let branches: Vec<(Terminal, Terminal, BranchKind)> = body.branches().iter().map(|b| (b.plus, b.minus, b.kind)).collect();
    assert_eq!(
        branches,
        [
            (Terminal::Node(1), Terminal::Ground, BranchKind::Flow),
            (Terminal::Node(0), Terminal::Node(1), BranchKind::Potential),
            (Terminal::Node(0), Terminal::Ground, BranchKind::Flow),
        ]
    );
    let limits: Vec<&str> = body.limits().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(limits, ["lb", "la"]);
    let stims: Vec<&str> = body.stims().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(stims, ["sb", "sa"]);
}

#[test]
fn an_instance_reaches_its_parent_and_body() {
    let ir = Library::ir();
    let r1 = ir.instance_at("Divider.r1").expect("r1");
    assert_eq!(r1.parent().map(|p| p.path()), Some("Divider"));
    assert_eq!(r1.body().name(), "Resistor");
    assert!(ir.top().parent().is_none());
    assert_eq!(r1.param(0), Some(&Constant::Real(Real::new(1000.0))));
    assert!(r1.param(1).is_none());
    assert_eq!(r1.node(0).map(|n| n.path()), Some("Divider.vin"));
    assert!(ir.instance_at("Divider.r2").expect("r2").node(1).is_none(), "ground has no node");
}

#[test]
fn path_lookup_finds_instances_and_nodes() {
    let ir = Library::ir();
    assert_eq!(ir.instance_at("Divider").map(|i| i.id()), Some(ir.top().id()));
    assert_eq!(ir.instance_at("Divider.v1").map(|i| i.name()), Some("v1"));
    assert!(ir.instance_at("Divider.nope").is_none());
    assert_eq!(ir.node_at("Divider.mid").map(|n| n.path()), Some("Divider.mid"));
    assert!(ir.node_at("Divider.nope").is_none());
    assert!(ir.node_at("Divider").is_none(), "an instance path is not a node path");
    let mid = ir.node_at("Divider.mid").expect("mid");
    let wired: Vec<&str> = mid.instances().into_iter().map(|i| i.path()).collect();
    assert_eq!(wired, ["Divider.r1", "Divider.r2"]);
}

#[test]
fn one_body_serves_every_instance_of_it() {
    let ir = Library::ir();
    let r1 = ir.instance_at("Divider.r1").expect("r1");
    let r2 = ir.instance_at("Divider.r2").expect("r2");
    assert_eq!(r1.body().id(), r2.body().id());
    assert_eq!(ir.bodies().filter(|b| b.name() == "Resistor").count(), 1);
}

#[test]
fn instances_are_stored_in_pre_order() {
    let ir = Library::ir();
    let paths: Vec<&str> = ir.instances().map(|i| i.path()).collect();
    assert_eq!(paths, ["Divider", "Divider.v1", "Divider.r1", "Divider.r2"]);
    for inst in ir.instances() {
        if let Some(parent) = inst.parent() {
            assert!(parent.id() < inst.id(), "a parent precedes its children");
        }
        assert_eq!(inst.bindings().len(), inst.body().nodes().len());
        assert_eq!(inst.params().len(), inst.body().params().len());
    }
    let v1 = ir.instance_at("Divider.v1").expect("v1");
    assert_eq!(v1.params(), [Constant::Real(Real::new(10.0)), Constant::Real(Real::new(0.0))]);
    assert_eq!(v1.binding(1), Some(piperine_ir::NodeRef::Ground));
}

#[test]
fn the_node_table_names_nature_declarer_and_init() {
    let ir = Library::ir();
    let paths: Vec<&str> = ir.nodes().map(|n| n.path()).collect();
    assert_eq!(paths, ["Divider.vin", "Divider.mid"]);
    for node in ir.nodes() {
        assert_eq!(node.nature().name(), "Electrical");
        assert_eq!(node.declared_by().path(), "Divider");
        assert!(node.init().is_none());
    }
    // A design node with an initial potential keeps it; its neighbour has none.
    let ir = Lists::ir();
    let inits: Vec<(&str, Option<f64>)> = ir.nodes().map(|n| (n.path(), n.init().map(Real::get))).collect();
    assert_eq!(inits, [("Lists.x", None), ("Lists.y", Some(0.25))]);
    let y = ir.node_at("Lists.y").expect("y");
    assert_eq!(y.nature().name(), "Electrical");
    assert_eq!(y.declared_by().path(), "Lists");
}

#[test]
fn the_authored_tree_is_walkable() {
    let ir = Library::ir();
    let mut leaves = Vec::new();
    let mut stack = vec![ir.top()];
    while let Some(inst) = stack.pop() {
        let kids: Vec<_> = inst.children().collect();
        if kids.is_empty() {
            leaves.push(inst.path());
        }
        stack.extend(kids.into_iter().rev());
    }
    assert_eq!(leaves, ["Divider.v1", "Divider.r1", "Divider.r2"]);
    // The functions the bodies call are reachable too.
    let names: Vec<&str> = ir.functions().map(|f| f.name()).collect();
    assert!(names.contains(&"vt") && names.contains(&"pnjlim"));
    let pnjlim = ir.functions().find(|f| f.name() == "pnjlim").expect("pnjlim");
    assert_eq!(pnjlim.params().len(), 4);
    assert_eq!(*pnjlim.ret(), ValueType::F64);
}
