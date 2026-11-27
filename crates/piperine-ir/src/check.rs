//! `Ir::check`: the invariants a correct builder sequence guarantees, verified over the arenas.
//! The digital forms add their own: a body writes only the carriers it may (never an `in`
//! port), a carrier has a drive or commits, never both, a reaction waits on events of its body, a commit
//! sits in a reaction, a `cross` watches a real, and a carrier's initial value is a value of its
//! type (an integer within its `N`-bit range). A contribution may stand anywhere inside the
//! `analog` block (under an `if`, in a nested block) but in a closure, a function or a
//! reaction; a flow or potential branch takes its one quantity, a switch branch both.
//! Every id is in range, instances agree with their bodies, paths are unique, `limit` and
//! `ac_stim` sites sit unconditionally at the top of the `analog` block and are used once, and
//! every expression has the type its position requires, every local is bound before it is read,
//! no function reaches itself through calls, and no expression reaches itself. It lists what is
//! broken and never panics.
//!
//! Three checks run in turn, each reporting into one list, so the order of the findings is the
//! order of the passes: `IdCheck` (`ids`: every id names an entry of its arena, instances agree
//! with their bodies, paths are unique), `TypeCheck` (`types`, `code`, `expressions`,
//! `statements`: every type written is well formed and every expression has the type its form
//! gives, in the position its form may stand) and `ClassCheck` (`classes`: what each class of
//! position may read and write — a drive, an event, a reaction, the `analog` block).
//!
//! The code walk is iterative (an explicit stack, so nesting depth costs no native stack) and
//! visits each expression once per placement context and set of bound locals, so an id shared by
//! many parents costs one visit, not one per path. An expression met again while it is still
//! being walked is a cycle, whatever its depth.
//!
//! A local is bound by scope, not by evaluation order: a `let` binds its local from the next
//! statement to the end of its block (of the `analog` block at the top level), a closure binds
//! its parameters in its body only, and a function's parameters are bound in its whole body. A
//! `let` in one arm of an `if` sits in that arm's block, so the other arm and what follows the
//! `if` do not see it.

use std::collections::BTreeSet;

use crate::ids::ExprId;
use crate::model::{Constant, EmitField, Ir, Origin, ReprForm, Stmt, ValueType};

mod aggregates;
mod carrier_array;
mod classes;
mod code;
mod expressions;
mod ids;
mod resolved;
mod statements;
mod traits;
mod types;
mod walk;
mod widths;

/// One broken invariant: the item, where it was written, and what is wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Broken {
    pub item: String,
    pub origin: Origin,
    pub what: String,
}

impl Ir {
    /// The broken invariants of this IR; empty when it is well formed.
    pub fn check(&self) -> Vec<Broken> {
        let reader = IrReader { ir: self };
        let mut findings = BrokenInvariants { list: Vec::new() };
        IdCheck { reader, findings: &mut findings }.run();
        TypeCheck { reader, findings: &mut findings, cycles: BTreeSet::new() }.run();
        ClassCheck { reader, findings: &mut findings }.run();
        findings.list
    }
}

/// The broken invariants the checks found, in the order they found them.
struct BrokenInvariants {
    list: Vec<Broken>,
}

impl BrokenInvariants {
    /// Adds one broken invariant.
    fn report(&mut self, item: String, origin: Origin, what: String) {
        self.list.push(Broken { item, origin, what });
    }
}

/// The IR under check, with the questions every check asks of its arenas: where an expression or a
/// statement was written, the type an expression records, and whether a constant is a value of a type.
#[derive(Clone, Copy)]
struct IrReader<'a> {
    ir: &'a Ir,
}

impl<'a> IrReader<'a> {
    /// The origin of an item the IR records no place for.
    fn unknown_origin() -> Origin {
        Origin { file: "<ir>".to_string(), line: 0, col: 0 }
    }

    /// Where expression `id` was written; the unknown origin when `id` is out of range.
    fn expression_origin(&self, id: ExprId) -> Origin {
        self.ir.exprs.get(id.index()).map_or_else(IrReader::unknown_origin, |e| e.origin.clone())
    }

    /// Where a statement was written: the origin of its first expression.
    fn statement_origin(&self, stmt: &Stmt) -> Origin {
        let first = match stmt {
            Stmt::Let { init, .. } => Some(*init),
            Stmt::Expr(e) => Some(*e),
            Stmt::Contribute { addends, .. } => addends.first().map(|a| a.value),
            Stmt::Commit { value, .. } | Stmt::BoundStep { value, .. } | Stmt::Assign { value, .. } => Some(*value),
            Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => fields.iter().find_map(|f| match f {
                EmitField::Value { value, .. } => Some(*value),
                EmitField::Text { .. } => None,
            }),
        };
        first.map_or_else(IrReader::unknown_origin, |e| self.expression_origin(e))
    }

    /// The recorded type of `id`, when it exists.
    fn type_of(&self, id: ExprId) -> Option<&'a ValueType> {
        self.ir.exprs.get(id.index()).map(|e| &e.ty)
    }

    /// Whether `value` is a value of the scalar type `ty`: a `bool` for `bool`, a real for `f64`,
    /// an integer within the `N`-bit range for `UInt<N>`/`Int<N>`.
    fn holds(&self, ty: &ValueType, value: Constant) -> bool {
        match (ty, value) {
            (_, Constant::Aggregate(id)) => self.ir.aggregates.get(id.index()).is_some_and(|data| data.ty == *ty),
            (ValueType::Bool, Constant::Bool(_)) | (ValueType::F64, Constant::Real(_)) => true,
            // a type with a hand-written `Repr` is initialized by its levels, the word `encode` gives (GN-05)
            (ValueType::Struct(id) | ValueType::Enum(id), Constant::Int(levels)) => match self.ir.types.get(id.index()).map(|data| data.repr) {
                Some(ReprForm::HandWritten { width, .. }) => width < 128 && (0..1i128 << width).contains(&levels),
                _ => false,
            },
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                ty.integer_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&v))
            }
            (ValueType::F32 | ValueType::F16 | ValueType::F128, Constant::FloatBits(bits)) => self.float_bits_fit(ty, bits),
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Wide(id)) => self.wide_fits(ty, id),
            _ => false,
        }
    }
}

/// The reference check (`ids.rs`, with the node and carrier arrays of `carrier_array.rs` and the
/// net resolutions of `resolved.rs`): every id names an entry of its arena, every instance agrees with
/// its body and its parent, a body's structure records are in range and kept by its instances, and
/// no two instances, nodes or nets share a path.
struct IdCheck<'a> {
    reader: IrReader<'a>,
    findings: &'a mut BrokenInvariants,
}

/// The type check (`types.rs`, `code.rs`, `expressions.rs`, `statements.rs`, and the aggregate,
/// array, resolve and width forms of their own files): every type written is well formed, every
/// constant a value of its type, and every expression of the code has the type its form gives and
/// stands where its form may.
struct TypeCheck<'a> {
    reader: IrReader<'a>,
    findings: &'a mut BrokenInvariants,
    /// Expressions already reported as reaching themselves.
    cycles: BTreeSet<ExprId>,
}

/// The class check (`classes.rs`): what each class of position may read and write. A body writes
/// only the carriers it may, a commit and an emit sit in a reaction, an event waits on what an event
/// may, and a drive, an event guard, an `at` argument or a placement's param value reads no input.
struct ClassCheck<'a> {
    reader: IrReader<'a>,
    findings: &'a mut BrokenInvariants,
}
