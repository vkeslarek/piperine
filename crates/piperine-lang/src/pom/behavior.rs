//! POM behavioral nodes — [`Behavior`] (an `analog`/`digital` block), its
//! statements, and [`Function`]/[`ImplBlock`] (value-layer computation).

use crate::value::Value;
use crate::parse::ast::BehaviorKind;
use crate::pom::net_type::TypeRef;
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named};

/// Behavior bodies are the surface [`Stmt`] type directly
/// (SIMPLIFICATION.md P3): the elaborator const-folds structural `if`s and
/// unrolls `for`s *in place*, and records the one thing it genuinely adds —
/// resolved `var` types — in [`Behavior::var_types`], instead of deep-copying
/// every statement into a parallel enum.
pub use crate::parse::ast::{Stmt as BehaviorStmt, StmtMatchArm as MatchArm};

/// A behavior block inside a module (analog or digital).
#[derive(Debug, Clone)]
pub struct Behavior {
    pub span: Option<miette::SourceSpan>,
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
        Self { span: None, name, kind, body, var_types: Default::default() }
    }

    /// The behavior block name.
    pub fn name(&self) -> &str { &self.name }
    /// The behavior kind (analog or digital).
    pub fn kind(&self) -> &BehaviorKind { &self.kind }
    /// The statements inside the behavior block.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }

    /// Returns `true` if this is an `analog` behavior block.
    pub fn is_analog(&self) -> bool { matches!(self.kind, BehaviorKind::Analog) }
    /// Returns `true` if this is a `digital` behavior block.
    pub fn is_digital(&self) -> bool { matches!(self.kind, BehaviorKind::Digital) }
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
    /// trailing defaulted one (SPEC_BENCH.md §10). Defaults are
    /// elaboration constants.
    pub defaults: Vec<Option<crate::parse::ast::Expr>>,
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
    pub fn defaults(&self) -> &[Option<crate::parse::ast::Expr>] { &self.defaults }
    /// The function return type.
    pub fn ret(&self) -> &TypeRef { &self.ret }
    /// The function body statements.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
    /// Whether this function is generic (not lowerable until monomorphized).
    pub fn is_generic(&self) -> bool { self.is_generic }
}

impl Named for Function { fn name(&self) -> &str { self.name() } }

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
