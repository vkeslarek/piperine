//! The interpreter is frozen at the M1 analog subset (R1, spec RS-09 AC 6): an `Ir` that holds a
//! body with a carrier, a drive or a reaction is `Rejected` by `Interpreter::new`, one entry
//! naming each such body; an analog body beside them is not named. So is an `Ir` that holds a
//! form of the msl-digital amendments (the clock, the analysis, an `at` event, an emit site, a
//! branch flow, a previous value): one entry per body and form, by name.

use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{
    Addend, AnalysisKind, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, Carrier, CarrierRole, Child, EmitField, Env, Event, EventSource, Expr, ExprId, FunctionSpec, Hoisted,
    Inflow, InstanceSpec, Ir, LevelPart, IrBuilder, Local, NetSpec, NodeRef, NodeRole, NodeSpec, Operator, Origin, Place, QuantityRole, Reach, ReachedItem, Site, Stmt, Stream,
    StreamField, StmtId, Terminal, ValueType,
};
use piperine_ir::{Derives, FieldsForm, TypeData, TypeKind, VariantData};
use piperine_ir::Method as M;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/models.rs"]
mod models;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/structure.rs"]
mod structure;

use devices::Devices;
use digital::{Buck, Dx, Ty};
use models::{In, Models, View};
use structure::{ParamEvents, Switching, Written};

#[test]
fn a_digital_body_is_rejected_by_name() {
    let mut b = IrBuilder::new();
    Devices::resistor(&mut b);
    Buck::osc(&mut b); // a carrier and a reaction
    Buck::gate_driver(&mut b); // carriers and drives
    Buck::switch(&mut b); // an `analog` block reading a carrier
    let top_body = b.body(BodySpec {
        name: "Top".into(),
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
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
        path: "Top".into(),
        name: "Top".into(),
        body: top_body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");

    let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a digital design was accepted") };
    let named: Vec<(&str, &str, u32)> = rejected.entries.iter().map(|e| (e.item.as_str(), e.origin.file.as_str(), e.origin.line)).collect();
    assert_eq!(named, vec![("body Osc", "dpwm.msl", 7), ("body GateDriver", "power.msl", 74), ("body Switch", "switch.msl", 1)]);
    for entry in &rejected.entries {
        assert!(entry.what.contains(&entry.item[5..]), "{entry:?}");
        assert!(entry.what.contains("carriers, drives or reactions"), "{entry:?}");
    }
}

/// The bodies of the msl-digital amendments, each holding one form (or two), built by hand.
struct Amended;

impl Amended {
    const F: &'static str = "amended.msl";

    fn at(line: u32) -> Origin {
        Origin { file: Amended::F.into(), line, col: 1 }
    }

    fn ports(b: &mut IrBuilder) -> Vec<BodyNode> {
        let el = Buck::electrical(b);
        ["p", "n"].iter().map(|n| BodyNode { name: (*n).into(), nature: el, role: NodeRole::Port, init: None, origin: Amended::at(1) }).collect()
    }

    /// `i(p, n) +> v(p, n) * if analysis(Static) { 0.0 } else { now() };`
    fn clocked(b: &mut IrBuilder) {
        let mut x = Dx::new(b, Amended::F, 2, 10);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let stat = x.b.expr(Expr::Env(Env::Static), ValueType::Bool, o.clone());
        let zero = x.real(0.0);
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o);
        let level = x.if_(stat, zero, now, ValueType::F64);
        let value = x.bin(BinaryOp::Mul, v, level, ValueType::F64);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let nodes = Amended::ports(b);
        b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(vec![stmt]),
            ..Buck::spec("Clocked", Amended::F, 1)
        });
    }

    /// The shape of the elaborator's host source (PI-07): `v(p, gnd) +> now() + ac_stim(1.0);`
    fn host_source(b: &mut IrBuilder) {
        let mut x = Dx::new(b, Amended::F, 30, 10);
        let o = x.origin();
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o.clone());
        let mag = x.real(1.0);
        let stim = x.b.expr(Expr::Stim { site: 0, mag, phase: None }, ValueType::F64, o);
        let value = x.bin(BinaryOp::Add, now, stim, ValueType::F64);
        let stmt = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Potential, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
        let el = Buck::electrical(b);
        b.body(BodySpec {
            nodes: vec![BodyNode { name: "p".into(), nature: el, role: NodeRole::Port, init: None, origin: Amended::at(1) }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Potential }],
            stims: vec![Site { name: "ac".into(), origin: Amended::at(30), enabled: None }],
            analog: Some(vec![stmt]),
            ..Buck::spec("HostSource", Amended::F, 29)
        });
    }

    /// `v(p, n) +> ddt(i(p, n));`
    fn inductor(b: &mut IrBuilder) {
        let x = Dx::new(b, Amended::F, 12, 10);
        let o = x.origin();
        let flow = x.b.expr(Expr::Flow { branch: 0 }, ValueType::F64, o);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Potential,
            addends: vec![Addend { operator: Operator::Ddt, negated: false, value: flow }],
        });
        let nodes = Amended::ports(b);
        b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Potential }],
            analog: Some(vec![stmt]),
            ..Buck::spec("Inductor", Amended::F, 11)
        });
    }

    /// `on at(1.0) { q <- true; }`
    fn timer(b: &mut IrBuilder) {
        let body = b.body(Buck::spec("Timer", Amended::F, 21));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Amended::F, 21, 20));
        let when = Dx::new(b, Amended::F, 22, 10).real(1.0);
        let event = b.event(body, Event { source: EventSource::At(when), guard: None, origin: Amended::at(22) });
        let mut x = Dx::new(b, Amended::F, 22, 20);
        let yes = x.boolean(true);
        let commit = x.commit(0, yes, None);
        Buck::reaction(b, body, vec![event], vec![commit], vec![], Amended::F, 22);
    }

    /// `on settle { q <- true; }`
    fn settling(b: &mut IrBuilder) {
        let body = b.body(Buck::spec("Settling", Amended::F, 51));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Amended::F, 51, 20));
        let event = b.event(body, Event { source: EventSource::Settle, guard: None, origin: Amended::at(52) });
        let mut x = Dx::new(b, Amended::F, 52, 20);
        let yes = x.boolean(true);
        let commit = x.commit(0, yes, None);
        Buck::reaction(b, body, vec![event], vec![commit], vec![], Amended::F, 52);
    }

    /// `on q.changed() { emit!("sim", finish = true); }`
    fn monitor(b: &mut IrBuilder) {
        let body = b.body(Buck::spec("Monitor", Amended::F, 31));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Amended::F, 31, 20));
        let sim = b.stream(body, Stream { name: "sim".into(), fields: vec![StreamField { name: "finish".into(), ty: ValueType::Bool }] });
        let event = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Amended::at(32) });
        let mut x = Dx::new(b, Amended::F, 32, 20);
        let yes = x.boolean(true);
        let emit = x.b.stmt(Stmt::Emit { stream: sim, fields: vec![EmitField::Value { field: 0, value: yes }] });
        Buck::reaction(b, body, vec![event], vec![emit], vec![], Amended::F, 32);
    }

    /// `i(p, n) +> if clk.prev().high() { 1.0 } else { 0.0 };`
    fn edge(b: &mut IrBuilder) {
        let mut x = Dx::new(b, Amended::F, 42, 10);
        let o = x.origin();
        let prev = x.b.expr(Expr::Previous(0), ValueType::Flagged(Box::new(ValueType::Bool)), o);
        let was_high = x.high(prev);
        let one = x.real(1.0);
        let zero = x.real(0.0);
        let value = x.if_(was_high, one, zero, ValueType::F64);
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let nodes = Amended::ports(b);
        let body = b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(vec![stmt]),
            ..Buck::spec("Edge", Amended::F, 41)
        });
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, Amended::F, 41, 30));
    }
}

