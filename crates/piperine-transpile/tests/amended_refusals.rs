//! Scope: the forms the IR amendments of msl-digital added, before the printer lowers them
//! (T40 onward, the timers' since T42, the emit sites' since T44): a body that holds one is `Error::Unsupported` naming the form, the body and
//! the origin, and no text is printed. Each lowering task deletes the case it lowers.

use std::sync::Arc;

use piperine_ir::{
    Addend, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Constant, Event, EventSource, Expr,
    InstanceSpec, Ir, IrBuilder, NetSpec, NodeRef, NodeRole, NodeSpec, Operator, QuantityRole, Stmt, Terminal, ValueType,
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
    fn design(kind: BranchKind, build: impl FnOnce(&mut IrBuilder) -> Vec<Stmt>) -> Ir {
        let mut b = IrBuilder::new();
        let el = Buck::electrical(&mut b);
        let stmts: Vec<Stmt> = build(&mut b);
        let analog = stmts.into_iter().map(|s| b.stmt(s)).collect();
        let port = |name: &str, col| BodyNode { name: name.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
        let body = b.body(BodySpec {
            nodes: vec![port("p", 10), port("n", 18)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind }],
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
fn the_flow_of_a_potential_branch_is_refused_by_name() {
    // `v(p, n) +> ddt(i(p, n));` at line 2: a potential contribution reading its own flow.
    let ir = Case::design(BranchKind::Potential, |b| {
        let x = Dx::new(b, F, 2, 24);
        let o = x.origin();
        let flow = x.b.expr(Expr::Flow { branch: 0 }, ValueType::F64, o);
        vec![Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Potential,
            addends: vec![Addend { operator: Operator::Ddt, negated: false, value: flow }],
        }]
    });
    assert_eq!(
        Case::refused(ir),
        Error::Unsupported { form: "the flow of a potential branch".into(), path: "body Amended e0".into(), origin: Buck::origin(F, 2, 24) }
    );
}

#[test]
fn the_previous_value_of_a_carrier_is_refused_by_name() {
    // `on q.changed() where q.prev().is_known() { q <- true; }`, the guard at line 2.
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Edge", F, 1));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 20));
    let mut x = Dx::new(&mut b, F, 2, 24);
    let o = x.origin();
    let prev = x.b.expr(Expr::Previous(0), ValueType::Flagged(Box::new(ValueType::Bool)), o);
    let known = x.is_known(prev);
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: Some(known), origin: Buck::origin(F, 2, 8) });
    let mut x = Dx::new(&mut b, F, 2, 50);
    let yes = x.boolean(true);
    let commit = x.commit(0, yes, None);
    Buck::reaction(&mut b, body, vec![changed], vec![commit], vec![], F, 2);
    let at = b.next_instance_id();
    let net = b.net(NetSpec { path: "Top.q".into(), ty: ValueType::Bool, declared_by: at, init: None, origin: Buck::origin(F, 1, 20) });
    let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![], origin: Buck::origin(F, 1, 1) });
    b.bind_nets(top, vec![net]);
    assert_eq!(
        Case::refused(b.finish(top)),
        Error::Unsupported { form: "the previous value of a carrier".into(), path: "body Edge e0".into(), origin: Buck::origin(F, 2, 24) }
    );
}
