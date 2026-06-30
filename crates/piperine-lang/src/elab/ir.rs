//! # Piperine Object Model (POM) — Elaborated IR
//!
//! Types produced by the [`Elaborator`][super::lower::Elaborator] from a
//! parsed [`SourceFile`][crate::parse::SourceFile]. After elaboration these
//! types double as the **Piperine Object Model** — the reflection API
//! defined in `docs/reflection_api.md`.
//!
//! ```text
//! SourceFile (parse AST)  ──Elaborator──▶  Design (POM root + elaborated IR)
//! ```
//!
//! ## Visibility convention
//!
//! Struct fields are `pub(crate)` — internal to `piperine-lang` so the
//! elaborator and codegen can construct and read them directly. External
//! consumers (plugins, tests, hosts) use the public accessor methods that
//! implement the POM reflection interface.
//!
//! Enum variants are `pub` — they ARE the protocol.

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use thiserror::Error;

use crate::parse::ast::{
    BehaviorKind, BindOp, CapabilityDecl, DisciplineDecl, EnumDecl, EventSpec, Pattern,
};
use crate::elab::const_eval::{ConstEvalError, ConstVal};
use crate::pom::{OverrideMap, Selection, Value};

// ─────────────────────────────── Errors ──────────────────────────────────────

#[derive(Debug, Error)]
pub enum ElabError {
    #[error("const eval error in `{context}`: {source}")]
    ConstEval { context: String, #[source] source: ConstEvalError },
    #[error("undefined type: `{0}`")]
    UndefinedType(String),
    #[error("undefined module: `{0}`")]
    UndefinedModule(String),
    #[error("bundle `{0}` is not net-capable (contains non-net fields)")]
    NotNetCapable(String),
    #[error("contribution `<+` is not allowed in a digital block")]
    ContribInDigital,
    #[error("contribution `<+` is not allowed in a mod body")]
    ContribInModBody,
    #[error("force `<-` is not allowed in a mod body")]
    ForceInModBody,
    #[error("unknown event kind: `{0}`")]
    UnknownEvent(String),
    #[error("analog-only event `{0}` used inside a digital block")]
    AnalogEventInDigital(String),
    #[error("digital-only event `{0}` used inside an analog block")]
    DigitalEventInAnalog(String),
    #[error("const param `{param}` not provided for module `{module}`")]
    MissingConstParam { param: String, module: String },
    #[error("expression cannot be reduced to a net reference: {0}")]
    NotANetRef(String),
    #[error("{0}")]
    Other(String),
}

// ─────────────────────────────── Net reference ───────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct NetRef {
    pub net: String,
    pub index: Option<u64>,
}

impl NetRef {
    pub fn simple(net: impl Into<String>) -> Self {
        Self { net: net.into(), index: None }
    }
    pub fn indexed(net: impl Into<String>, index: u64) -> Self {
        Self { net: net.into(), index: Some(index) }
    }
    pub fn net(&self) -> &str { &self.net }
    pub fn index(&self) -> Option<u64> { self.index }
}

impl std::fmt::Display for NetRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.index {
            None => write!(f, "{}", self.net),
            Some(i) => write!(f, "{}[{}]", self.net, i),
        }
    }
}

// ─────────────────────────────── Net types ───────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum NetType {
    Discipline(String),
    Array(Box<NetType>, u64),
}

impl NetType {
    pub fn discipline_name(&self) -> &str {
        match self { Self::Discipline(s) => s, Self::Array(inner, _) => inner.discipline_name() }
    }
    pub fn width(&self) -> u64 {
        match self { Self::Discipline(_) => 1, Self::Array(inner, n) => inner.width() * n }
    }
}

// ─────────────────────────────── Value types ─────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    Real, Natural, Integer, Complex, Boolean, Quad, Str,
    Enum(String),
    Array(Box<ValueType>, u64),
    FnPtr(Vec<TypeRef>, Box<TypeRef>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Net(NetType),
    Value(ValueType),
}

impl TypeRef {
    pub fn as_net(&self) -> Option<&NetType> {
        match self { TypeRef::Net(n) => Some(n), _ => None }
    }
    pub fn as_value(&self) -> Option<&ValueType> {
        match self { TypeRef::Value(v) => Some(v), _ => None }
    }
}

// ─────────────────────────────── Port ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Port {
    pub direction: crate::parse::ast::Direction,
    pub name: String,
    pub ty: NetType,
}

impl Port {
    pub fn name(&self) -> &str { &self.name }
    pub fn direction(&self) -> &crate::parse::ast::Direction { &self.direction }
    pub fn net_type(&self) -> &NetType { &self.ty }
}

// ─────────────────────────────── Param ───────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: ValueType,
    pub default: Option<ConstVal>,
}

impl Param {
    pub fn name(&self) -> &str { &self.name }
    pub fn value_type(&self) -> &ValueType { &self.ty }
    pub fn default(&self) -> Option<&ConstVal> { self.default.as_ref() }

