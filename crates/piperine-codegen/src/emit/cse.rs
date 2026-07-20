//! Common-subexpression bookkeeping for analog emission: the CSE key space,
//! [`SimCtx`](crate::emit::SimCtx) field offsets, load-bank/op tags, and
//! structural equality for POM `Expr` (used for `$limit` slot deduplication).

use piperine_lang::parse::ast::Expr;

// ─── Analog CSE infrastructure (copied from jit/emit.rs) ──────────────────────

/// Structural key for common-subexpression elimination in analog emission.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum CseKey {
    /// f64/const bit pattern.
    Const(u64),
    /// A load: `(bank tag, byte offset)`.
    Load(u8, i32),
    /// Unary op `(tag, child)`.
    Op1(u8, u32),
    /// Binary op / comparison `(tag, lhs, rhs)`.
    Op2(u8, u32, u32),
    /// Ternary (select) `(tag, a, b, c)`.
    Op3(u8, u32, u32, u32),
    /// Math builtin call `(name, args)`.
    Call(&'static str, Vec<u32>),
    /// Voltage-limited value for `$limit` slot `i`.
    Limit(u32),
}

/// Byte offsets of [`SimCtx`](crate::emit::SimCtx) fields, as read by JIT code.
pub(crate) struct SimField;

impl SimField {
    pub(crate) const TEMPERATURE: i32 = 0;
    pub(crate) const ABSTIME: i32 = 8;
    pub(crate) const MFACTOR: i32 = 16;
    pub(crate) const GMIN: i32 = 24;
    pub(crate) const STEP: i32 = 32;
    pub(crate) const TFINAL: i32 = 40;
    pub(crate) const PARAM_GIVEN_MASK: i32 = 48;
    pub(crate) const CURRENT_ANALYSIS: i32 = 56;
    // FREQUENCY (offset 64) is consumed Rust-side only (noise PSD scaling).
    pub(crate) const SRCFACT: i32 = 72;
}

// Load-bank tags.
pub(crate) const BANK_STATE: u8 = 0;
pub(crate) const BANK_VARS: u8 = 1;
pub(crate) const BANK_SIM: u8 = 2;
// Op tags (namespaced across unary/binary/select/cmp).
pub(crate) const T_NEG: u8 = 0;
pub(crate) const T_SELECT: u8 = 1;
pub(crate) const T_NOT: u8 = 2;
pub(crate) const T_FCMP_BASE: u8 = 16;
pub(crate) const T_BIN_BASE: u8 = 40;

/// Distinct CSE tag per binary op (offset past the fcmp/select tags).
pub(crate) fn bin_tag(op: crate::resolve::BinOp) -> u8 {
    T_BIN_BASE
        + match op {
            crate::resolve::BinOp::Add => 0,
            crate::resolve::BinOp::Sub => 1,
            crate::resolve::BinOp::Mul => 2,
            crate::resolve::BinOp::Div => 3,
            crate::resolve::BinOp::Rem => 4,
            crate::resolve::BinOp::Eq => 5,
            crate::resolve::BinOp::Ne => 6,
            crate::resolve::BinOp::Lt => 7,
            crate::resolve::BinOp::Le => 8,
            crate::resolve::BinOp::Gt => 9,
            crate::resolve::BinOp::Ge => 10,
            crate::resolve::BinOp::And => 11,
            crate::resolve::BinOp::Or => 12,
            crate::resolve::BinOp::Pow => 13,
            crate::resolve::BinOp::BitAnd => 14,
            crate::resolve::BinOp::BitOr => 15,
            crate::resolve::BinOp::BitXor => 16,
            crate::resolve::BinOp::Shl => 17,
            crate::resolve::BinOp::Shr => 18,
        }
}

/// Structural equality for POM `Expr` (which doesn't derive `PartialEq`).
/// Used for `$limit` slot deduplication.
pub fn expr_structural_eq(a: &Expr, b: &Expr) -> bool {
    use piperine_lang::parse::ast::Literal;
    match (a, b) {
        (Expr::Literal(la), Expr::Literal(lb)) => match (la, lb) {
            (Literal::Real(x), Literal::Real(y)) => x == y,
            (Literal::Int(x), Literal::Int(y)) => x == y,
            (Literal::Bool(x), Literal::Bool(y)) => x == y,
            (Literal::String(x), Literal::String(y)) => x == y,
            (Literal::Quad(x), Literal::Quad(y)) => x == y,
            (Literal::None, Literal::None) => true,
            _ => false,
        },
        (Expr::Ident(x), Expr::Ident(y)) => x == y,
        (Expr::Path(x), Expr::Path(y)) => x.segments == y.segments,
        (Expr::SysCall(na, aa), Expr::SysCall(nb, ab)) => {
            na == nb && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (Expr::Call(fa, aa), Expr::Call(fb, ab)) => {
            expr_structural_eq(fa, fb) && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (Expr::Unary(oa, xa), Expr::Unary(ob, xb)) => oa == ob && expr_structural_eq(xa, xb),
        (Expr::Binary(la, oa, ra), Expr::Binary(lb, ob, rb)) => {
            oa == ob && expr_structural_eq(la, lb) && expr_structural_eq(ra, rb)
        }
        (Expr::Cast(ta, xa), Expr::Cast(tb, xb)) => ta == tb && expr_structural_eq(xa, xb),
        (Expr::Field(ba, fa), Expr::Field(bb, fb)) => expr_structural_eq(ba, bb) && fa == fb,
        (Expr::Index(ba, ia), Expr::Index(bb, ib)) => expr_structural_eq(ba, bb) && expr_structural_eq(ia, ib),
        (Expr::If { cond: ca, then_body: ta, else_body: ea },
         Expr::If { cond: cb, then_body: tb, else_body: eb }) => {
            expr_structural_eq(ca, cb)
                && blocks_eq(ta, tb)
                && blocks_eq(ea, eb)
        }
        _ => false,
    }
}

fn blocks_eq(a: &piperine_lang::parse::ast::Block, b: &piperine_lang::parse::ast::Block) -> bool {
    a.stmts.len() == b.stmts.len()
        && a.stmts.iter().zip(&b.stmts).all(|(x, y)| stmts_eq(x, y))
        && match (&a.expr, &b.expr) {
            (Some(x), Some(y)) => expr_structural_eq(x, y),
            (None, None) => true,
            _ => false,
        }
}

fn stmts_eq(a: &piperine_lang::parse::ast::Stmt, b: &piperine_lang::parse::ast::Stmt) -> bool {
    use piperine_lang::parse::ast::Stmt as S;
    match (a, b) {
        (S::Bind { dest: da, op: oa, src: sa }, S::Bind { dest: db, op: ob, src: sb }) => {
            oa == ob && expr_structural_eq(da, db) && expr_structural_eq(sa, sb)
        }
        (S::Expr(ea), S::Expr(eb)) => expr_structural_eq(ea, eb),
        (S::VarDecl { name: na, default: da, .. }, S::VarDecl { name: nb, default: db, .. }) => {
            na == nb && match (da, db) {
                (Some(x), Some(y)) => expr_structural_eq(x, y),
                (None, None) => true,
                _ => false,
            }
        }
        _ => false,
    }
}
