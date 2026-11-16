//! The interpreter is frozen at the M1 analog subset (R1, spec RS-09 AC 6): an `Ir` that holds a
//! body with a carrier, a drive or a reaction is `Rejected` by `Interpreter::new`, one entry
//! naming each such body; an analog body beside them is not named. So is an `Ir` that holds a
//! form of the msl-digital amendments (the clock, the analysis, an `at` event, an emit site, a
//! branch flow, a previous value): one entry per body and form, by name.

use std::sync::Arc;

use piperine_interp::Interpreter;
use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, EmitField, Env, Event, EventSource, Expr, InstanceSpec, IrBuilder,
    NodeRole, Operator, Origin, QuantityRole, Stmt, Stream, StreamField, Terminal, ValueType,
};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use devices::Devices;
use digital::{Buck, Dx};

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
