//! Scope: the storage of `piperine-ir` (IR-01), the node set (IR-07), the handles (IR-02) and the
//! body and instance tables (IR-08), all on hand-built IRs.

use std::mem::size_of;

use piperine_ir::{BodyId, ExprId, FunctionId, InstanceId, Ir, NatureId, NodeId, StmtId};

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
