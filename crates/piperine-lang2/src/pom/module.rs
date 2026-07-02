//! POM structural nodes — [`Port`], [`Param`], [`Wire`], [`Instance`],
//! [`Connection`], and the [`Module`] that owns them.

use crate::elab::const_eval::ConstVal;
use crate::pom::net_type::{NetRef, NetType};
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named, NetTyped};
use crate::pom::{Behavior, Value, ValueType};

// ─────────────────────────────── Port ────────────────────────────────────────

/// A module port — direction, name, and net type.
#[derive(Debug, Clone)]
pub struct Port {
    pub direction: crate::parse::ast::Direction,
    pub name: String,
    pub ty: NetType,
}

impl Port {
    /// The port's declared name.
    pub fn name(&self) -> &str { &self.name }
    /// The port's I/O direction.
    pub fn direction(&self) -> &crate::parse::ast::Direction { &self.direction }
    /// The port's net type (discipline or bus).
    pub fn net_type(&self) -> &NetType { &self.ty }
}

impl Named for Port { fn name(&self) -> &str { self.name() } }
impl NetTyped for Port { fn net_type(&self) -> &NetType { self.net_type() } }
impl Kinded for Port { fn kind(&self) -> Kind { Kind::Port } }

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
        self.default.as_ref().map(Value::from)
    }
}

impl Named for Param { fn name(&self) -> &str { self.name() } }
impl Kinded for Param { fn kind(&self) -> Kind { Kind::Param } }

// ─────────────────────────────── Wire ────────────────────────────────────────

/// A named wire with a net type.
#[derive(Debug, Clone)]
pub struct Wire {
    pub name: String,
    pub ty: NetType,
}

impl Wire {
    /// The wire's declared name.
    pub fn name(&self) -> &str { &self.name }
    /// The wire's net type (discipline or bus).
    pub fn net_type(&self) -> &NetType { &self.ty }
}

impl Named for Wire { fn name(&self) -> &str { self.name() } }
impl NetTyped for Wire { fn net_type(&self) -> &NetType { self.net_type() } }
impl Kinded for Wire { fn kind(&self) -> Kind { Kind::Wire } }

// ─────────────────────────────── Instance ────────────────────────────────────

/// A submodule instance — label, module name, port bindings, and params.
#[derive(Debug, Clone)]
pub struct Instance {
    pub label: Option<String>,
    pub module: String,
    pub ports: Vec<NetRef>,
    pub params: Vec<(String, ConstVal)>,
}

impl Instance {
    /// The instance's label if set, otherwise the module name.
    pub fn name(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.module)
    }
    /// The name of the module this instance instantiates.
    pub fn module_name(&self) -> &str { &self.module }
    /// Port bindings for this instance.
    pub fn ports(&self) -> &[NetRef] { &self.ports }
    /// Parameter assignments for this instance.
    pub fn params(&self) -> &[(String, ConstVal)] { &self.params }
    /// The instance label, if one was given.
    pub fn label(&self) -> Option<&str> { self.label.as_deref() }
}

impl Named for Instance { fn name(&self) -> &str { self.name() } }
impl Kinded for Instance { fn kind(&self) -> Kind { Kind::Instance } }

// ─────────────────────────────── Var ─────────────────────────────────────────

/// A module-level persistent variable (GAPS §I.15), e.g. `var sw_state :
/// Real = 0.0;` in a switch's `mod` body. Unlike a `var` declared inside an
/// `analog`/`digital` block (which is inlined at lowering time), a
/// module-level `var` survives across evaluations — it is the PHDL
/// equivalent of a C `static` local, used for things like hysteresis state.
#[derive(Debug, Clone)]
pub struct Var {
    pub name: String,
    pub ty: ValueType,
    pub init: Option<ConstVal>,
}

impl Var {
    pub fn name(&self) -> &str { &self.name }
    pub fn value_type(&self) -> &ValueType { &self.ty }
    pub fn init(&self) -> Option<&ConstVal> { self.init.as_ref() }
}

impl Named for Var { fn name(&self) -> &str { self.name() } }
impl Kinded for Var { fn kind(&self) -> Kind { Kind::Var } }

// ─────────────────────────────── Connection ──────────────────────────────────

/// A named connection between two net references.
#[derive(Debug, Clone)]
pub struct Connection {
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

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub ports: Vec<Port>,
    pub params: Vec<Param>,
    pub wires: Vec<Wire>,
    /// Module-level persistent variables (GAPS §I.15). Empty unless the
    /// `mod` body declares `var`s directly (as opposed to `var`s inside an
    /// `analog`/`digital` block, which are local and inlined at lowering).
    pub vars: Vec<Var>,
    pub instances: Vec<Instance>,
    pub connections: Vec<Connection>,
    pub behaviors: Vec<Behavior>,
}

impl Module {
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
        Self { name, ports, params, wires, vars: Vec::new(), instances, connections, behaviors }
    }

    /// The module's name.
    pub fn name(&self) -> &str { &self.name }
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
}

impl Named for Module { fn name(&self) -> &str { self.name() } }
impl Kinded for Module { fn kind(&self) -> Kind { Kind::Module } }