#[test]
fn a_body_with_an_amended_form_is_rejected_by_name() {
    let mut b = IrBuilder::new();
    Devices::resistor(&mut b); // M1 analog: not named
    Amended::clocked(&mut b);
    Amended::inductor(&mut b);
    Amended::timer(&mut b);
    Amended::monitor(&mut b);
    Amended::edge(&mut b);
    let top_body = b.body(Buck::spec("Top", "top.msl", 1));
    let top = b.instance(InstanceSpec {
        path: "Top".into(),
        name: "Top".into(),
        body: top_body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");

    let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a design with amended forms was accepted") };
    // One entry per body and form, in body order; the carriers of `Timer`, `Monitor` and `Edge`
    // are named too, as in `a_digital_body_is_rejected_by_name`. The resistor and the empty top
    // are not.
    let named: Vec<(&str, &str)> = rejected
        .entries
        .iter()
        .map(|e| (e.item.as_str(), e.what.split_once(": ").map_or(e.what.as_str(), |(head, _)| head)))
        .collect();
    assert_eq!(
        named,
        vec![
            ("body Clocked", "body Clocked holds now()"),
            ("body Clocked", "body Clocked holds analysis(Static)"),
            ("body Inductor", "body Inductor holds the flow of a potential branch"),
            ("body Timer", "body Timer has carriers, drives or reactions"),
            ("body Timer", "body Timer holds at(t)"),
            ("body Monitor", "body Monitor has carriers, drives or reactions"),
            ("body Monitor", "body Monitor holds emit!"),
            ("body Edge", "body Edge has carriers, drives or reactions"),
            ("body Edge", "body Edge holds the previous value of a carrier"),
        ]
    );
    for entry in &rejected.entries {
        assert_eq!(entry.origin.file, Amended::F, "{entry:?}");
    }
}

/// §13 rule 17 (unknown-semantics-events UE-03): the event `settle` is a form the interpreter
/// refuses by name, as it refuses `at(t)` (codegen context R1).
#[test]
fn a_settle_event_is_refused_by_name() {
    let mut b = IrBuilder::new();
    Amended::settling(&mut b);
    let top_body = b.body(Buck::spec("Top", "top.msl", 1));
    let top = b.instance(InstanceSpec {
        path: "Top".into(),
        name: "Top".into(),
        body: top_body,
        parent: None,
        nodes: vec![],
        params: vec![],
        origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
    });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");
    let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a design with a settle event was accepted") };
    let named: Vec<&str> = rejected.entries.iter().map(|e| e.what.split_once(": ").map_or(e.what.as_str(), |(head, _)| head)).collect();
    assert!(named.contains(&"body Settling holds settle"), "{named:?}");
}

/// Bodies that hold one form msl-fixes added, each in a design of its own.
struct Added;

impl Added {
    /// The names `Interpreter::new` refuses a design that `build` fills with, one per entry of
    /// its bodies (`holds <name>`).
    fn refused(build: impl FnOnce(&mut IrBuilder)) -> Vec<String> {
        let mut b = IrBuilder::new();
        build(&mut b);
        let top_body = b.body(Buck::spec("Top", "top.msl", 1));
        let top = b.instance(InstanceSpec {
            path: "Top".into(),
            name: "Top".into(),
            body: top_body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: Origin { file: "top.msl".into(), line: 1, col: 1 },
        });
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the design itself is well formed");
        let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a design with a form of msl-fixes was accepted") };
        rejected.entries.iter().filter_map(|e| e.what.split_once(" holds ").map(|(_, rest)| rest.split(':').next().unwrap_or("").to_string())).collect()
    }

    /// `i(p, n) +> <value>` as the one statement of body `name`.
    fn contribution(b: &mut IrBuilder, name: &str, value: piperine_ir::ExprId, extra: Vec<piperine_ir::StmtId>) -> piperine_ir::BodyId {
        let stmt = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        let nodes = Amended::ports(b);
        let analog: Vec<_> = extra.into_iter().chain([stmt]).collect();
        b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(analog),
            ..Buck::spec(name, Amended::F, 1)
        })
    }

    /// `i(p, n) +> v(p, n) * <read>`, the read an environment form of `ty`.
    fn env(b: &mut IrBuilder, name: &str, env: Env) {
        let mut x = Dx::new(b, Amended::F, 2, 10);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let read = match env {
            Env::Analysis(_) => {
                let on = x.b.expr(Expr::Env(env), ValueType::Bool, o);
                let (one, zero) = (x.real(1.0), x.real(0.0));
                x.if_(on, one, zero, ValueType::F64)
            }
            _ => x.b.expr(Expr::Env(env), ValueType::F64, o),
        };
        let value = x.bin(BinaryOp::Mul, v, read, ValueType::F64);
        Added::contribution(b, name, value, vec![]);
    }

    /// A body with a `cross` event of the given tolerances, and a reaction on it.
    fn cross(b: &mut IrBuilder, name: &str, ttol: bool, etol: bool) {
        let nodes = Amended::ports(b);
        let body = b.body(BodySpec { nodes, ..Buck::spec(name, Amended::F, 1) });
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Amended::F, 1, 20));
        let mut x = Dx::new(b, Amended::F, 2, 10);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let tol = x.real(1e-9);
        let event = b.event(
            body,
            Event { source: EventSource::Cross { e: v, ttol: ttol.then_some(tol), etol: etol.then_some(tol) }, guard: None, origin: Amended::at(2) },
        );
        let mut x = Dx::new(b, Amended::F, 3, 10);
        let yes = x.boolean(true);
        let commit = x.commit(0, yes, None);
        Buck::reaction(b, body, vec![event], vec![commit], vec![], Amended::F, 3);
    }

    /// `Row(p, n)`: `i(p, n) +> <the row as an f64>`, run through its `terms` function.
    fn row(method: M, recv: &ValueType, args: &[ValueType], result: &ValueType) -> String {
        let mut b = IrBuilder::new();
        let mut x = Dx::new(&mut b, Amended::F, 2, 10);
        let operand = |x: &mut Dx<'_>, ty: &ValueType| match ty {
            ValueType::F64 => x.probe(Terminal::Node(0), Terminal::Node(1)),
            other => x.int(1, other.clone()),
        };
        let r = operand(&mut x, recv);
        let a: Vec<_> = args.iter().map(|t| if *t == ValueType::F64 { x.real(0.5) } else { x.int(1, t.clone()) }).collect();
        let call = x.method(method, r, a, result.clone());
        let value = match result {
            ValueType::F64 => call,
            ValueType::Bool => {
                let (one, zero) = (x.real(1.0), x.real(0.0));
                x.if_(call, one, zero, ValueType::F64)
            }
            // An integer row is compared with itself: the walk meets the row first (a cast or
            // an integer literal would be refused before it).
            _ => {
                let same = x.bin(BinaryOp::Ge, call, call, ValueType::Bool);
                let (one, zero) = (x.real(1.0), x.real(0.0));
                x.if_(same, one, zero, ValueType::F64)
            }
        };
        let body = Added::contribution(&mut b, "Row", value, vec![]);
        let top = Devices::alone(&mut b, body, "H", 1, vec![]);
        Added::terms(Models::of(b.finish(top)))
    }

    /// The message of the fault the `terms` function of body `Row` gives.
    fn terms(m: Models) -> String {
        let view = View { shape: m.module("Row").prototype() };
        let k = view.function_names().iter().position(|n| *n == "terms").expect("a terms function") as u32;
        let reads = view.function("terms").0.len();
        let h = m.instance("Row");
        Models::eval(&h, k, &(0..reads).map(|_| In::F64(0.3)).collect::<Vec<_>>(), None).expect_err("the form is refused").message
    }
}

