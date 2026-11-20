//! Scope: the control forms of the IR (composite-values phase A, CV-01): a mutable local and its
//! assignment, built by hand, checked (valid and refused), printed, and walked by the dependency
//! analysis, where an assignment joins what it reads into the local.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;

use digital::{Buck, Dx};
use piperine_ir::{
    Addend, BinaryOp, BodyNode, BodySpec, Branch, BranchKind, Broken, Deps, ExprId, InstanceSpec, Ir, IrBuilder, Local, NodeRef,
    NodeRole, Operator, Origin, Place, QuantityRole, Stmt, StmtId, Target, Terminal, ValueType,
};

const FILE: &str = "control.msl";

/// What a test writes into the body `Shape(p, n)` that `Shape` builds: its locals, the
/// statements of its `analog` block before the contribution, and the contributed value.
struct Written {
    locals: Vec<Local>,
    stmts: Vec<StmtId>,
    value: ExprId,
}

/// A body `Shape(p, n)` alone as the design: it contributes `value` to the flow of `(p, n)` after
/// the statements, so everything a test writes is reached from the `analog` block.
struct Shape;

impl Shape {
    fn origin() -> Origin {
        Origin { file: FILE.into(), line: 1, col: 1 }
    }

    /// A local the source declared `let mut`.
    fn mutable(name: &str, ty: ValueType) -> Local {
        Local { name: name.into(), ty, mutable: true }
    }

    /// A local the source declared `let`.
    fn local(name: &str, ty: ValueType) -> Local {
        Local { name: name.into(), ty, mutable: false }
    }

    /// The expression helper at line `line`.
    fn dx(b: &mut IrBuilder, line: u32) -> Dx<'_> {
        Dx::new(b, FILE, line, 5)
    }

    /// `local = value;`, or `local op= value;`.
    fn assign(b: &mut IrBuilder, local: u32, op: Option<BinaryOp>, value: ExprId) -> StmtId {
        b.stmt(Stmt::Assign { place: Place { local, path: vec![] }, op, value })
    }

    /// The design: `write` builds the locals, the statements and the value; functions are added
    /// to the builder by `write` itself.
    fn ir(write: impl FnOnce(&mut IrBuilder) -> Written) -> Ir {
        let mut b = IrBuilder::new();
        let electrical = Buck::electrical(&mut b);
        let Written { locals, mut stmts, value } = write(&mut b);
        stmts.push(b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] }));
        let port = |name: &str| BodyNode { name: name.into(), nature: electrical, role: NodeRole::Port, init: None, origin: Shape::origin() };
        let body = b.body(BodySpec {
            name: "Shape".into(),
            origin: Shape::origin(),
            params: vec![],
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals,
            analog: Some(stmts),
            children: vec![],
        });
        let top = b.instance(InstanceSpec { path: "Shape".into(), name: "Shape".into(), body, parent: None, nodes: vec![NodeRef::Ground, NodeRef::Ground], params: vec![], origin: Shape::origin() });
        b.finish(top)
    }

    /// `v(p, n)`.
    fn probe(b: &mut IrBuilder, line: u32) -> ExprId {
        Shape::dx(b, line).probe(Terminal::Node(0), Terminal::Node(1))
    }

    /// The flow of the one branch of `Shape`: what its contribution reads.
    fn reads(ir: &Ir) -> Deps {
        ir.instance_at("Shape").expect("placed").body().dependencies(Target::Flow(0))
    }

}

/// The entries of a failed check that mention a phrase.
struct Entries;

impl Entries {
    fn with<'b>(broken: &'b [Broken], phrase: &str) -> Vec<&'b Broken> {
        broken.iter().filter(|entry| entry.what.contains(phrase)).collect()
    }
}

// ---- T2: mutable locals and assignment (CV-01) -------------------------------------------------------------

#[test]
fn an_assignment_to_a_mutable_local_checks_and_prints() {
    // `let mut x = 1.0; x = 2.0; x += v(p, n);` (§1.5.12), then the contribution reads `x`.
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let two = Shape::dx(b, 3).real(2.0);
        let set = Shape::assign(b, 0, None, two);
        let probe = Shape::probe(b, 4);
        let add = Shape::assign(b, 0, Some(BinaryOp::Add), probe);
        let read = Shape::dx(b, 5).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, set, add], value: read }
    });
    assert_eq!(ir.check(), vec![], "a mutable local is assigned");
    let text = ir.to_string();
    assert!(text.contains("local mut x: f64"), "the table says `mut`:\n{text}");
    assert!(text.contains("x = 2"), "the assignment prints as source:\n{text}");
    assert!(text.contains("x += probe(p, n)"), "the compound assignment prints as source:\n{text}");
    assert!(text.contains("assign 0 - e") && text.contains("assign 0 add e"), "the arena lines show the target and the operator:\n{text}");
}

#[test]
fn an_assignment_to_an_immutable_local_is_refused() {
    // `let x = 1.0; x = 2.0;` (§1.5.12: assignment only to a `let mut`).
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let two = Shape::dx(b, 3).real(2.0);
        let set = Shape::assign(b, 0, None, two);
        let read = Shape::dx(b, 4).local(0, ValueType::F64);
        Written { locals: vec![Shape::local("x", ValueType::F64)], stmts: vec![declare, set], value: read }
    });
    let broken = ir.check();
    let hits = Entries::with(&broken, "not `mutable`");
    assert_eq!(hits.len(), 1, "{broken:?}");
    assert_eq!(hits[0].origin, Origin { file: FILE.into(), line: 3, col: 5 }, "the entry names the assigned value's position");
}

#[test]
fn an_assignment_is_typed_by_its_target() {
    // A value of another type, and a compound operator the target type lacks (`bool += bool`,
    // `f64 <= f64`: a comparison has no compound form), are refused.
    let ir = Shape::ir(|b| {
        let init = Shape::dx(b, 2).real(1.0);
        let declare = Shape::dx(b, 2).let_(0, init);
        let flag = Shape::dx(b, 3).boolean(true);
        let mismatched = Shape::assign(b, 0, None, flag);
        let one = Shape::dx(b, 4).real(1.0);
        let comparison = Shape::assign(b, 0, Some(BinaryOp::Le), one);
        let read = Shape::dx(b, 5).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, mismatched, comparison], value: read }
    });
    let broken = ir.check();
    assert_eq!(Entries::with(&broken, "the value of an assignment must be f64, found bool").len(), 1, "{broken:?}");
    assert_eq!(Entries::with(&broken, "`Le` does not apply").len(), 1, "{broken:?}");
}

#[test]
fn an_assignment_joins_what_it_reads_into_the_local() {
    // `let mut x = 0.0; x = v(p, n);` then the contribution of `x`: the flow reads both nodes,
    // which the `let` alone (a constant) would not say.
    let ir = Shape::ir(|b| {
        let zero = Shape::dx(b, 2).real(0.0);
        let declare = Shape::dx(b, 2).let_(0, zero);
        let probe = Shape::probe(b, 3);
        let set = Shape::assign(b, 0, None, probe);
        let read = Shape::dx(b, 4).local(0, ValueType::F64);
        Written { locals: vec![Shape::mutable("x", ValueType::F64)], stmts: vec![declare, set], value: read }
    });
    assert_eq!(ir.check(), vec![]);
    assert_eq!(Shape::reads(&ir), Deps { nodes: vec![0, 1], ..Deps::default() });
}
