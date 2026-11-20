//! Shared by the targets that build one body `Shape(p, n)` by hand: the harness that wraps what a
//! test writes (its locals, its statements, its contributed value) into a whole design, and the
//! filter over a failed check's entries.

use super::devices::Ex;
use super::digital::{Buck, Dx};
use piperine_ir::{Addend, BodyNode, BodySpec, Branch, BranchKind, Broken, Deps, Env, Expr, ExprId, FunctionId, FunctionSpec, InstanceSpec, Ir, IrBuilder, Local, NodeRef, NodeRole, Operator, Origin, QuantityRole, Place, Stmt, StmtId, Target, Terminal, ValueType, BinaryOp, Constant, Param};

pub const FILE: &str = "control.msl";

/// What a test writes into the body `Shape(p, n)` that `Shape` builds: its locals, the
/// statements of its `analog` block before the contribution, and the contributed value.
pub struct Written {
    pub locals: Vec<Local>,
    pub stmts: Vec<StmtId>,
    pub value: ExprId,
}

/// A body `Shape(p, n)` alone as the design: it contributes `value` to the flow of `(p, n)` after
/// the statements, so everything a test writes is reached from the `analog` block.
pub struct Shape;

impl Shape {
    pub fn origin() -> Origin {
        Origin { file: FILE.into(), line: 1, col: 1 }
    }

    /// A local the source declared `let mut`.
    pub fn mutable(name: &str, ty: ValueType) -> Local {
        Local { name: name.into(), ty, mutable: true }
    }

    /// A local the source declared `let`.
    pub fn local(name: &str, ty: ValueType) -> Local {
        Local { name: name.into(), ty, mutable: false }
    }

    /// The expression helper at line `line`.
    pub fn dx(b: &mut IrBuilder, line: u32) -> Dx<'_> {
        Dx::new(b, FILE, line, 5)
    }

    /// `local = value;`, or `local op= value;`.
    pub fn assign(b: &mut IrBuilder, local: u32, op: Option<BinaryOp>, value: ExprId) -> StmtId {
        b.stmt(Stmt::Assign { place: Place { local, path: vec![] }, op, value })
    }

    /// An expression of kind `kind` and type `ty`.
    pub fn expr(b: &mut IrBuilder, kind: Expr, ty: ValueType, line: u32) -> ExprId {
        b.expr(kind, ty, Origin { file: FILE.into(), line, col: 5 })
    }

    /// The design: `write` builds the locals, the statements and the value; functions are added
    /// to the builder by `write` itself.
    pub fn ir(write: impl FnOnce(&mut IrBuilder) -> Written) -> Ir {
        Shape::ir_with_params(|b| (write(b), vec![]))
    }

    /// `ir` for a body that has params: `write` also returns them (they may name constants it adds
    /// to the builder), and the top instance binds each to its default (`Constant::Bool(false)`
    /// when it has none).
    pub fn ir_with_params(write: impl FnOnce(&mut IrBuilder) -> (Written, Vec<Param>)) -> Ir {
        let mut b = IrBuilder::new();
        let electrical = Buck::electrical(&mut b);
        let (Written { locals, mut stmts, value }, params) = write(&mut b);
        stmts.push(b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] }));
        let port = |name: &str| BodyNode { name: name.into(), nature: electrical, role: NodeRole::Port, init: None, origin: Shape::origin() };
        let body = b.body(BodySpec {
            name: "Shape".into(),
            origin: Shape::origin(),
            params: params.clone(),
            nodes: vec![port("p"), port("n")],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals,
            analog: Some(stmts),
            children: vec![],
        });
        let top = b.instance(InstanceSpec { path: "Shape".into(), name: "Shape".into(), body, parent: None, nodes: vec![NodeRef::Ground, NodeRef::Ground], params: params.iter().map(|p| p.default.unwrap_or(Constant::Bool(false))).collect(), origin: Shape::origin() });
        b.finish(top)
    }

    /// `temperature()`.
    pub fn temperature(b: &mut IrBuilder, line: u32) -> ExprId {
        Ex::new(b, FILE, line, 5).env(Env::Temperature)
    }

    /// `v(p, n)`.
    pub fn probe(b: &mut IrBuilder, line: u32) -> ExprId {
        Shape::dx(b, line).probe(Terminal::Node(0), Terminal::Node(1))
    }

    /// The flow of the one branch of `Shape`: what its contribution reads.
    pub fn reads(ir: &Ir) -> Deps {
        ir.instance_at("Shape").expect("placed").body().dependencies(Target::Flow(0))
    }

    /// A function `name(x: f64) -> f64` whose body is `body`, over the local `x`.
    pub fn function(b: &mut IrBuilder, name: &str, body: ExprId) -> FunctionId {
        let x = Shape::local("x", ValueType::F64);
        b.function(FunctionSpec { name: name.into(), origin: Origin { file: FILE.into(), line: 9, col: 1 }, params: vec![x.clone()], ret: ValueType::F64, locals: vec![x], body })
    }
}

/// The entries of a failed check that mention a phrase.
pub struct Entries;

impl Entries {
    pub fn with<'b>(broken: &'b [Broken], phrase: &str) -> Vec<&'b Broken> {
        broken.iter().filter(|entry| entry.what.contains(phrase)).collect()
    }
}

