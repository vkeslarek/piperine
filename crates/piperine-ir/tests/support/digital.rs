//! The digital and mixed bodies of the buck slice (ir-rust phase B), built by hand through
//! `IrBuilder` from the MSL text each one transcribes, and the expression helper they share.
//! Every expression is given the type the MSL source gives it, written out at each node: the
//! fixture states the types, `Ir::check` judges them. Shared by the tests of `piperine-plan` and
//! `piperine-transpile` through `#[path]`.

use piperine_ir::{
    BinaryOp, Expr, ExprId, FunctionId, IrBuilder, Method, Origin, Real, Stmt, StmtId, UnaryOp, ValueType,
};

/// The value types of the MSL source, by their aliases.
pub struct Ty;

impl Ty {
    pub fn u(n: u32) -> ValueType {
        ValueType::UInt(n)
    }

    pub fn i(n: u32) -> ValueType {
        ValueType::Int(n)
    }
}

/// Expression construction at one source position; every node takes its type from the caller.
pub struct Dx<'b> {
    pub b: &'b mut IrBuilder,
    pub file: &'static str,
    pub line: u32,
    pub col: u32,
}

impl<'b> Dx<'b> {
    pub fn new(b: &'b mut IrBuilder, file: &'static str, line: u32, col: u32) -> Self {
        Dx { b, file, line, col }
    }

    pub fn origin(&self) -> Origin {
        Origin { file: self.file.to_string(), line: self.line, col: self.col }
    }

    pub fn at(&mut self, line: u32, col: u32) -> &mut Self {
        self.line = line;
        self.col = col;
        self
    }

    fn emit(&mut self, kind: Expr, ty: ValueType) -> ExprId {
        let o = self.origin();
        self.b.expr(kind, ty, o)
    }

    pub fn int(&mut self, v: i128, ty: ValueType) -> ExprId {
        self.emit(Expr::Int(v), ty)
    }

    pub fn real(&mut self, v: f64) -> ExprId {
        self.emit(Expr::Real(Real::new(v)), ValueType::F64)
    }

    pub fn boolean(&mut self, v: bool) -> ExprId {
        self.emit(Expr::Bool(v), ValueType::Bool)
    }

    pub fn unit(&mut self) -> ExprId {
        self.emit(Expr::Unit, ValueType::Unit)
    }

    pub fn local(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Local(i), ty)
    }

    pub fn param(&mut self, i: u32, ty: ValueType) -> ExprId {
        self.emit(Expr::Param(i), ty)
    }

    pub fn bin(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Binary { op, lhs, rhs }, ty)
    }

    pub fn un(&mut self, op: UnaryOp, expr: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::Unary { op, expr }, ty)
    }

    pub fn cast(&mut self, expr: ExprId, to: ValueType, ty: ValueType) -> ExprId {
        self.emit(Expr::Cast { expr, to }, ty)
    }

    pub fn method(&mut self, method: Method, receiver: ExprId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Method { method, receiver, args }, ty)
    }

    pub fn call(&mut self, function: FunctionId, args: Vec<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Call { function, args }, ty)
    }

    pub fn if_(&mut self, cond: ExprId, then: ExprId, otherwise: ExprId, ty: ValueType) -> ExprId {
        self.emit(Expr::If { cond, then, otherwise }, ty)
    }

    pub fn block(&mut self, stmts: Vec<StmtId>, value: Option<ExprId>, ty: ValueType) -> ExprId {
        self.emit(Expr::Block { stmts, value }, ty)
    }

    pub fn let_(&mut self, local: u32, init: ExprId) -> StmtId {
        self.b.stmt(Stmt::Let { local, init })
    }

    pub fn stmt(&mut self, e: ExprId) -> StmtId {
        self.b.stmt(Stmt::Expr(e))
    }
}

/// The buck-slice bodies and the functions they call, each beside the MSL text it transcribes.
pub struct Buck;

impl Buck {
    fn local(name: &str, ty: ValueType) -> piperine_ir::Local {
        piperine_ir::Local { name: name.to_string(), ty }
    }