    /// Returns the param's value as a POM `Value`. If the param has a
    /// default, it is converted; otherwise `None`.
    pub fn value(&self) -> Option<Value> {
        self.default.as_ref().map(const_val_to_value)
    }
}

fn const_val_to_value(cv: &ConstVal) -> Value {
    match cv {
        ConstVal::Real(v) => Value::Real(*v),
        ConstVal::Int(v) => Value::Integer(*v),
        ConstVal::Nat(v) => Value::Natural(*v),
        ConstVal::Bool(v) => Value::Boolean(*v),
        ConstVal::Str(v) => Value::String(v.clone()),
    }
}

// ─────────────────────────────── Wire ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Wire {
    pub name: String,
    pub ty: NetType,
}

impl Wire {
    pub fn name(&self) -> &str { &self.name }
    pub fn net_type(&self) -> &NetType { &self.ty }
}

// ─────────────────────────────── Instance ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Instance {
    pub label: Option<String>,
    pub module: String,
    pub ports: Vec<NetRef>,
    pub params: Vec<(String, ConstVal)>,
}

impl Instance {
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.module)
    }
    pub fn module_name(&self) -> &str { &self.module }
    pub fn ports(&self) -> &[NetRef] { &self.ports }
    pub fn params(&self) -> &[(String, ConstVal)] { &self.params }
    pub fn label(&self) -> Option<&str> { self.label.as_deref() }
}

// ─────────────────────────────── Connection ──────────────────────────────────

#[derive(Debug, Clone)]
pub struct Connection {
    pub lhs: NetRef,
    pub rhs: NetRef,
}

impl Connection {
    pub fn lhs(&self) -> &NetRef { &self.lhs }
    pub fn rhs(&self) -> &NetRef { &self.rhs }
}

// ─────────────────────────────── Module ──────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub ports: Vec<Port>,
    pub params: Vec<Param>,
    pub wires: Vec<Wire>,
    pub instances: Vec<Instance>,
    pub connections: Vec<Connection>,
    pub behaviors: Vec<Behavior>,
}

impl Module {
    /// Construct a new Module (used by the elaborator and codegen).
    #[doc(hidden)]
    pub fn new(
        name: String,
        ports: Vec<Port>,
        params: Vec<Param>,
        wires: Vec<Wire>,
        instances: Vec<Instance>,
        connections: Vec<Connection>,
        behaviors: Vec<Behavior>,
    ) -> Self {
        Self { name, ports, params, wires, instances, connections, behaviors }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn is_generic(&self) -> bool { false } // always false post-monomorphization

    pub fn ports(&self) -> &[Port] { &self.ports }
    pub fn params(&self) -> &[Param] { &self.params }
    pub fn wires(&self) -> &[Wire] { &self.wires }
    pub fn instances(&self) -> &[Instance] { &self.instances }
    pub fn connections(&self) -> &[Connection] { &self.connections }
    pub fn behaviors(&self) -> &[Behavior] { &self.behaviors }

    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|p| p.name == name)
    }
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }
    pub fn wire(&self, name: &str) -> Option<&Wire> {
        self.wires.iter().find(|w| w.name == name)
    }
    pub fn instance(&self, name: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.label.as_deref() == Some(name))
    }
}

// ─────────────────────────────── Behavior ────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BehaviorStmt {
    VarDecl { name: String, ty: ValueType, default: Option<crate::parse::ast::Expr> },
    Bind { dest: crate::parse::ast::Expr, op: BindOp, src: crate::parse::ast::Expr },
    If { cond: crate::parse::ast::Expr, then_body: Vec<BehaviorStmt>, else_body: Option<Vec<BehaviorStmt>> },
    Match { expr: crate::parse::ast::Expr, arms: Vec<MatchArm> },
    Event { spec: EventSpec, guard: Option<crate::parse::ast::Expr>, body: Vec<BehaviorStmt> },
    Return(crate::parse::ast::Expr),
    Diagnostic { sys: String, args: Vec<crate::parse::ast::Expr> },
    Expr(crate::parse::ast::Expr),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub body: Vec<BehaviorStmt>,
}

impl MatchArm {
    pub fn pattern(&self) -> &Pattern { &self.pat }
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
}

#[derive(Debug, Clone)]
pub struct Behavior {
    pub name: String,
    pub kind: BehaviorKind,
    pub body: Vec<BehaviorStmt>,
}

impl Behavior {
    /// Construct a new Behavior (used by the elaborator and codegen).
    #[doc(hidden)]
    pub fn new(name: String, kind: BehaviorKind, body: Vec<BehaviorStmt>) -> Self {
        Self { name, kind, body }
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn kind(&self) -> &BehaviorKind { &self.kind }
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }

