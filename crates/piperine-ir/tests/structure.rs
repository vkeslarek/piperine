//! Scope: the IR forms of `analog-structure`, built by hand through `IrBuilder`, checked,
//! printed and round-tripped through serde (the printer is one way, `tests/print.rs`): the own
//! terminal of a child's own signal (AS-16).

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::Devices;
use piperine_ir::{
    BodySpec, BodyNode, Branch, BranchKind, Child, InstanceSpec, Ir, IrBuilder, NodeRef, NodeRole, NodeSpec, Origin, Terminal,
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