#[test]
fn the_interpreter_refuses_each_new_form_by_name() {
    // `Interpreter::new` names the body-level forms, one entry per body and form.
    assert_eq!(Added::refused(|b| Added::env(b, "Freq", Env::Freq)), ["freq()"]);
    for (kind, name) in [
        (AnalysisKind::Dc, "analysis(Dc)"),
        (AnalysisKind::Transient, "analysis(Transient)"),
        (AnalysisKind::Ac, "analysis(Ac)"),
        (AnalysisKind::Noise, "analysis(Noise)"),
    ] {
        assert_eq!(Added::refused(|b| Added::env(b, "Kind", Env::Analysis(kind))), [name]);
    }
    assert_eq!(
        Added::refused(|b| {
            let mut x = Dx::new(b, Amended::F, 2, 10);
            let step = x.real(1e-9);
            let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: None });
            let value = Dx::new(b, Amended::F, 3, 10).real(0.0);
            Added::contribution(b, "Cap", value, vec![cap]);
        }),
        ["bound_step"]
    );
    // a body with a `cross` is digital as well: its own refusal comes first, then the tolerance
    let named = |ttol, etol| Added::refused(|b| Added::cross(b, "Tol", ttol, etol)).into_iter().filter(|n| n.contains("cross")).collect::<Vec<_>>();
    assert_eq!(named(true, false), ["the ttol of a cross"]);
    assert_eq!(named(false, true), ["the etol of a cross"]);
    assert_eq!(named(true, true), ["the ttol of a cross", "the etol of a cross"]);
    assert_eq!(
        Added::refused(|b| {
            let body = b.body(Buck::spec("Which", Amended::F, 1));
            b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Amended::F, 1, 20));
            let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Amended::at(2) });
            let mut x = Dx::new(b, Amended::F, 3, 10);
            let fired = x.b.expr(Expr::Fired(0), ValueType::Bool, x.origin());
            let commit = x.commit(0, fired, None);
            Buck::reaction(b, body, vec![changed], vec![commit], vec![], Amended::F, 3);
        })
        .into_iter()
        .filter(|n| n == "fired")
        .collect::<Vec<_>>(),
        ["fired"]
    );

    // The walk names a method row, `unwrap` and `expect` when it meets one: one case per row.
    let (f, i8_, u8_, u32_, i32_) = (ValueType::F64, Ty::i(8), Ty::u(8), Ty::u(32), Ty::i(32));
    let rows: Vec<(M, ValueType, Vec<ValueType>, ValueType)> = vec![
        (M::Sin, f.clone(), vec![], f.clone()),
        (M::Cos, f.clone(), vec![], f.clone()),
        (M::Tan, f.clone(), vec![], f.clone()),
        (M::Asin, f.clone(), vec![], f.clone()),
        (M::Acos, f.clone(), vec![], f.clone()),
        (M::Atan, f.clone(), vec![], f.clone()),
        (M::Atan2, f.clone(), vec![f.clone()], f.clone()),
        (M::Sinh, f.clone(), vec![], f.clone()),
        (M::Cosh, f.clone(), vec![], f.clone()),
        (M::Tanh, f.clone(), vec![], f.clone()),
        (M::Powf, f.clone(), vec![f.clone()], f.clone()),
        (M::Powi, f.clone(), vec![i32_], f.clone()),
        (M::Log10, f.clone(), vec![], f.clone()),
        (M::Log2, f.clone(), vec![], f.clone()),
        (M::Ceil, f.clone(), vec![], f.clone()),
        (M::Round, f.clone(), vec![], f.clone()),
        (M::Trunc, f.clone(), vec![], f.clone()),
        (M::Fract, f.clone(), vec![], f.clone()),
        (M::Signum, f.clone(), vec![], f.clone()),
        (M::RemEuclid, f.clone(), vec![f.clone()], f.clone()),
        (M::Hypot, f.clone(), vec![f.clone()], f.clone()),
        (M::IsNan, f.clone(), vec![], ValueType::Bool),
        (M::IsFinite, f.clone(), vec![], ValueType::Bool),
        (M::IsInfinite, f.clone(), vec![], ValueType::Bool),
        (M::IntAbs, i8_.clone(), vec![], i8_.clone()),
        (M::Pow, u8_.clone(), vec![u32_], u8_.clone()),
        (M::SaturatingAdd, u8_.clone(), vec![u8_.clone()], u8_.clone()),
        (M::SaturatingSub, i8_.clone(), vec![i8_.clone()], i8_.clone()),
        (M::SaturatingMul, i8_.clone(), vec![i8_.clone()], i8_.clone()),
        (M::AddOverflows, u8_.clone(), vec![u8_.clone()], ValueType::Bool),
        (M::SubOverflows, i8_.clone(), vec![i8_.clone()], ValueType::Bool),
        (M::MulOverflows, i8_.clone(), vec![i8_.clone()], ValueType::Bool),
    ];
    for (method, recv, args, result) in &rows {
        let message = Added::row(*method, recv, args, result);
        let want = format!("the IR method `{}` is not executed by this walk", method.name());
        assert!(message.contains(&want), "{want}: {message}");
        assert!(message.contains("amended.msl:2:10"), "the origin of the row: {message}");
    }
    for (message, name) in [(None, "unwrap"), (Some("no flag".to_string()), "expect")] {
        let mut b = IrBuilder::new();
        let mut x = Dx::new(&mut b, Amended::F, 2, 10);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let yes = x.boolean(true);
        let value = x.b.expr(Expr::Unwrap { is: yes, value: v, message }, ValueType::F64, x.origin());
        let body = Added::contribution(&mut b, "Row", value, vec![]);
        let top = Devices::alone(&mut b, body, "H", 1, vec![]);
        let got = Added::terms(Models::of(b.finish(top)));
        assert!(got.contains(&format!("the IR form `{name}` is not executed by this walk")), "{name}: {got}");
    }
}