    /// `dpwm.msl:14`: `fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }`. A shift
    /// amount literal is `i32`, Rust's fallback for an unconstrained shift operand.
    pub fn rev3(b: &mut IrBuilder) -> FunctionId {
        let mut x = Dx::new(b, "dpwm.msl", 14, 24);
        let k = x.local(0, Ty::u(3));
        let one = x.int(1, Ty::u(3));
        let k_and_1 = x.bin(BinaryOp::BitAnd, k, one, Ty::u(3));
        let two = x.int(2, Ty::i(32));
        let shifted = x.bin(BinaryOp::Shl, k_and_1, two, Ty::u(3));
        let k = x.local(0, Ty::u(3));
        let two_mask = x.int(2, Ty::u(3));
        let middle = x.bin(BinaryOp::BitAnd, k, two_mask, Ty::u(3));
        let high_low = x.bin(BinaryOp::BitOr, shifted, middle, Ty::u(3));
        let k = x.local(0, Ty::u(3));
        let two = x.int(2, Ty::i(32));
        let down = x.bin(BinaryOp::Shr, k, two, Ty::u(3));
        let one = x.int(1, Ty::u(3));
        let low = x.bin(BinaryOp::BitAnd, down, one, Ty::u(3));
        let body = x.bin(BinaryOp::BitOr, high_low, low, Ty::u(3));
        let origin = Origin { file: "dpwm.msl".into(), line: 14, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: "rev3".into(),
            origin,
            params: vec![Buck::local("k", Ty::u(3))],
            ret: Ty::u(3),
            locals: vec![Buck::local("k", Ty::u(3))],
            body,
        })
    }

    /// `dpwm.msl:18-22`:
    ///
    /// ```text
    /// fn dithered_width(duty: u12, frame: u3) -> u9 {
    ///     let coarse = (duty >> 3) as u9;
    ///     let fine = (duty & 7) as u3;
    ///     if rev3(frame) < fine { coarse + 1 } else { coarse }
    /// }
    /// ```
    pub fn dithered_width(b: &mut IrBuilder, rev3: FunctionId) -> FunctionId {
        let mut x = Dx::new(b, "dpwm.msl", 19, 18);
        let duty = x.local(0, Ty::u(12));
        let three = x.int(3, Ty::i(32));
        let coarse = x.bin(BinaryOp::Shr, duty, three, Ty::u(12));
        let coarse = x.cast(coarse, Ty::u(9), Ty::u(9));
        let let_coarse = x.let_(2, coarse);
        x.at(20, 16);
        let duty = x.local(0, Ty::u(12));
        let seven = x.int(7, Ty::u(12));
        let fine = x.bin(BinaryOp::BitAnd, duty, seven, Ty::u(12));
        let fine = x.cast(fine, Ty::u(3), Ty::u(3));
        let let_fine = x.let_(3, fine);
        x.at(21, 5);
        let frame = x.local(1, Ty::u(3));
        let rev = x.call(rev3, vec![frame], Ty::u(3));
        let fine = x.local(3, Ty::u(3));
        let cond = x.bin(BinaryOp::Lt, rev, fine, ValueType::Bool);
        let coarse = x.local(2, Ty::u(9));
        let one = x.int(1, Ty::u(9));
        let wider = x.bin(BinaryOp::Add, coarse, one, Ty::u(9));
        let coarse = x.local(2, Ty::u(9));
        let width = x.if_(cond, wider, coarse, Ty::u(9));
        x.at(18, 52);
        let body = x.block(vec![let_coarse, let_fine], Some(width), Ty::u(9));
        let origin = Origin { file: "dpwm.msl".into(), line: 18, col: 1 };
        b.function(piperine_ir::FunctionSpec {
            name: "dithered_width".into(),
            origin,
            params: vec![Buck::local("duty", Ty::u(12)), Buck::local("frame", Ty::u(3))],
            ret: Ty::u(9),
            locals: vec![
                Buck::local("duty", Ty::u(12)),
                Buck::local("frame", Ty::u(3)),
                Buck::local("coarse", Ty::u(9)),
                Buck::local("fine", Ty::u(3)),
            ],
            body,
        })
    }
}
