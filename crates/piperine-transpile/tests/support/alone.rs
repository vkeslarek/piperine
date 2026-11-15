//! One body, or the whole buck slice, placed in a design as the IR's own tests place them: under
//! an empty `Top` body, so `check()` has a top instance to hold.

use piperine_ir::{BodyId, BodySpec, InstanceSpec, Ir, IrBuilder, Origin};

/// Designs of hand-built bodies.
pub struct Alone;

impl Alone {
    fn origin() -> Origin {
        Origin { file: "top.msl".to_string(), line: 1, col: 1 }
    }

    /// The design holding the body `make` builds, which must pass `check()`.
    pub fn design(make: impl FnOnce(&mut IrBuilder) -> BodyId) -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let ir = Self::finish(b);
        (ir, body)
    }

    /// The builder's bodies under an empty top body, checked.
    pub fn finish(b: IrBuilder) -> Ir {
        let ir = Self::finish_unchecked(b);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }

    /// The same, not checked: for an IR a printer must refuse on its own.
    pub fn finish_unchecked(mut b: IrBuilder) -> Ir {
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: Self::origin(),
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
            origin: Self::origin(),
        });
        b.finish(top)
    }
}
