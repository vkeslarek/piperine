//! Common-subexpression bookkeeping for analog emission: the CSE key space,
//! [`SimCtx`](crate::emit::SimCtx) field offsets, load-bank/op tags, and
//! structural equality for POM `Expr` (`piperine_vams::ast::Expr`, used for
//! `$limit` slot deduplication).

use piperine_vams::ast::Expr;
use piperine_vams::lexer::NumberLit;

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
            crate::resolve::BinOp::BitXnor => 19,
        }
}

/// Structural equality for POM `Expr` (which doesn't derive `PartialEq`...
/// actually `piperine_vams::ast::Expr` *does* derive `PartialEq`, but this
/// helper predates that and stays as the one place `$limit` slot lookup
/// goes through, kept for minimal-diff continuity).
/// Used for `$limit` slot deduplication.
pub fn expr_structural_eq(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Number { value: va, .. }, Expr::Number { value: vb, .. }) => number_eq(va, vb),
        (Expr::Str { value: va, .. }, Expr::Str { value: vb, .. }) => va == vb,
        (Expr::Ident { name: na, .. }, Expr::Ident { name: nb, .. }) => na == nb,
        (
            Expr::SystemFunctionCall { name: na, args: aa, .. },
            Expr::SystemFunctionCall { name: nb, args: ab, .. },
        ) => {
            na == nb && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (Expr::Call { name: na, args: aa, .. }, Expr::Call { name: nb, args: ab, .. }) => {
            na == nb && aa.len() == ab.len()
                && aa.iter().zip(ab).all(|(x, y)| expr_structural_eq(x, y))
        }
        (
            Expr::AccessFunction { function: fa, pos: pa, neg: na, .. },
            Expr::AccessFunction { function: fb, pos: pb, neg: nb, .. },
        ) => {
            fa == fb
                && expr_structural_eq(pa, pb)
                && match (na, nb) {
                    (Some(x), Some(y)) => expr_structural_eq(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Expr::Unary { op: oa, operand: xa, .. }, Expr::Unary { op: ob, operand: xb, .. }) => {
            oa == ob && expr_structural_eq(xa, xb)
        }
        (
            Expr::Binary { op: oa, lhs: la, rhs: ra, .. },
            Expr::Binary { op: ob, lhs: lb, rhs: rb, .. },
        ) => oa == ob && expr_structural_eq(la, lb) && expr_structural_eq(ra, rb),
        (
            Expr::Ternary { cond: ca, then_branch: ta, else_branch: ea, .. },
            Expr::Ternary { cond: cb, then_branch: tb, else_branch: eb, .. },
        ) => expr_structural_eq(ca, cb) && expr_structural_eq(ta, tb) && expr_structural_eq(ea, eb),
        (Expr::Index { base: ba, index: ia, .. }, Expr::Index { base: bb, index: ib, .. }) => {
            expr_structural_eq(ba, bb) && expr_structural_eq(ia, ib)
        }
        (
            Expr::PartSelect { base: ba, msb: ma, lsb: la, .. },
            Expr::PartSelect { base: bb, msb: mb, lsb: lb, .. },
        ) => expr_structural_eq(ba, bb) && expr_structural_eq(ma, mb) && expr_structural_eq(la, lb),
        (Expr::Concat { parts: pa, .. }, Expr::Concat { parts: pb, .. }) => {
            pa.len() == pb.len() && pa.iter().zip(pb).all(|(x, y)| expr_structural_eq(x, y))
        }
        (
            Expr::Replication { count: ca, parts: pa, .. },
            Expr::Replication { count: cb, parts: pb, .. },
        ) => {
            expr_structural_eq(ca, cb)
                && pa.len() == pb.len()
                && pa.iter().zip(pb).all(|(x, y)| expr_structural_eq(x, y))
        }
        _ => false,
    }
}

fn number_eq(a: &NumberLit, b: &NumberLit) -> bool {
    match (a, b) {
        (NumberLit::Real(x), NumberLit::Real(y)) => x == y,
        (NumberLit::Integer(x), NumberLit::Integer(y)) => x == y,
        (NumberLit::Based(x), NumberLit::Based(y)) => {
            x.size == y.size && x.signed == y.signed && x.base == y.base && x.digits == y.digits
        }
        _ => false,
    }
}
