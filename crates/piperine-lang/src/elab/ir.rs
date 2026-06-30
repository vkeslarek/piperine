//! # Elaborated IR
//!
//! Types produced by the [`Elaborator`][super::lower::Elaborator] from a
//! parsed [`SourceFile`][crate::parse::SourceFile].
//!
//! ```text
//! SourceFile (parse AST)  ──Elaborator──▶  ElabProgram (elaborated IR)
//! ```
//!
//! ## Guarantees carried by these types
//!
//! Every field in this module is *stronger* than its counterpart in the parse
//! AST. The Rust type system encodes each guarantee:
//!
//! | Parse AST | Elaborated IR | Guarantee |
//! |-----------|---------------|-----------|
//! | `Type { name: String, dimensions: Vec<Expr> }` | `ElabNetType` / `ElabValueType` | type is resolved; dimensions are concrete `u64` |
//! | `Port { ty: Type }` — may be a bundle | `ElabPort { ty: ElabNetType }` | net type only; bundles expanded to flat fields |
//! | `ModDecl { const_params, type_params }` | `ElabMod` — no params lists | all generic params substituted |
//! | `ModStmt::StructuralFor` / `StructuralIf` | absent | unrolled / evaluated away |
//! | `Instance::ports: Vec<Expr>` — raw exprs | `ElabInstance::ports: Vec<ElabNetRef>` | concrete net name + optional index |
//! | `Connection { lhs: Expr, rhs: Expr }` | `ElabConn { lhs: ElabNetRef, rhs: ElabNetRef }` | both sides are concrete net references |
//! | `FnDecl { body: Block }` — raw AST | `ElabFn { body: Vec<ElabBehaviorStmt> }` | body lowered, for loops unrolled |
//! | `ImplDecl { methods: Vec<FnDecl> }` | `ElabImpl { methods: Vec<ElabFn> }` | methods fully elaborated |
//! | `EventSpec::Named { name }` — any string | validated against `EventRegistry` | name is a known event kind |
//! | `BehaviorStmt::For` — may be non-const | `ElabBehaviorStmt` — unrolled | loop bounds were elaboration constants |
//! | generic module instances | appear in `ElabProgram::modules` | monomorphized on demand |
//!
//! Code that holds an `ElabProgram` can rely on all of the above without
//! additional checking.

use std::collections::HashMap;
use thiserror::Error;

use crate::parse::ast::{
    BehaviorKind, BindOp, CapabilityDecl, DisciplineDecl, EnumDecl, EventSpec, Pattern,
};
use crate::elab::const_eval::{ConstEvalError, ConstVal};

// ─────────────────────────────── Errors ──────────────────────────────────────

/// An error raised during elaboration.
///
/// Each variant names the invariant that was violated. Call sites wrap these
/// with context (the `context` field in `ConstEval`) so error messages point
/// to the offending declaration.
#[derive(Debug, Error)]
pub enum ElabError {
    /// A const expression in elaboration position could not be evaluated.
    #[error("const eval error in `{context}`: {source}")]
    ConstEval {
        context: String,
        #[source]
        source: ConstEvalError,
    },
    /// A type name was not found in any symbol table.
    #[error("undefined type: `{0}`")]
    UndefinedType(String),
    /// A module name was not found during monomorphization.
    #[error("undefined module: `{0}`")]
    UndefinedModule(String),
    /// A bundle was used as a net type but contains value-type fields.
    #[error("bundle `{0}` is not net-capable (contains non-net fields)")]
    NotNetCapable(String),
    /// `<+` inside a `digital` block.
    #[error("contribution `<+` is not allowed in a digital block")]
    ContribInDigital,
    /// `<+` inside a `mod` body.
    #[error("contribution `<+` is not allowed in a mod body")]
    ContribInModBody,
    /// `<-` inside a `mod` body.
    #[error("force `<-` is not allowed in a mod body")]
    ForceInModBody,
    /// An event name that has no registration in the `EventRegistry`.
    #[error("unknown event kind: `{0}`")]
    UnknownEvent(String),
    /// An analog-only event (cross/above) inside a `digital` block.
    #[error("analog-only event `{0}` used inside a digital block")]
    AnalogEventInDigital(String),
    /// A digital-only event (posedge/negedge/change) inside an `analog` block.
    #[error("digital-only event `{0}` used inside an analog block")]
    DigitalEventInAnalog(String),
    /// A module was instantiated with the wrong number of const arguments.
    #[error("const param `{param}` not provided for module `{module}`")]
    MissingConstParam { param: String, module: String },
    /// An expression in a port connection or net connection could not be
    /// reduced to a concrete net reference.
    #[error("expression cannot be reduced to a net reference: {0}")]
    NotANetRef(String),
    /// Catch-all for other elaboration errors.
    #[error("{0}")]
    Other(String),
}