#[test]
fn the_unknown_nodes_are_refused_by_name() {
    // The walk names `level`, `merge`, `same` and `cast` when it meets one, as the source spells
    // them (R1, the interpreter is frozen at the M1 analog subset): `x.level(k)` of a value read
    // from a probe, in a statement of the block that gives the contribution its value.
    type Node = fn(&mut Dx<'_>, piperine_ir::ExprId) -> piperine_ir::ExprId;
    let nodes: [(&str, Node); 5] = [
        ("level", |x, v| {
            let k = x.int(0, Ty::u(32));
            x.level(v, k, LevelPart::Is, ValueType::Bool)
        }),
        ("level", |x, v| {
            let k = x.int(0, Ty::u(32));
            x.level(v, k, LevelPart::Value, ValueType::Bool)
        }),
        ("merge", |x, v| x.merge(v, v, Ty::flagged(ValueType::F64))),
        ("same", |x, v| x.same(v, v)),
        ("cast", |x, v| x.level_cast(v, Ty::u(64), None, Ty::u(64))),
    ];
    for (name, node) in nodes {
        let mut b = IrBuilder::new();
        let mut x = Dx::new(&mut b, Amended::F, 2, 10);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let form = node(&mut x, v);
        let held = x.stmt(form);
        let value = x.block(vec![held], Some(v), ValueType::F64);
        let body = Added::contribution(&mut b, "Row", value, vec![]);
        let top = Devices::alone(&mut b, body, "H", 1, vec![]);
        let got = Added::terms(Models::of(b.finish(top)));
        assert!(got.contains(&format!("the IR form `{name}` is not executed by this walk")), "{name}: {got}");
        assert!(got.contains("amended.msl:2:10"), "the origin of the form: {got}");
    }
}

/// Designs that hold one form of `analog-structure`, each refused by name (R1, AS-17).
struct Structured;

impl Structured {
    /// The forms `Interpreter::new` refuses `ir` for, in the order it names them (`holds <form>`).
    fn refused(ir: Ir) -> Vec<String> {
        assert_eq!(ir.check(), vec![], "the design itself is well formed");
        let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a design with a form of analog-structure was accepted") };
        rejected.entries.iter().filter_map(|e| e.what.split_once(" holds ").map(|(_, rest)| rest.split(':').next().unwrap_or("").to_string())).collect()
    }

    /// `Top` reads `mid`, the `signal s` of its child `leaf`, and the carrier `q` as `seen`.
    fn reaching() -> Ir {
        let at = |line: u32| Origin { file: "reach.msl".into(), line, col: 1 };
        let mut b = IrBuilder::new();
        let el = Buck::electrical(&mut b);
        let node = |name: &str, role| BodyNode { name: name.into(), nature: el, role, init: None, origin: at(1) };
        let spec = |name: &str, nodes, children| BodySpec { name: name.into(), origin: at(1), params: vec![], nodes, branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children };
        let leaf = b.body(spec("Leaf", vec![node("p", NodeRole::Port), node("s", NodeRole::Signal)], vec![]));
        let leaf_q = b.carrier(leaf, Carrier { name: "q".into(), ty: ValueType::Bool, role: CarrierRole::Signal, init: None, origin: at(2) });
        let child = Child { name: "leaf".into(), body: leaf, nodes: vec![Terminal::Node(0), Terminal::Own(1)], params: vec![], origin: at(5) };
        let top_body = b.body(spec("Top", vec![node("a", NodeRole::Signal), node("mid", NodeRole::Reached)], vec![child]));
        let seen = b.carrier(top_body, Carrier { name: "seen".into(), ty: ValueType::Bool, role: CarrierRole::Reached, init: None, origin: at(4) });
        b.reach(top_body, Reach { child: 0, item: ReachedItem::Node { node: 1, child_node: 1 } });
        b.reach(top_body, Reach { child: 0, item: ReachedItem::Carrier { carrier: seen, child_carrier: leaf_q } });
        let top = b.next_instance_id();
        let a = b.node(NodeSpec { path: "Top.a".into(), nature: el, declared_by: top, init: None, origin: at(4) });
        let top_net = b.net(NetSpec { path: "Top.seen".into(), ty: ValueType::Bool, declared_by: top, init: None, origin: at(4) });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![NodeRef::Node(a), NodeRef::Ground], params: vec![], origin: at(4) });
        b.bind_nets(top, vec![top_net]);
        let leaf_id = b.next_instance_id();
        let s = b.node(NodeSpec { path: "Top.leaf.s".into(), nature: el, declared_by: leaf_id, init: None, origin: at(2) });
        let q = b.net(NetSpec { path: "Top.leaf.q".into(), ty: ValueType::Bool, declared_by: leaf_id, init: None, origin: at(2) });
        let leaf_instance = b.instance(InstanceSpec { path: "Top.leaf".into(), name: "leaf".into(), body: leaf, parent: Some(top), nodes: vec![NodeRef::Node(a), NodeRef::Node(s)], params: vec![], origin: at(5) });
        b.bind_nets(leaf_instance, vec![q]);
        b.bind_reached_node(top, 1, NodeRef::Node(s));
        b.bind_reached_net(top, seen, q);
        b.finish(top)
    }
}

