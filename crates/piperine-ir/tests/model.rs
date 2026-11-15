//! Scope: the storage of `piperine-ir` (IR-01), the node set (IR-07), the handles (IR-02) and the
//! body and instance tables (IR-08), all on hand-built IRs.

use std::mem::size_of;

use piperine_ir::{
    BodyId, BodySpec, ExprId, FunctionId, InstanceId, InstanceSpec, Ir, IrBuilder, NatureId,
    NodeId, NodeSpec, Origin, Quantity, Real, StmtId,
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
    // A `&mut self` method may exist only inside an `impl` of the builder.
    for (path, text) in Sources::all() {
        let mut owner_is_builder = false;
        for (n, line) in text.lines().enumerate() {
            if line.starts_with("impl") {
                owner_is_builder = line.contains("IrBuilder");
            }
            if line.contains("&mut self") && !owner_is_builder {
                panic!("{path}:{}: a mutating method outside IrBuilder: {line}", n + 1);
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
}