// ─────────────────────────────── Net reference ───────────────────────────────

/// A concrete reference to a net, post-elaboration.
///
/// ## Guarantee
///
/// Both `net` and `index` (if present) are fully resolved — no free variables,
/// no unevaluated expressions. The `net` name refers to a port, wire, or
/// parameter declared in the enclosing module.
///
/// ## Naming convention
///
/// - Simple net: `ElabNetRef { net: "a", index: None }`
/// - Array element: `ElabNetRef { net: "node", index: Some(3) }`
/// - Bundle-expanded field: `ElabNetRef { net: "inp_p", index: None }`
///   (already flattened at port-expansion time)
#[derive(Debug, Clone, PartialEq)]
pub struct ElabNetRef {
    pub net: String,
    pub index: Option<u64>,
}

impl ElabNetRef {
    pub fn simple(net: impl Into<String>) -> Self {
        Self { net: net.into(), index: None }
    }

    pub fn indexed(net: impl Into<String>, index: u64) -> Self {
        Self { net: net.into(), index: Some(index) }
    }
}

impl std::fmt::Display for ElabNetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.index {
            None => write!(f, "{}", self.net),
            Some(i) => write!(f, "{}[{}]", self.net, i),
        }
    }
}

// ─────────────────────────────── Net types ───────────────────────────────────

/// A resolved, concrete **net** type.
///
/// ## Guarantees
///
/// - No generic parameters — all type variables have been substituted.
/// - Array extents are concrete `u64` values — no free expressions.
/// - The discipline name exists in the program's discipline table.
/// - `Array` is never zero-length (validated during elaboration).
#[derive(Debug, Clone, PartialEq)]
pub enum ElabNetType {
    /// A named discipline, e.g. `Electrical`, `Bit`, `Logic`.
    Discipline(String),
    /// An array of a net type with a concrete element count.
    Array(Box<ElabNetType>, u64),
}

// ─────────────────────────────── Value types ─────────────────────────────────

/// A resolved, concrete **value** type.
///
/// ## Guarantees
///
/// - No generic parameters.
/// - All array extents are concrete `u64` values.
/// - Enum names exist in the program's enum table.
#[derive(Debug, Clone, PartialEq)]
pub enum ElabValueType {
    Real,
    Natural,
    Integer,
    Complex,
    Boolean,
    Quad,
    Str,
    /// A named enum, e.g. `SarState`.
    Enum(String),
    /// An array of a value type with a concrete element count.
    Array(Box<ElabValueType>, u64),
    /// A function pointer type, e.g. `fn(Real) -> Real`.
    FnPtr(Vec<ElabType>, Box<ElabType>),
}

/// A resolved type — either a net type or a value type.
///
/// Used in positions that may hold either (function params, generic bounds).
#[derive(Debug, Clone, PartialEq)]
pub enum ElabType {
    Net(ElabNetType),
    Value(ElabValueType),
}

impl ElabType {
    pub fn as_net(&self) -> Option<&ElabNetType> {
        match self { ElabType::Net(n) => Some(n), _ => None }
    }

    pub fn as_value(&self) -> Option<&ElabValueType> {
        match self { ElabType::Value(v) => Some(v), _ => None }
    }
}