#[test]
fn the_interpreter_refuses_each_analog_structure_form_by_name() {
    let held = |forms: &[String], form: &str| assert!(forms.iter().any(|f| f == form), "{form} among {forms:?}");

    // a contribution under an `if` (a selector), and the same in a nested block
    let nested = Switching::ir(BranchKind::Flow, |b| {
        let (cond, taken) = (Switching::compare(b, BinaryOp::Gt, 0.5), Switching::contribute(b, QuantityRole::Flow, 1.0));
        Written { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], ..Written::default() }
    });
    held(&Structured::refused(nested), "a contribution under a selector");

    // a switch branch: both quantities written at the top of the block, so no selector is named
    let switch = Switching::ir(BranchKind::Switch, |b| {
        let (potential, flow) = (Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::contribute(b, QuantityRole::Flow, 2.0));
        Written { analog: vec![potential, flow], ..Written::default() }
    });
    assert_eq!(Structured::refused(switch), ["a switch branch"]);

    // a module placed in an expression whose node or `analog` block joined the body
    let hoisted = Added::refused(|b| {
        let body = Devices::resistor(b);
        b.hoisted(body, Hoisted { name: "lag".into(), module: "Lag".into(), origin: Amended::at(7) });
    });
    assert_eq!(hoisted, ["a module placed in an expression with a node or an analog block"]);

    // a `ddt` that is not a whole addend: a derivative site
    let derivative = Switching::ir(BranchKind::Flow, |b| {
        let site = Switching::site(b, None);
        Written { analog: vec![Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0)], derivatives: vec![site], ..Written::default() }
    });
    assert_eq!(Structured::refused(derivative), ["a ddt off a whole addend or with abstol"]);

    // a node and a carrier of a child, read as `inst.name`
    held(&Structured::refused(Structured::reaching()), "a reached node or carrier (inst.name)");

    // `p.inflow()`: the ammeter branch `(n, p)` measuring port `p`
    let inflow = Added::refused(|b| {
        let nodes = Amended::ports(b);
        let body = b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(1), minus: Terminal::Node(0), kind: BranchKind::Potential }],
            ..Buck::spec("Ammeter", Amended::F, 1)
        });
        b.inflow(body, Inflow { port: 0, branch: 0 });
    });
    assert_eq!(inflow, ["inflow()"]);

    // `changed()` of a param, and `prev()` of one
    let changed = ParamEvents::ir(ValueType::Bool, |b| b.expr(Expr::Fired(0), ValueType::Bool, ParamEvents::at()));
    held(&Structured::refused(changed), "changed() or prev() on a param");
    held(&Structured::refused(ParamEvents::previous_drive()), "changed() or prev() on a param");

    // a `limit` read under a condition (the site has an `enabled`), and a `bound_step ... when`
    let limit = Switching::ir(BranchKind::Flow, |b| {
        let read = Switching::halving_limit(b);
        let (cond, enabled) = (Switching::compare(b, BinaryOp::Gt, 0.5), Switching::compare(b, BinaryOp::Gt, 0.5));
        let taken = Switching::contribute_expr(b, QuantityRole::Flow, Operator::Direct, read);
        let site = Site { name: "lim".into(), origin: Switching::at(), enabled: Some(enabled) };
        Written { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], limits: vec![site], locals: Switching::limit_locals(), ..Written::default() }
    });
    held(&Structured::refused(limit), "a guarded limit");
    let capped = Switching::ir(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(0.5), Switching::compare(b, BinaryOp::Gt, 0.5));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    held(&Structured::refused(capped), "a bound_step under a selector");
}

