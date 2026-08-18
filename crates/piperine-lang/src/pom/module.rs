//! POM structural nodes — [`Port`], [`Param`], [`Wire`], [`Instance`],
//! [`Connection`], and the [`Module`] that owns them.

use crate::value::Value;
use crate::pom::net_type::{NetRef, NetType};
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named, NetTyped};
use crate::pom::{Behavior, ValueType};


// ─────────────────────────────── Attribute ───────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Attribute {
    pub schema: String,
    pub data: std::collections::HashMap<String, Value>,
}

impl Attribute {
    pub fn schema(&self) -> &str { &self.schema }
    pub fn data(&self) -> &std::collections::HashMap<String, Value> { &self.data }
    pub fn field(&self, name: &str) -> Option<&Value> { self.data.get(name) }
}

impl Kinded for Attribute { fn kind(&self) -> Kind { Kind::Attribute } }

// ─────────────────────────────── Port ────────────────────────────────────────

/// Port direction — mirrors `crate::ast::Direction` (LRM §3.7); a
/// separate POM-native type because the AST crate is serde-free by purity
/// contract (T1) and `Port` crosses the serialization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    Input,
    Output,
    Inout,
}

impl From<crate::ast::Direction> for Direction {
    fn from(d: crate::ast::Direction) -> Self {
        match d {
            crate::ast::Direction::Input => Direction::Input,
            crate::ast::Direction::Output => Direction::Output,
            crate::ast::Direction::Inout => Direction::Inout,
        }
    }
}

/// A module port — direction, name, and net type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Port {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub direction: Direction,
    pub name: String,
    pub ty: NetType,
}

impl Port {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    /// The port's declared name.
    pub fn name(&self) -> &str { &self.name }
    /// The port's I/O direction.
    pub fn direction(&self) -> &Direction { &self.direction }
    /// The port's net type (discipline or bus).
    pub fn net_type(&self) -> &NetType { &self.ty }
    /// The port's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
}

impl Named for Port { fn name(&self) -> &str { self.name() } }
impl NetTyped for Port { fn net_type(&self) -> &NetType { self.net_type() } }
impl Kinded for Port { fn kind(&self) -> Kind { Kind::Port } }

// ─────────────────────────────── Param ───────────────────────────────────────

/// A `tol`-declared statistical distribution on a [`Param`] (dv-core DVC-02).
/// Declaration only — sampling the distribution is dv-gradients' scope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Distribution {
    /// The distribution kind (`gauss`, `uniform`). Formerly resolved
    /// against `headers/statistics.phdl`, deleted at T40 (MD-45 — the
    /// `tol`-clause dv-core surface this backs has no VAMS Annex A
    /// production and stays dark until a future phase rebuilds it).
    pub kind: String,
    /// The keyed arguments, evaluated as elaboration constants in module
    /// param scope (e.g. `sigma = 0.005 / sqrt(w * l)`).
    pub args: Vec<(String, Value)>,
    /// `true` for process variation (the `global` marker), `false` for
    /// mismatch (the default).
    pub global: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub name: String,
    pub ty: ValueType,
    pub default: Option<Value>,
    /// For a scalar flattened out of a bundle-typed param (GAPS §I.14):
    /// `(logical param name, bundle type)` — e.g. `model_rsh` carries
    /// `("model", "ResModel")`. Lets the lowering resolve `model.method()`.
    pub bundle_origin: Option<(String, String)>,
    /// The `tol`-declared statistical distribution, if any (dv-core
    /// DVC-01/02) — additive, inert at solve.
    #[serde(default)]
    pub distribution: Option<Distribution>,
    /// The declared `from` ranges (LRM §3.4.2), const-evaluated at
    /// elaboration — the legal domain. Sampling reads this to report a
    /// draw that escapes it (random-variable-catalogue); `exclude`
    /// clauses don't constrain draws (disclosed).
    #[serde(default)]
    pub from_ranges: Vec<FromRange>,
    /// `true` for `localparam` (§3.4.3) — a named constant local to the
    /// module, never overridable at instantiation, as opposed to
    /// `parameter` (`false`, the default). Additive: every pre-VAMS
    /// serialized `Param` deserializes as `parameter` (`false`).
    #[serde(default)]
    pub is_local: bool,
}

