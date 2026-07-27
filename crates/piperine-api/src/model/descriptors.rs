//! The reflected children of a `Module`: one owned
//! snapshot per authored declaration.
//!
//! Each descriptor is a snapshot taken when the enumerating accessor
//! (`Module::ports()`, `Module::instances()`, …) walks the POM. The model is
//! read-only, so a snapshot is both an honest reflection of the design at
//! enumeration time and free of the borrow the POM's own `&Port`/`&Wire`
//! references would impose on every caller.
//!
//! Typed values stay typed here — `Direction`, `ValueType`, `BehaviorKind` and
//! `Value` are handed through as the POM spells them. Rendering them as strings
//! is a binding concern and belongs to the host that needs strings (the Python
//! wrappers do exactly that, the way `_TerminalDescriptor` already does for the
//! solver's descriptor enums).

use piperine_lang::parse::ast::{BehaviorKind, Direction};
use piperine_lang::{Value, ValueType};

/// A reflected port — its name, direction, and net (discipline) type.
#[derive(Debug, Clone)]
pub struct Port {
    name: String,
    direction: Direction,
    ty: String,
}

impl Port {
    /// Snapshot a POM port.
    pub fn of(port: &piperine_lang::Port) -> Self {
        Self {
            name: port.name().to_string(),
            direction: port.direction().clone(),
            ty: port.net_type().discipline_name().to_string(),
        }
    }

    /// The port's declared name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The declared direction (`in`/`out`/`inout`).
    pub fn direction(&self) -> &Direction {
        &self.direction
    }

    /// The net (discipline) type, e.g. `"Electrical"`.
    pub fn ty(&self) -> &str {
        &self.ty
    }
}

/// A reflected net (a module's `wire` declaration) — name and discipline type.
#[derive(Debug, Clone)]
pub struct Net {
    name: String,
    ty: String,
}

impl Net {
    /// Snapshot a POM wire.
    pub fn of(wire: &piperine_lang::Wire) -> Self {
        Self {
            name: wire.name().to_string(),
            ty: wire.net_type().discipline_name().to_string(),
        }
    }

    /// The net's declared name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The net (discipline) type, e.g. `"Electrical"`.
    pub fn ty(&self) -> &str {
        &self.ty
    }
}

/// A reflected submodule instance — the label the author wrote and the module
/// it instantiates. Walking `Design::module(instance.module())` from here is how
/// the authored hierarchy is descended (MD-25).
#[derive(Debug, Clone)]
pub struct Instance {
    name: String,
    module: String,
}

impl Instance {
    /// Snapshot a POM instance.
    pub fn of(inst: &piperine_lang::Instance) -> Self {
        Self {
            name: inst.name().to_string(),
            module: inst.module_name().to_string(),
        }
    }

    /// The instance label (or the module name when unlabeled).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The name of the module this instance instantiates — the next step down
    /// the authored hierarchy.
    pub fn module(&self) -> &str {
        &self.module
    }
}

/// A reflected param — name, declared value type, and default value.
#[derive(Debug, Clone)]
pub struct Param {
    name: String,
    ty: ValueType,
    default: Option<Value>,
}

impl Param {
    /// Snapshot a POM param.
    pub fn of(param: &piperine_lang::Param) -> Self {
        Self {
            name: param.name().to_string(),
            ty: param.value_type().clone(),
            default: param.default().cloned(),
        }
    }

    /// The param's declared name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The declared value type.
    pub fn ty(&self) -> &ValueType {
        &self.ty
    }

    /// The pre-folded default value, or `None` when the param has none.
    pub fn default(&self) -> Option<&Value> {
        self.default.as_ref()
    }
}

/// A reflected `analog`/`digital` behavior block.
#[derive(Debug, Clone)]
pub struct Behavior {
    name: String,
    kind: BehaviorKind,
}

impl Behavior {
    /// Snapshot a POM behavior block.
    pub fn of(beh: &piperine_lang::Behavior) -> Self {
        Self {
            name: beh.name().to_string(),
            kind: beh.kind().clone(),
        }
    }

    /// The name of the module the block belongs to.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the block is `analog` or `digital`.
    pub fn kind(&self) -> &BehaviorKind {
        &self.kind
    }
}

/// One entry in an `InstanceView`'s terminal
/// connectivity list: a port on the instance's module declaration and the
/// parent-scope net it is wired to.
#[derive(Debug, Clone)]
pub struct Terminal {
    port: String,
    net: String,
}

impl Terminal {
    /// Pair a port name with the net it connects to.
    pub fn new(port: String, net: String) -> Self {
        Self { port, net }
    }

    /// The port name on the instance's module declaration.
    pub fn port(&self) -> &str {
        &self.port
    }

    /// The parent-scope net name this terminal connects to — the name a
    /// voltage/current readout on the parent result takes.
    pub fn net(&self) -> &str {
        &self.net
    }
}