#[test]
fn an_analog_body_without_new_forms_still_runs_on_the_interpreter() {
    // The five M1 bodies hold none of the forms: the interpreter accepts them and plans each.
    let mut b = IrBuilder::new();
    let resistor = Devices::resistor(&mut b);
    Devices::capacitor(&mut b);
    Devices::diode(&mut b);
    let top = Devices::alone(&mut b, resistor, "R", 1, vec![1000.0]);
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");
    let interpreter = Interpreter::new(Arc::new(ir)).expect("a body of the M1 subset is accepted");
    assert!(interpreter.module(resistor).is_some(), "the resistor has a module");
}

/// Designs that hold one form of `composite-values`, each refused by name (R1).
struct Composite;

impl Composite {
    /// The design whose `analog` block runs what `write` returns (with its locals) and then
    /// contributes `1.0`; `Structured::refused` names the forms the interpreter refuses it for.
    fn refused(write: impl FnOnce(&mut IrBuilder) -> (Vec<Local>, Vec<StmtId>)) -> Vec<String> {
        let ir = Switching::ir(BranchKind::Flow, |b| {
            let (locals, mut analog) = write(b);
            analog.push(Switching::contribute(b, QuantityRole::Flow, 1.0));
            Written { analog, locals, ..Written::default() }
        });
        Structured::refused(ir)
    }