/// One declared `from` interval, const-evaluated at elaboration:
/// `[lo, hi]` with per-bound exclusivity.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FromRange {
    pub lo: f64,
    pub lo_exclusive: bool,
    pub hi: f64,
    pub hi_exclusive: bool,
}

impl FromRange {
    /// Whether `v` falls inside the interval.
    pub fn contains(&self, v: f64) -> bool {
        let lo_ok = if self.lo_exclusive { v > self.lo } else { v >= self.lo };
        let hi_ok = if self.hi_exclusive { v < self.hi } else { v <= self.hi };
        lo_ok && hi_ok
    }
}

impl Param {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    pub fn name(&self) -> &str { &self.name }
    pub fn value_type(&self) -> &ValueType { &self.ty }
    pub fn default(&self) -> Option<&Value> { self.default.as_ref() }
    /// The param's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
    /// `true` for `localparam` — not overridable at instantiation (§3.4.3).
    pub fn is_local(&self) -> bool { self.is_local }
    /// The `tol`-declared statistical distribution — kind, evaluated
    /// arguments, and the process (`global`) vs mismatch marker — if the
    /// param carries a `tol` clause (dv-core DVC-02).
    pub fn distribution(&self) -> Option<&Distribution> { self.distribution.as_ref() }

    /// Returns the param's default value, cloned, if it has one. (With the
    /// unified [`Value`] there is no conversion — kept for the existing
    /// reflection-API surface.)
    pub fn value(&self) -> Option<Value> {
        self.default.clone()
    }
}

impl Named for Param { fn name(&self) -> &str { self.name() } }
impl Kinded for Param { fn kind(&self) -> Kind { Kind::Param } }

// ─────────────────────────────── Wire ────────────────────────────────────────

/// A named wire with a net type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Wire {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub name: String,
    pub ty: NetType,
}

impl Wire {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    /// The wire's declared name.
    pub fn name(&self) -> &str { &self.name }
    /// The wire's net type (discipline or bus).
    pub fn net_type(&self) -> &NetType { &self.ty }
    /// The wire's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
}

impl Named for Wire { fn name(&self) -> &str { self.name() } }
impl NetTyped for Wire { fn net_type(&self) -> &NetType { self.net_type() } }
impl Kinded for Wire { fn kind(&self) -> Kind { Kind::Wire } }

// ─────────────────────────────── Instance ────────────────────────────────────

/// A submodule instance — label, module name, port bindings, and params.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Instance {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    /// Byte span of just the label token (`src` in `src : Type(...)`), if
    /// the instance is labeled (LSB-07..10).
    #[serde(skip)]
    pub label_span: Option<miette::SourceSpan>,
    /// Byte span of just the module-type-identifier token (LSB-07..10).
    #[serde(skip)]
    pub type_span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub label: Option<String>,
    pub module: String,
    pub ports: Vec<NetRef>,
    pub params: Vec<(String, Value)>,
    /// `name [msb:lsb] (...)` — an array-of-instances range (§6.2.2,
    /// LRMC-C5-08), `(msb, lsb)`. Authored form only (MD-25): stays a
    /// single `Instance` with this field set; `FlattenHierarchy` expands
    /// it into `msb.abs_diff(lsb) + 1` leaf instances in `flat_modules`
    /// only, never here.
    #[serde(default)]
    pub range: Option<(i64, i64)>,
}

impl Instance {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    /// The instance's label if set, otherwise the module name.
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.module)
    }
    /// The name of the module this instance instantiates.
    pub fn module_name(&self) -> &str { &self.module }
    /// Port bindings for this instance.
    pub fn ports(&self) -> &[NetRef] { &self.ports }
    /// Parameter assignments for this instance.
    pub fn params(&self) -> &[(String, Value)] { &self.params }
    /// The instance label, if one was given.
    pub fn label(&self) -> Option<&str> { self.label.as_deref() }
    /// The instance's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
}

impl Named for Instance { fn name(&self) -> &str { self.name() } }
impl Kinded for Instance { fn kind(&self) -> Kind { Kind::Instance } }

// ─────────────────────────────── Var ─────────────────────────────────────────

