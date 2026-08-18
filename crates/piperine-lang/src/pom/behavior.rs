//! POM behavioral nodes — [`Behavior`] (an `analog`/`digital` block), its
//! statements, and [`Function`]/[`ImplBlock`] (value-layer computation).

use crate::value::Value;
use crate::ast::BehaviorKind;
use crate::pom::net_type::TypeRef;
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named};

/// Behavior bodies are the surface [`Stmt`](crate::ast::Stmt) type
/// directly (design F1, preserved across the MD-44 frontend swap — only the
/// surface AST's identity changed, `piperine_lang::parse::ast` →
/// `crate::ast`).
pub use crate::ast::Stmt as BehaviorStmt;

/// A behavior block inside a module (analog or digital).
#[derive(Debug, Clone)]
pub struct Behavior {
    pub span: Option<miette::SourceSpan>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07). Not
    /// `#[serde]`-carried: `Behavior` itself does not cross the
    /// serialization boundary (SPEC Part IV §7).
    pub doc: Option<String>,
    /// Behavior block name.
    pub name: String,
    /// Whether this is an analog or digital block.
    pub kind: BehaviorKind,
    /// The statements making up the behavior body (elaborated: structural
    /// `if`/`for` folded away).
    pub body: Vec<BehaviorStmt>,
    /// Resolved value types of the body's `var` declarations, keyed by
    /// name — the side table elaboration adds on top of the surface AST.
    pub var_types: std::collections::HashMap<String, crate::pom::ValueType>,
}

impl Behavior {
    /// Construct a new Behavior (used by the elaborator and codegen).
    #[doc(hidden)]
    pub fn new(name: String, kind: BehaviorKind, body: Vec<BehaviorStmt>) -> Self {
        Self { span: None, doc: None, name, kind, body, var_types: Default::default() }
    }

    /// The behavior block name.
    pub fn name(&self) -> &str { &self.name }
    /// The behavior's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
    /// The behavior kind (analog or digital).
    pub fn kind(&self) -> &BehaviorKind { &self.kind }
    /// The statements inside the behavior block.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }

    /// Returns `true` for `analog`/`analog initial` behavior blocks (LRM
    /// §5, continuous-time).
    pub fn is_analog(&self) -> bool {
        matches!(self.kind, BehaviorKind::Analog | BehaviorKind::AnalogInitial)
    }
    /// Returns `true` for `initial`/`always` behavior blocks (event-driven
    /// digital, inherited from IEEE 1364).
    pub fn is_digital(&self) -> bool {
        matches!(self.kind, BehaviorKind::Initial | BehaviorKind::Always)
    }
}

impl Named for Behavior { fn name(&self) -> &str { self.name() } }
impl Kinded for Behavior { fn kind(&self) -> Kind { Kind::Behavior } }

// ─────────────────────────────── Function ────────────────────────────────────

/// A value-layer function definition.
#[derive(Debug, Clone)]
pub struct Function {
    pub span: Option<miette::SourceSpan>,
    /// Function name.
    pub name: String,
    /// Parameter names and types.
    pub params: Vec<(String, TypeRef)>,
    /// Default value expressions, parallel to [`params`](Self::params) —
    /// `None` for a non-defaulted (leading) param, `Some(expr)` for a
    /// trailing defaulted one (the language spec Part I §9.1). Defaults are
    /// elaboration constants.
    pub defaults: Vec<Option<crate::ast::Expr>>,
    /// Return type.
    pub ret: TypeRef,
    /// Function body statements.
    pub body: Vec<BehaviorStmt>,
    /// Whether this function is generic (has type or const parameters).
    /// Generic functions are retained for reflection but not lowered into
    /// the IR until monomorphized at a call site.
    pub is_generic: bool,
}

impl Function {
    /// The function name.
    pub fn name(&self) -> &str { &self.name }
    /// The function parameters (name, type).
    pub fn params(&self) -> &[(String, TypeRef)] { &self.params }
    /// Default value expressions, parallel to [`params`](Self::params).
    pub fn defaults(&self) -> &[Option<crate::ast::Expr>] { &self.defaults }
    /// The function return type.
    pub fn ret(&self) -> &TypeRef { &self.ret }
    /// The function body statements.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
    /// Whether this function is generic (not lowerable until monomorphized).
    pub fn is_generic(&self) -> bool { self.is_generic }
}

impl Named for Function { fn name(&self) -> &str { self.name() } }

// ─────────────────────────────── PomRequire ──────────────────────────────────

/// One authored `require name : pred [tol scale];` of a module's
/// `constraint begin … end` block (sanctioned extension, MD-49) — the
/// authored surface on the POM (design F1), in authored order across all
/// of the module's blocks. Name resolution and margin lowering are the
/// codegen `resolve` stage's job, exactly as for the analog body.
#[derive(Debug, Clone)]
pub struct PomRequire {
    pub span: Option<miette::SourceSpan>,
    pub name: String,
    pub pred: crate::ast::Expr,
    pub tol: Option<crate::ast::Expr>,
}

