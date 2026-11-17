//! The interpreter is frozen at the M1 analog subset (R1, spec RS-09 AC 6): an `Ir` that holds a
//! body with a carrier, a drive or a reaction is `Rejected` by `Interpreter::new`, one entry
//! naming each such body; an analog body beside them is not named. So is an `Ir` that holds a
//! form of the msl-digital amendments (the clock, the analysis, an `at` event, an emit site, a
//! branch flow, a previous value): one entry per body and form, by name.

use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{
    Addend, AnalysisKind, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, EmitField, Env, Event, EventSource, Expr, InstanceSpec, IrBuilder,
    NodeRole, Operator, Origin, QuantityRole, Stmt, Stream, StreamField, Terminal, ValueType,
};
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

use devices::Devices;
use digital::{Buck, Dx, Ty};
use models::{In, Models, View};

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
            let cap = b.stmt(Stmt::BoundStep { site: 0, value: step });
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
