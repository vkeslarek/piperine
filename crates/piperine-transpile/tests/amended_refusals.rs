//! Scope: the forms the IR amendments of msl-digital added, before the printer lowers them
//! (T39 onward): a body that holds one is `Error::Unsupported` naming the form, the body and
//! the origin, and no text is printed. Each lowering task deletes the case it lowers.

use std::sync::Arc;

use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Constant, EmitField, Env, Event, EventSource, Expr, FunctionSpec,
    InstanceSpec, Ir, IrBuilder, NetSpec, NodeRef, NodeRole, NodeSpec, Operator, QuantityRole, Stmt, Stream, StreamField, Terminal, ValueType,
};
use piperine_transpile::{Error, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use digital::{Buck, Dx};

const F: &str = "amended.msl";

/// A one-body design `Amended(p, n)` whose `analog` block is `contributions`, built by `build`.
struct Case;

impl Case {
    fn design(build: impl FnOnce(&mut IrBuilder) -> Vec<Stmt>) -> Ir {
        let mut b = IrBuilder::new();
        let el = Buck::electrical(&mut b);
        let stmts: Vec<Stmt> = build(&mut b);
        let analog = stmts.into_iter().map(|s| b.stmt(s)).collect();
        let port = |name: &str, col| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
        let body = b.body(BodySpec {
            nodes: vec![port("p", 10), port("n", 18)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(analog),
            ..Buck::spec("Amended", F, 1)
        });
        let at = b.next_instance_id();
        let nodes = ["p", "n"]
            .map(|n| NodeRef::Node(b.node(NodeSpec { path: format!("Top.{n}"), nature: el, declared_by: at, init: None, origin: Buck::origin(F, 1, 1) })))
            .to_vec();
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes, params: vec![], origin: Buck::origin(F, 1, 1) });
        b.finish(top)
    }

    /// `i(p, n) +> <value>;`
    fn flow(value: piperine_ir::ExprId) -> Stmt {
        Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] }
    }

    /// What printing `ir` gives: the refusal.
    fn refused(ir: Ir) -> Error {
        assert_eq!(ir.check(), vec![]);
        let transpiler = Transpiler::new(Arc::new(ir)).expect("the design passes check()");
        match transpiler.source() {
            Err(e) => e,
            Ok(text) => panic!("a form with no lowering printed:\n{text}"),
        }
    }
}

#[test]
fn time_and_static_are_refused_by_name() {
    // `i(p, n) +> v(p, n) * now();` at line 2, and `i(p, n) +> keep(analysis(Static));` through a
    // function at line 4.
    let ir = Case::design(|b| {
        let mut x = Dx::new(b, F, 2, 20);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o);
        let value = x.bin(BinaryOp::Mul, v, now, ValueType::F64);
        vec![Case::flow(value)]
    });
    assert_eq!(
        Case::refused(ir),
        Error::Unsupported { form: "now()".into(), path: "body Amended e1".into(), origin: Buck::origin(F, 2, 20) }
    );

    let ir = Case::design(|b| {
        let mut x = Dx::new(b, F, 3, 5);
        let o = x.origin();
        let stat = x.b.expr(Expr::Env(Env::Static), ValueType::Bool, o);
        let one = x.real(1.0);
        let zero = x.real(0.0);
        let level = x.if_(stat, one, zero, ValueType::F64);
        let keep = b.function(FunctionSpec { name: "keep".into(), origin: Buck::origin(F, 3, 1), params: vec![], ret: ValueType::F64, locals: vec![], body: level });
        let mut x = Dx::new(b, F, 4, 20);
        let call = x.call(keep, vec![], ValueType::F64);
        vec![Case::flow(call)]
    });
    assert_eq!(
        Case::refused(ir),
        Error::Unsupported { form: "analysis(Static)".into(), path: "body Amended e0".into(), origin: Buck::origin(F, 3, 5) }
    );
}

#[test]
fn an_at_event_is_refused_by_name() {
    // `module Timer(out q: Bit = false) {}` and `digital for Timer { on at(1.0) { q <- true; } }`.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Timer", F, 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 20));
    let time = Dx::new(&mut b, F, 2, 16).real(1.0);
    let at = b.event(body, Event { source: EventSource::At(time), guard: None, origin: Buck::origin(F, 2, 8) });
    let mut x = Dx::new(&mut b, F, 2, 24);
    let yes = x.boolean(true);
    let commit = x.commit(0, yes, None);
    Buck::reaction(&mut b, body, vec![at], vec![commit], vec![], F, 2);
    let at = b.next_instance_id();
    let net = b.net(NetSpec { path: "Top.q".into(), ty: ValueType::Bool, declared_by: at, init: None, origin: Buck::origin(F, 1, 20) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![], origin: Buck::origin(F, 1, 1) });
    b.bind_nets(top, vec![net]);
    assert_eq!(
        Case::refused(b.finish(top)),
        Error::Unsupported { form: "at(t)".into(), path: "body Timer event 0".into(), origin: Buck::origin(F, 2, 8) }
    );
}

#[test]
fn an_emit_site_is_refused_by_name() {
    // `on q.changed() { emit!("sim", finish = true); }` at line 3, the second statement after a
    // commit at line 2.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Monitor", F, 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 20));
    let sim = b.stream(body, Stream { name: "sim".into(), fields: vec![StreamField { name: "finish".into(), ty: ValueType::Bool }] });
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 2, 8) });
    let mut x = Dx::new(&mut b, F, 2, 24);
    let no = x.boolean(false);
    let commit = x.commit(0, no, None);
    x.at(3, 40);
    let stop = x.boolean(true);
    let emit = x.b.stmt(Stmt::Emit { stream: sim, fields: vec![EmitField::Value { field: 0, value: stop }] });
    Buck::reaction(&mut b, body, vec![changed], vec![commit, emit], vec![], F, 2);
    let at = b.next_instance_id();
    let net = b.net(NetSpec { path: "Top.q".into(), ty: ValueType::Bool, declared_by: at, init: None, origin: Buck::origin(F, 1, 20) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![], origin: Buck::origin(F, 1, 1) });
    b.bind_nets(top, vec![net]);
    assert_eq!(
        Case::refused(b.finish(top)),
        Error::Unsupported { form: "emit!".into(), path: "body Monitor reaction 0 s1".into(), origin: Buck::origin(F, 3, 40) }
    );
}
