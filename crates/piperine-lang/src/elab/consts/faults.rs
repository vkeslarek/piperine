//! Why an expression has no constant value: [`ConstFault`] and the [`Exit`] it can carry.

use super::super::value::files::FileFault;
use crate::refusal::{Refusal, RefusedForm};
use crate::pom::{DeclId, ExprId};

/// Why an expression has no constant value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstFault {
    /// A read of a param with no binding: the caller says whether it is a param of the parent
    /// (`P0001`) or a param no value was given to (`E0321`).
    UnboundParam { expr: ExprId, param: DeclId },
    /// Something only a run can give: a probe, an environment function, a node, a module.
    Dynamic { expr: ExprId, what: String },
    /// A form of the subset the evaluator does not fold: `P0001` naming `refusal`.
    Refused { expr: ExprId, refusal: Box<Refusal> },
    /// A `const` item with no value (a declaration in a trait): nothing to fold.
    NoValue { decl: DeclId },
    /// A panic of the evaluation (§1 rule 11): an integer division or remainder by zero, a shift
    /// by a negative amount, a `clamp` whose bounds are crossed. `E0104` at elaboration.
    Panic { expr: ExprId, what: String },
    /// The initializer of the `const` item `constant` reads, at `expr`, something only a run gives
    /// (`what`): the `const` is not a const expression (§3.9.2, `E0188`).
    NotConstant { expr: ExprId, constant: DeclId, what: String },
    /// A `const` read, at `expr`, while its own initializer is being folded: it depends on itself
    /// (§3.9.2, `E0188`).
    Cyclic { expr: ExprId, constant: DeclId },
    /// The file an `include_str!` at `expr` names has no text (§20 errors: `E0612`, `E0613`).
    File { expr: ExprId, fault: FileFault },
    /// The text `parse_table` or `parse_hex` reads does not fit the `const`'s type (`E0614`) or is not in the
    /// format (`E0706`, §22.6.4): `message` names the source and the row.
    Table { expr: ExprId, code: &'static str, message: String },
    /// A read of a `let` written in a generation arm the evaluation does not take (§8.2.24, `E0342`): `message` names it.
    Ungenerated { expr: ExprId, message: String },
    /// A `break`, `continue` or `return` on its way to the loop or function it leaves; the value it
    /// carries waits in the evaluator. Never reaches a caller of [`ConstEval::value`].
    Leaves { expr: ExprId, exit: Exit },
}

/// Which exit a [`ConstFault::Leaves`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// `break`, with or without a value.
    Break,
    /// `continue`.
    Continue,
    /// `return`, with or without a value.
    Return,
}

impl ConstFault {
    /// The fault of a form the evaluator does not fold: `P0001` naming `form` at `expr`. The refusal is boxed so the fault stays
    /// small: a fault travels in every `Result` of the recursive evaluator.
    pub fn refused(expr: ExprId, form: impl Into<RefusedForm>) -> ConstFault {
        ConstFault::refused_with(expr, form, [])
    }

    /// [`refused`](Self::refused) with `details` for the `{}` of the form's words.
    pub fn refused_with(expr: ExprId, form: impl Into<RefusedForm>, details: impl IntoIterator<Item = String>) -> ConstFault {
        ConstFault::Refused { expr, refusal: Box::new(details.into_iter().fold(Refusal::of(form), Refusal::with_detail)) }
    }

    /// The expression the fault sits on; none for a `const` item without a value.
    pub fn expr(&self) -> Option<ExprId> {
        match self {
            ConstFault::UnboundParam { expr, .. } | ConstFault::Dynamic { expr, .. } | ConstFault::Refused { expr, .. } | ConstFault::Panic { expr, .. } | ConstFault::Cyclic { expr, .. } | ConstFault::NotConstant { expr, .. } | ConstFault::File { expr, .. } | ConstFault::Table { expr, .. } | ConstFault::Ungenerated { expr, .. } | ConstFault::Leaves { expr, .. } => Some(*expr),
            ConstFault::NoValue { .. } => None,
        }
    }
}