// ─────────────────────────────── Module IR ───────────────────────────────────

/// An elaborated port — always a concrete net type, never a bundle reference.
///
/// ## Guarantee
///
/// Bundles in the source port list are expanded to one `ElabPort` per field,
/// named `{port}_{field}`. The `ty` field is always a pure net type.
#[derive(Debug, Clone)]
pub struct ElabPort {
    pub direction: crate::parse::ast::Direction,
    /// May be `"inp_p"` if expanded from a `DiffPair` port named `inp`.
    pub name: String,
    pub ty: ElabNetType,
}

/// An elaborated `param` declaration inside a module.
#[derive(Debug, Clone)]
pub struct ElabParam {
    pub name: String,
    pub ty: ElabValueType,
    /// Evaluated default value. `None` means the param is required at instantiation.
    pub default: Option<ConstVal>,
}

/// An elaborated `wire` declaration inside a module.
#[derive(Debug, Clone)]
pub struct ElabWire {
    pub name: String,
    pub ty: ElabNetType,
}

/// An elaborated module instance.
///
/// ## Guarantees
///
/// - `module` is a monomorphized module name (e.g. `Dac__8`). The referenced
///   module is guaranteed to exist in `ElabProgram::modules`.
/// - `ports` are concrete `ElabNetRef`s — no raw expressions.
/// - `params` have values already evaluated to `ConstVal`.
#[derive(Debug, Clone)]
pub struct ElabInstance {
    /// `None` for anonymous instances; `Some` for `label : Module(...)`.
    pub label: Option<String>,
    /// Monomorphized module name.
    pub module: String,
    /// Positional port connections — concrete net references.
    pub ports: Vec<ElabNetRef>,
    /// Named parameter overrides, values resolved to `ConstVal`.
    pub params: Vec<(String, ConstVal)>,
}

/// An elaborated net connection: `lhs = rhs;`.
///
/// ## Guarantee
///
/// Both sides are concrete net references — no unevaluated expressions.
#[derive(Debug, Clone)]
pub struct ElabConn {
    pub lhs: ElabNetRef,
    pub rhs: ElabNetRef,
}

/// An elaborated module.
///
/// ## Guarantees
///
/// - No `const_params` or `type_params` — all have been substituted.
/// - No `StructuralFor` or `StructuralIf` — both have been evaluated away.
/// - All port types are `ElabNetType` — bundles are expanded to flat fields.
/// - All array dimensions (in port/wire types) are concrete `u64` values.
/// - All instance `module` names exist in `ElabProgram::modules`.
/// - All port connections are `ElabNetRef` — no raw expressions.
#[derive(Debug, Clone)]
pub struct ElabMod {
    /// Monomorphized name, e.g. `RcChain__4` for `RcChain[4]`.
    pub name: String,
    pub ports: Vec<ElabPort>,
    pub params: Vec<ElabParam>,
    pub wires: Vec<ElabWire>,
    pub instances: Vec<ElabInstance>,
    pub connections: Vec<ElabConn>,
}

// ─────────────────────────────── Behavior IR ─────────────────────────────────

/// An elaborated behavior statement.
///
/// ## Guarantees
///
/// - No `For` variant — all behavioral for loops with elaboration-constant
///   bounds have been unrolled. What was `for i in 0..4 { ... }` is now four
///   copies of the body with `i` substituted.
/// - `If` conditions that were elaboration-constant have been folded — dead
///   branches dropped.
/// - `Event { spec }` — the event name in `spec` has been validated against
///   the `EventRegistry`; the spec is a known event kind.
/// - `VarDecl { ty }` is a concrete `ElabValueType`.
#[derive(Debug, Clone)]
pub enum ElabBehaviorStmt {
    VarDecl {
        name: String,
        ty: ElabValueType,
        default: Option<crate::parse::ast::Expr>,
    },
    Bind {
        dest: crate::parse::ast::Expr,
        op: BindOp,
        src: crate::parse::ast::Expr,
    },
    If {
        cond: crate::parse::ast::Expr,
        then_body: Vec<ElabBehaviorStmt>,
        else_body: Option<Vec<ElabBehaviorStmt>>,
    },
    Match {
        expr: crate::parse::ast::Expr,
        arms: Vec<ElabMatchArm>,
    },
    /// Event block: `@ spec [when (guard)] { body }`.
    /// `spec` has been validated against the `EventRegistry`.
    Event {
        spec: EventSpec,
        guard: Option<crate::parse::ast::Expr>,
        body: Vec<ElabBehaviorStmt>,
    },
    Diagnostic {
        sys: String,
        args: Vec<crate::parse::ast::Expr>,
    },
    Expr(crate::parse::ast::Expr),
}