/// A module-level persistent variable (GAPS §I.15), e.g. `var sw_state :
/// Real = 0.0;` in a switch's `mod` body. Unlike a `var` declared inside an
/// `analog`/`digital` block (which is inlined at lowering time), a
/// module-level `var` survives across evaluations — it is the PHDL
/// equivalent of a C `static` local, used for things like hysteresis state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Var {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub name: String,
    pub ty: ValueType,
    pub init: Option<Value>,
}

impl Var {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    pub fn name(&self) -> &str { &self.name }
    pub fn value_type(&self) -> &ValueType { &self.ty }
    pub fn init(&self) -> Option<&Value> { self.init.as_ref() }
    /// The var's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
}

impl Named for Var { fn name(&self) -> &str { self.name() } }
impl Kinded for Var { fn kind(&self) -> Kind { Kind::Var } }

// ─────────────────────────────── Connection ──────────────────────────────────

/// A named connection between two net references.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Connection {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub lhs: NetRef,
    pub rhs: NetRef,
}

impl Connection {
    /// The left-hand side net reference.
    pub fn lhs(&self) -> &NetRef { &self.lhs }
    /// The right-hand side net reference.
    pub fn rhs(&self) -> &NetRef { &self.rhs }
}

// ─────────────────────────────── Module ──────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Module {
    #[serde(skip)]
    pub span: Option<miette::SourceSpan>,
    pub attributes: Vec<Attribute>,
    /// Attached `///` doc comment, if any (MD-25 — additive, LSP-07).
    #[serde(default)]
    pub doc: Option<String>,
    pub name: String,
    pub ports: Vec<Port>,
    pub params: Vec<Param>,
    pub wires: Vec<Wire>,
    pub vars: Vec<Var>,
    pub instances: Vec<Instance>,
    pub connections: Vec<Connection>,
    /// Behavior bodies are compiled ASTs, not reflection data — they
    /// do not cross a serialization boundary (SPEC Part IV §7).
    #[serde(skip)]
    pub behaviors: Vec<Behavior>,
    /// Verilog-AMS `branch (p,n) name;` declarations (LRM §3.12), plus
    /// implicit branches created by access-function call sites. Authored
    /// form — the elaborator records each branch's kind from the first
    /// contribution seen (T23). Skipped on serialize, same rule as
    /// behaviors: branches reference compiled access-function call sites.
    #[serde(skip, default)]
    pub branches: Vec<crate::pom::Branch>,
    /// Verilog-AMS `genvar` names declared in this module (LRM §3.5) —
    /// loop indices for `generate` regions. Authored form; generated
    /// instances land in `flat_modules`, never here (MD-25 VAMS clause,
    /// design §5.3).
    #[serde(default)]
    pub genvars: Vec<String>,
    /// Verilog-AMS `event` names declared in this module (LRM §5.10.4,
    /// Annex A.2.1.3) — the `-> name;` trigger and `@(name)` sensitivity
    /// resolve against this list. Authored form (MD-25), same simplicity
    /// as `genvars` above — LRMC-C5-04.
    #[serde(default)]
    pub events: Vec<String>,
    /// Authored constraint statements (`require`/`var`/`target`/`cover`/
    /// `cover cross`) of this module's `constraint begin … end` block(s)
    /// (sanctioned extension, MD-49 + `constraint-statements`), in
    /// authored order across all blocks — multiple blocks compose, none
    /// dropped, and the order crosses kinds (`target#<n>` is
    /// statement-index-based, `var` substitution is authored-order).
    /// Authored surface (design F1); skipped on serialize, same rule as
    /// behaviors: expressions are compiled.
    #[serde(skip, default)]
    pub constraints: Vec<crate::pom::PomConstraintStmt>,
    /// The `` `timescale `` in force for this module (1364-2005): the
    /// last directive at or before the module's start. `None` when no
    /// directive precedes it — delays then carry raw simulation-time
    /// units (seconds). Recorded per module so gate `#delay` values
    /// scale by the unit and round to the precision grid.
    #[serde(default)]
    pub timescale: Option<crate::directives::Timescale>,
    /// Gate-primitive instances (`and g1(y, a, b);`, Annex A.4.2,
    /// LRMC-C7-13, T39) — a gate is not a module instance (no port
    /// declarations to resolve against), so it gets its own list rather
    /// than joining `instances`. Authored form (MD-25); a UDP instance
    /// (`primitive`-declared, syntactically identical to a module
    /// instantiation) lands in `instances` instead, distinguished
    /// downstream by `Design::udp(&inst.module).is_some()`.
    #[serde(default)]
    pub gates: Vec<GateInstance>,
    pub origin: Option<String>,
}