/// One authored constraint statement (sanctioned extension, MD-49 +
/// `constraint-statements`), in authored order — one list across kinds
/// because `target#<n>` naming is statement-index-based and `var`
/// substitution is authored-order. Authored surface (design F1); the
/// codegen `resolve` stage classifies and lowers.
#[derive(Debug, Clone)]
pub enum PomConstraintStmt {
    Require(PomRequire),
    /// `var name = expr;` — a constraint-local computed quantity.
    Var {
        span: Option<miette::SourceSpan>,
        name: String,
        expr: crate::ast::Expr,
    },
    /// `target expr cmp level [tol scale];` — an unlabeled soft goal,
    /// named `target#<stmt-index>` by the lowering.
    Target {
        span: Option<miette::SourceSpan>,
        expr: crate::ast::Expr,
        cmp: crate::ast::TargetCmp,
        level: crate::ast::Expr,
        tol: Option<crate::ast::Expr>,
    },
    /// `cover name : expr bins …;` — a coverage point (counter, no margin).
    Cover {
        span: Option<miette::SourceSpan>,
        name: String,
        expr: crate::ast::Expr,
        bins: crate::ast::CoverBins,
    },
    /// `cover cross name : p1, p2, …;` — joint coverage over named points.
    CoverCross {
        span: Option<miette::SourceSpan>,
        name: String,
        points: Vec<String>,
    },
    /// `@ <analysis> [(after, dur)] { statements }` — an analysis-scoped
    /// group (DVC-19/20): contained statements evaluate only under the
    /// named analysis, and (for `@ tran`) only inside the window — the
    /// codegen lowering propagates `analyses`/`window` into the contained
    /// margins. `dur: None` is open-ended.
    Scope {
        span: Option<miette::SourceSpan>,
        analysis: String,
        after: Option<crate::ast::Expr>,
        dur: Option<crate::ast::Expr>,
        stmts: Vec<PomConstraintStmt>,
    },
}

impl PomConstraintStmt {
    /// The statement's authored label, if it has one — requires, vars,
    /// and covers are named; a target is unlabeled (`target#<n>` is
    /// assigned by the lowering); a scope group has none of its own (its
    /// contained statements keep theirs). The duplicate-label check
    /// (E4008) consults this across kinds.
    pub fn label(&self) -> Option<&str> {
        match self {
            PomConstraintStmt::Require(r) => Some(&r.name),
            PomConstraintStmt::Var { name, .. } => Some(name),
            PomConstraintStmt::Cover { name, .. } => Some(name),
            PomConstraintStmt::CoverCross { name, .. } => Some(name),
            PomConstraintStmt::Target { .. } | PomConstraintStmt::Scope { .. } => None,
        }
    }

    /// Recursively visit this statement's labels (scope groups descend
    /// into their contained statements) — the duplicate check's walker.
    pub fn for_each_label<'a>(&'a self, f: &mut impl FnMut(&'a PomConstraintStmt)) {
        match self {
            PomConstraintStmt::Scope { stmts, .. } => {
                for s in stmts {
                    s.for_each_label(f);
                }
            }
            _ => f(self),
        }
    }

    /// The statement label's span (the duplicate-label diagnostic's
    /// `first`/`second` anchors).
    pub fn label_span(&self) -> Option<miette::SourceSpan> {
        match self {
            PomConstraintStmt::Require(r) => r.span,
            PomConstraintStmt::Var { span, .. }
            | PomConstraintStmt::Target { span, .. }
            | PomConstraintStmt::Cover { span, .. }
            | PomConstraintStmt::CoverCross { span, .. }
            | PomConstraintStmt::Scope { span, .. } => *span,
        }
    }
}

// ─────────────────────────────── ImplBlock ───────────────────────────────────

/// An `impl` block — associates methods with a type, optionally gated by a capability.
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub span: Option<miette::SourceSpan>,
    /// Optional capability gate (e.g. `analog`, `digital`).
    pub capability: Option<String>,
    /// The type being implemented.
    pub ty: String,
    /// Constant generic arguments of the type.
    pub const_args: Vec<Value>,
    /// Methods defined in this impl block.
    pub methods: Vec<Function>,
}

impl ImplBlock {
    /// The capability gate, if any.
    pub fn capability(&self) -> Option<&str> { self.capability.as_deref() }
    /// The type being implemented.
    pub fn ty(&self) -> &str { &self.ty }
    /// All methods in this impl block.
    pub fn methods(&self) -> &[Function] { &self.methods }
}