    /// A statement that evaluates a form of type `ty`.
    fn run(b: &mut IrBuilder, kind: Expr, ty: ValueType) -> StmtId {
        let form = b.expr(kind, ty, Switching::at());
        b.stmt(Stmt::Expr(form))
    }

    /// `fn name(x: f64) -> f64 { body }` over the one local `x`.
    fn function(b: &mut IrBuilder, name: &str, body: ExprId) -> piperine_ir::FunctionId {
        let x = Local { name: "x".into(), ty: ValueType::F64, mutable: false };
        b.function(FunctionSpec { name: name.into(), origin: Switching::at(), params: vec![x.clone()], ret: ValueType::F64, locals: vec![x], body })
    }
}

#[test]
fn each_composite_values_form_is_refused_by_name() {
    // `let mut x = 0.0; x = 1.0;` (T2)
    let assignment = Composite::refused(|b| {
        let (zero, one) = (Switching::ex(b).real(0.0), Switching::ex(b).real(1.0));
        let declare = b.stmt(Stmt::Let { local: 0, init: zero });
        let set = b.stmt(Stmt::Assign { place: Place { local: 0, path: vec![] }, op: None, value: one });
        (vec![Local { name: "x".into(), ty: ValueType::F64, mutable: true }], vec![declare, set])
    });
    assert_eq!(assignment, ["an assignment"]);

    // `loop { break }`, `while true { continue }`, `for k in 0..2 { }` (T3)
    let looped = Composite::refused(|b| {
        let stop = Composite::run(b, Expr::Break(None), ValueType::Unit);
        (vec![], vec![Composite::run(b, Expr::Loop { body: vec![stop] }, ValueType::Unit)])
    });
    assert_eq!(looped, ["a loop", "break"]);
    let whiled = Composite::refused(|b| {
        let (always, skip) = (Switching::ex(b).boolean(true), Composite::run(b, Expr::Continue, ValueType::Unit));
        (vec![], vec![Composite::run(b, Expr::While { cond: always, body: vec![skip] }, ValueType::Unit)])
    });
    assert_eq!(whiled, ["a while loop", "continue"]);
    let ranged = Composite::refused(|b| {
        let (low, high) = {
            let mut x = Dx::new(b, "composite.msl", 2, 5);
            (x.int(0, Ty::u(8)), x.int(2, Ty::u(8)))
        };
        let kind = Expr::ForRange { local: 0, low, high, inclusive: false, reverse: false, body: vec![] };
        (vec![Local { name: "k".into(), ty: Ty::u(8), mutable: false }], vec![Composite::run(b, kind, ValueType::Unit)])
    });
    assert_eq!(ranged, ["a for loop"]);

    // `fn early(x) -> f64 { return x }` called from the block (T4), and the same function named
    // and called through its value (T5)
    let returned = Composite::refused(|b| {
        let result = Switching::ex(b).local(0);
        let exit = Composite::run(b, Expr::Return(Some(result)), ValueType::F64);
        let fallthrough = Switching::ex(b).real(0.0);
        let body = Switching::ex(b).block(vec![exit], Some(fallthrough), ValueType::F64);
        let early = Composite::function(b, "early", body);
        let argument = Switching::ex(b).real(1.0);
        let call = Switching::ex(b).call(early, vec![argument]);
        (vec![], vec![b.stmt(Stmt::Expr(call))])
    });
    assert_eq!(returned, ["return"]);
    let through_a_value = Composite::refused(|b| {
        let read = Switching::ex(b).local(0);
        let identity = Composite::function(b, "identity", read);
        let reference = b.expr(Expr::FunctionRef(identity), ValueType::Fn(vec![ValueType::F64], Box::new(ValueType::F64)), Switching::at());
        let argument = Switching::ex(b).real(1.0);
        let call = b.expr(Expr::CallValue { callee: reference, args: vec![argument] }, ValueType::F64, Switching::at());
        (vec![], vec![b.stmt(Stmt::Expr(call))])
    });
    assert_eq!(through_a_value, ["a function value", "a call through a function value"]);

    // `let t = (1.0, 2.0); let a = [t.0; 2]; a[0]; a.len()` and the struct and variant
    // expressions (T31): each form is named, in the order of the list.
    let aggregates = Composite::refused(|b| {
        let (one, two) = (Switching::ex(b).real(1.0), Switching::ex(b).real(2.0));
        let pair = ValueType::Tuple(vec![ValueType::F64, ValueType::F64]);
        let tuple = b.expr(Expr::Tuple(vec![one, two]), pair, Switching::at());
        let field = b.expr(Expr::Field { expr: tuple, index: 0 }, ValueType::F64, Switching::at());
        let array_type = ValueType::Array(Box::new(ValueType::F64), 2);
        let repeated = b.expr(Expr::ArrayRepeat { element: field, count: 2 }, array_type.clone(), Switching::at());
        let listed = b.expr(Expr::Array(vec![one, two]), array_type, Switching::at());
        let zero = Dx::new(b, "composite.msl", 2, 5).int(0, Ty::u(8));
        let element = b.expr(Expr::Index { expr: listed, index: zero }, ValueType::F64, Switching::at());
        let length = b.expr(Expr::Length(repeated), ValueType::UInt(64), Switching::at());
        let shape = b.type_data(TypeData {
            name: "S".into(),
            path: "S".into(),
            origin: Switching::at(),
            kind: TypeKind::Struct { form: FieldsForm::Tuple, fields: vec![("0".into(), ValueType::F64)] },
            derives: Derives::default(),
        });
        let built = b.expr(Expr::StructOf { ty: shape, fields: vec![element] }, ValueType::Struct(shape), Switching::at());
        let choice = b.type_data(TypeData {
            name: "E".into(),
            path: "E".into(),
            origin: Switching::at(),
            kind: TypeKind::Enum { tag_levels: 1, variants: vec![VariantData { name: "A".into(), discriminant: 0, form: FieldsForm::Unit, fields: vec![] }] },
            derives: Derives::default(),
        });
        let variant = b.expr(Expr::VariantOf { ty: choice, variant: 0, fields: vec![] }, ValueType::Enum(choice), Switching::at());
        (vec![], [length, built, variant].into_iter().map(|e| b.stmt(Stmt::Expr(e))).collect())
    });
    assert_eq!(
        aggregates,
        ["a tuple", "an array", "an array repeat", "a struct expression", "a variant expression", "a field read", "an element read", "a length read"]
    );

    // `matches!(e, E::A)`, `e.0` of a variant, an option from its pair and back, `default` (T32)
    let variants = Composite::refused(|b| {
        let (yes, one) = (Switching::ex(b).boolean(true), Switching::ex(b).real(1.0));
        let choice = b.type_data(TypeData {
            name: "E".into(),
            path: "E".into(),
            origin: Switching::at(),
            kind: TypeKind::Enum { tag_levels: 1, variants: vec![VariantData { name: "A".into(), discriminant: 0, form: FieldsForm::Tuple, fields: vec![("0".into(), ValueType::F64)] }] },
            derives: Derives::default(),
        });
        let built = b.expr(Expr::VariantOf { ty: choice, variant: 0, fields: vec![one] }, ValueType::Enum(choice), Switching::at());
        let test = b.expr(Expr::IsVariant { expr: built, variant: 0 }, ValueType::Bool, Switching::at());
        let payload = b.expr(Expr::VariantField { expr: built, variant: 0, field: 0 }, ValueType::F64, Switching::at());
        let packed = b.expr(Expr::OptionPack { is: yes, value: one }, ValueType::Option(Box::new(ValueType::F64)), Switching::at());
        let part = b.expr(Expr::OptionPart { option: packed, part: piperine_ir::LevelPart::Is }, ValueType::Bool, Switching::at());
        let fresh = b.expr(Expr::Default(ValueType::F64), ValueType::F64, Switching::at());
        (vec![], [test, payload, part, fresh].into_iter().map(|e| b.stmt(Stmt::Expr(e))).collect())
    });
    assert_eq!(variants, ["a variant expression", "a variant test", "a variant field read", "an option pack", "an option part", "a default value"]);
}

#[test]
fn a_host_source_body_is_rejected_by_name() {
    // PI-07, design R2: the host source reads the clock, which the frozen interpreter does not
    // run; it is refused naming the body and the form, never run wrongly
    let mut b = IrBuilder::new();
    Amended::host_source(&mut b);
    let top_body = b.body(Buck::spec("Top", "top.msl", 1));
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: Origin { file: "top.msl".into(), line: 1, col: 1 } });
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the design itself is well formed");
    let Err(rejected) = Interpreter::new(Arc::new(ir)) else { panic!("a host source was accepted") };
    let named: Vec<&str> = rejected.entries.iter().map(|e| e.what.split_once(": ").map_or(e.what.as_str(), |(head, _)| head)).collect();
    assert_eq!(named, ["body HostSource holds now()"]);
}