#[derive(Debug, Clone)]
pub struct ElabMatchArm {
    pub pat: Pattern,
    pub body: Vec<ElabBehaviorStmt>,
}

/// An elaborated behavior block (`analog` or `digital`).
///
/// ## Guarantee
///
/// `body` contains only `ElabBehaviorStmt`s — no unresolved for loops,
/// no unvalidated event names.
#[derive(Debug, Clone)]
pub struct ElabBehavior {
    pub name: String,
    pub kind: BehaviorKind,
    pub body: Vec<ElabBehaviorStmt>,
}

// ─────────────────────────────── Function IR ─────────────────────────────────

/// An elaborated function.
///
/// ## Guarantees
///
/// - `body` is fully lowered to `ElabBehaviorStmt`s — no raw AST `Block`.
/// - For loops in the body have been unrolled if bounds were const.
/// - `params` and `ret` are resolved types (`ElabType`).
///
/// ## Note on generics
///
/// Generic functions (those with `type_params`) are stored with type-param-
/// dependent types resolved to best-effort `Real` placeholders. Full generic
/// monomorphization at call sites is future work.
#[derive(Debug, Clone)]
pub struct ElabFn {
    pub name: String,
    pub params: Vec<(String, ElabType)>,
    pub ret: ElabType,
    /// Fully lowered body — no raw `Block`.
    pub body: Vec<ElabBehaviorStmt>,
}

// ─────────────────────────────── Impl IR ─────────────────────────────────────

/// An elaborated capability implementation.
///
/// ## Guarantee
///
/// `methods` are fully elaborated `ElabFn`s — bodies lowered, for loops
/// unrolled. Not raw `FnDecl` AST.
#[derive(Debug, Clone)]
pub struct ElabImpl {
    /// `Some` for capability impls; `None` for inherent impls.
    pub capability: Option<String>,
    pub ty: String,
    /// Evaluated const arguments (e.g. `N=8` in `impl Add for UInt[8]`).
    pub const_args: Vec<ConstVal>,
    pub methods: Vec<ElabFn>,
}

// ─────────────────────────────── Program ─────────────────────────────────────

/// The complete output of elaboration.
///
/// ## Guarantees
///
/// - `modules`: all non-generic modules from the source, plus every generic
///   module that was instantiated (monomorphized on demand). Keys are
///   monomorphized names. All instance references resolve to a key here.
/// - `behaviors`: one entry per `analog`/`digital` block; for loops unrolled.
/// - `disciplines`, `enums`, `capabilities`: registered verbatim.
/// - `functions`: bodies lowered to `Vec<ElabBehaviorStmt>`.
/// - `impls`: methods fully elaborated.
#[derive(Debug, Clone)]
pub struct ElabProgram {
    pub modules: HashMap<String, ElabMod>,
    pub behaviors: Vec<ElabBehavior>,
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub capabilities: HashMap<String, CapabilityDecl>,
    pub functions: HashMap<String, ElabFn>,
    pub impls: Vec<ElabImpl>,
}

impl ElabProgram {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            behaviors: Vec::new(),
            disciplines: HashMap::new(),
            enums: HashMap::new(),
            capabilities: HashMap::new(),
            functions: HashMap::new(),
            impls: Vec::new(),
        }
    }
}

impl Default for ElabProgram {
    fn default() -> Self {
        Self::new()
    }
}