    /// Returns `true` if this is an `analog` behavior block.
    pub fn is_analog(&self) -> bool { matches!(self.kind, BehaviorKind::Analog) }
    /// Returns `true` if this is a `digital` behavior block.
    pub fn is_digital(&self) -> bool { matches!(self.kind, BehaviorKind::Digital) }
}

// ─────────────────────────────── Function ────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, TypeRef)>,
    pub ret: TypeRef,
    pub body: Vec<BehaviorStmt>,
}

impl Function {
    pub fn name(&self) -> &str { &self.name }
    pub fn params(&self) -> &[(String, TypeRef)] { &self.params }
    pub fn ret(&self) -> &TypeRef { &self.ret }
    /// The function body statements.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
}

// ─────────────────────────────── ImplBlock ───────────────────────────────────

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub capability: Option<String>,
    pub ty: String,
    pub const_args: Vec<ConstVal>,
    pub methods: Vec<Function>,
}

impl ImplBlock {
    pub fn capability(&self) -> Option<&str> { self.capability.as_deref() }
    pub fn ty(&self) -> &str { &self.ty }
    pub fn methods(&self) -> &[Function] { &self.methods }
}

// ─────────────────────────────── Design (POM root) ───────────────────────────

/// The complete output of elaboration — the POM root.
///
/// Fields are `pub(crate)`; external consumers use the public accessor
/// methods that implement the POM reflection interface
/// (`docs/reflection_api.md`).
#[derive(Debug, Clone)]
pub struct Design {
    pub modules: HashMap<String, Module>,
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub enums: HashMap<String, EnumDecl>,
    pub capabilities: HashMap<String, CapabilityDecl>,
    pub functions: HashMap<String, Function>,
    pub impls: Vec<ImplBlock>,
    /// Staged parameter overrides — the single mutation surface in POM.
    /// Writing via `set_param()` stages here; re-elaboration consumes.
    pub overrides: Rc<RefCell<OverrideMap>>,
    /// The top module name, if set by the user or inferred.
    pub top_module: Option<String>,
}

impl Design {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            disciplines: HashMap::new(),
            enums: HashMap::new(),
            capabilities: HashMap::new(),
            functions: HashMap::new(),
            impls: Vec::new(),
            overrides: Rc::new(RefCell::new(OverrideMap::new())),
            top_module: None,
        }
    }

    // ── POM navigation ────────────────────────────────────────────────────

    /// The elaborated top module, if set.
    pub fn top(&self) -> Option<&Module> {
        self.top_module.as_ref().and_then(|n| self.modules.get(n))
    }

    /// Set the top module by name.
    pub fn set_top(&mut self, name: &str) {
        self.top_module = Some(name.into());
    }

    /// Look up a module by name.
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.get(name)
    }

    /// Every elaborated (monomorphized) module.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    /// Look up a function by name.
    pub fn function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name)
    }

    /// Every discipline declaration.
    pub fn disciplines(&self) -> impl Iterator<Item = (&String, &DisciplineDecl)> {
        self.disciplines.iter()
    }

    /// Every enum declaration.
    pub fn enums(&self) -> impl Iterator<Item = (&String, &EnumDecl)> {
        self.enums.iter()
    }

    /// Every capability declaration.
    pub fn capabilities(&self) -> impl Iterator<Item = (&String, &CapabilityDecl)> {
        self.capabilities.iter()
    }

    /// Every global function.
    pub fn functions(&self) -> impl Iterator<Item = &Function> {
        self.functions.values()
    }

    /// Every impl block.
    pub fn impls(&self) -> &[ImplBlock] { &self.impls }

    // ── Staging layer ─────────────────────────────────────────────────────

    /// Stage a parameter override. Does NOT mutate the elaborated design —
    /// a subsequent re-elaboration consumes the override purely.
    pub fn set_param(&self, path: &str, param: &str, value: Value) {
        self.overrides.borrow_mut().set(path, param, value);
    }

    /// Look up a staged override.
    pub fn get_override(&self, path: &str, param: &str) -> Option<Value> {
        self.overrides.borrow().get(path, param).cloned()
    }

    /// True if any overrides are staged.
    pub fn has_overrides(&self) -> bool {
        !self.overrides.borrow().is_empty()
    }

    /// Clear all staged overrides.
    pub fn clear_overrides(&self) {
        self.overrides.borrow_mut().clear();
    }

    // ── Internal access (pub(crate)) ──────────────────────────────────────

    /// Internal: mutable access to modules map (for the elaborator).
    pub fn modules_mut(&mut self) -> &mut HashMap<String, Module> {
        &mut self.modules
    }

    /// Insert a module by name. Used by the codegen to build synthetic
    /// modules for digital-only test scenarios.
    #[doc(hidden)]
    pub fn insert_module(&mut self, name: String, module: Module) {
        self.modules.insert(name, module);
    }
}

impl Default for Design {
    fn default() -> Self { Self::new() }
}