/// One gate-primitive instance lowered into the POM (Annex A.4.2,
/// LRMC-C7-13, T39). `kind` reuses `ast::GateKind` directly — a mirror
/// type would add nothing (the same closed keyword set, no elaboration-
/// time transformation happens to it).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GateInstance {
    pub name: Option<String>,
    pub kind: crate::ast::GateKind,
    pub terminals: Vec<NetRef>,
    /// `#delay`, if present — a single scalar (Annex A.4.2's `delay2`/
    /// `delay3` rise/fall/turn-off triad collapses to one value; finer
    /// per-transition delay is out of this task's scope, same
    /// simplification the grammar's own `parse_gate_instantiation` doc
    /// comment already made for drive strength).
    pub delay: Option<f64>,
}

impl Module {
    pub fn attributes(&self) -> &[Attribute] { &self.attributes }
    /// Construct a new Module (used by the elaborator and codegen).
    /// Module-level `var`s are empty; use struct-literal construction if
    /// the module declares persistent state.
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
        Self {
            span: None, attributes: Vec::new(), doc: None, name,
            ports, params, wires, vars: Vec::new(), instances, connections,
            behaviors,
            branches: Vec::new(), genvars: Vec::new(), events: Vec::new(),
            constraints: Vec::new(), timescale: None, gates: Vec::new(), origin: None,
        }
    }

    /// The module's name.
    pub fn name(&self) -> &str { &self.name }
    /// The module's attached `///` doc comment, if any.
    pub fn doc(&self) -> Option<&str> { self.doc.as_deref() }
    /// Returns `false` — always false post-monomorphization.
    pub fn is_generic(&self) -> bool { false }

    /// All ports of the module.
    pub fn ports(&self) -> &[Port] { &self.ports }
    /// All params of the module.
    pub fn params(&self) -> &[Param] { &self.params }
    /// All wires of the module.
    pub fn wires(&self) -> &[Wire] { &self.wires }
    /// All module-level persistent variables (GAPS §I.15).
    pub fn vars(&self) -> &[Var] { &self.vars }
    /// All submodule instances of the module.
    pub fn instances(&self) -> &[Instance] { &self.instances }
    /// All named connections of the module.
    pub fn connections(&self) -> &[Connection] { &self.connections }
    /// All behavior blocks of the module.
    pub fn behaviors(&self) -> &[Behavior] { &self.behaviors }
    /// All Verilog-AMS branches declared or implied in this module
    /// (LRM §3.12). Authored form; `flat_modules` carries the
    /// post-monormophization shape (MD-25 VAMS clause).
    pub fn branches(&self) -> &[crate::pom::Branch] { &self.branches }
    /// All `genvar` names declared in this module (LRM §3.5).
    pub fn genvars(&self) -> &[String] { &self.genvars }
    /// All `event` names declared in this module (LRM §5.10.4).
    pub fn events(&self) -> &[String] { &self.events }
    /// All authored constraint statements of this module's `constraint`
    /// block(s), in authored order across kinds (multiple blocks compose).
    pub fn constraints(&self) -> &[crate::pom::PomConstraintStmt] { &self.constraints }

    /// Look up a port by name.
    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|p| p.name == name)
    }
    /// Look up a param by name.
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }
    /// Look up a wire by name.
    pub fn wire(&self, name: &str) -> Option<&Wire> {
        self.wires.iter().find(|w| w.name == name)
    }
    /// Look up an instance by label.
    pub fn instance(&self, name: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.label.as_deref() == Some(name))
    }
    /// Look up a named branch by name. Implicit branches (no authored
    /// name) are not reachable here — they are returned by the
    /// elaboration-driven access-function call-site resolver (T23).
    pub fn branch(&self, name: &str) -> Option<&crate::pom::Branch> {
        self.branches.iter().find(|b| b.name.as_deref() == Some(name))
    }
}

impl Named for Module { fn name(&self) -> &str { self.name() } }
impl Kinded for Module { fn kind(&self) -> Kind { Kind::Module } }
