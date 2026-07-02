//! POM `Node`, `Id`, and `Kind` — the discriminated supertype and identity.

use std::fmt;

/// A stable node identity that survives re-elaboration as long as the
/// source construct is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u64);

impl Id {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// The kind of a POM node — how `Node` discriminates its concrete type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Module,
    Instance,
    Port,
    Param,
    Wire,
    Var,
    Behavior,
    Discipline,
    Enum,
    Bundle,
    Capability,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Module => write!(f, "module"),
            Self::Instance => write!(f, "instance"),
            Self::Port => write!(f, "port"),
            Self::Param => write!(f, "param"),
            Self::Wire => write!(f, "wire"),
            Self::Var => write!(f, "var"),
            Self::Behavior => write!(f, "behavior"),
            Self::Discipline => write!(f, "discipline"),
            Self::Enum => write!(f, "enum"),
            Self::Bundle => write!(f, "bundle"),
            Self::Capability => write!(f, "capability"),
        }
    }
}

// `Node` and the concrete node types are defined in `module.rs` and
// `defn.rs` after the Elab* types are renamed. For now, `Node` is a
// forward declaration that will be filled in Step 3+.
// The concrete node types (`Module`, `Instance`, etc.) will be the
// renamed `Elab*` types themselves — not wrappers — per the user's
// decision to "rename ElabProgram → Design".

use crate::pom::traits::{Kinded, Named};

/// A reference to any node in the POM graph.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    Module(&'a crate::pom::Module),
    Instance(&'a crate::pom::Instance),
    Port(&'a crate::pom::Port),
    Param(&'a crate::pom::Param),
    Wire(&'a crate::pom::Wire),
    Behavior(&'a crate::pom::Behavior),
}

impl<'a> PartialEq for Node<'a> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Node::Module(a), Node::Module(b)) => std::ptr::eq(*a, *b),
            (Node::Instance(a), Node::Instance(b)) => std::ptr::eq(*a, *b),
            (Node::Port(a), Node::Port(b)) => std::ptr::eq(*a, *b),
            (Node::Param(a), Node::Param(b)) => std::ptr::eq(*a, *b),
            (Node::Wire(a), Node::Wire(b)) => std::ptr::eq(*a, *b),
            (Node::Behavior(a), Node::Behavior(b)) => std::ptr::eq(*a, *b),
            _ => false,
        }
    }
}

impl<'a> Kinded for Node<'a> {
    fn kind(&self) -> Kind {
        match self {
            Node::Module(_) => Kind::Module,
            Node::Instance(_) => Kind::Instance,
            Node::Port(_) => Kind::Port,
            Node::Param(_) => Kind::Param,
            Node::Wire(_) => Kind::Wire,
            Node::Behavior(_) => Kind::Behavior,
        }
    }
}

impl<'a> Node<'a> {
    pub fn select(&self, design: &'a crate::pom::design::Design, path: &str) -> Result<crate::pom::selection::NodeSelection<'a>, String> {
        let sel = path.parse::<crate::pom::selector::Selector>()?;
        crate::pom::selector::Evaluator::new(design).evaluate(&sel, crate::pom::selection::NodeSelection::from_vec(vec![*self]))
    }
}

// Conversions from specific types to Node
impl<'a> From<&'a crate::pom::Module> for Node<'a> {
    fn from(m: &'a crate::pom::Module) -> Self {
        Node::Module(m)
    }
}

impl<'a> From<&'a crate::pom::Instance> for Node<'a> {
    fn from(i: &'a crate::pom::Instance) -> Self {
        Node::Instance(i)
    }
}

impl<'a> From<&'a crate::pom::Port> for Node<'a> {
    fn from(p: &'a crate::pom::Port) -> Self {
        Node::Port(p)
    }
}

impl<'a> From<&'a crate::pom::Param> for Node<'a> {
    fn from(p: &'a crate::pom::Param) -> Self {
        Node::Param(p)
    }
}

impl<'a> From<&'a crate::pom::Wire> for Node<'a> {
    fn from(w: &'a crate::pom::Wire) -> Self {
        Node::Wire(w)
    }
}

impl<'a> From<&'a crate::pom::Behavior> for Node<'a> {
    fn from(b: &'a crate::pom::Behavior) -> Self {
        Node::Behavior(b)
    }
}

// Conversions from Node to specific types
impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Module {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Module(m) = node {
            Ok(m)
        } else {
            Err("Node is not a Module")
        }
    }
}

impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Instance {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Instance(i) = node {
            Ok(i)
        } else {
            Err("Node is not an Instance")
        }
    }
}

impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Port {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Port(p) = node {
            Ok(p)
        } else {
            Err("Node is not a Port")
        }
    }
}

impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Param {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Param(p) = node {
            Ok(p)
        } else {
            Err("Node is not a Param")
        }
    }
}

impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Wire {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Wire(w) = node {
            Ok(w)
        } else {
            Err("Node is not a Wire")
        }
    }
}

impl<'a> TryFrom<Node<'a>> for &'a crate::pom::Behavior {
    type Error = &'static str;
    fn try_from(node: Node<'a>) -> Result<Self, Self::Error> {
        if let Node::Behavior(b) = node {
            Ok(b)
        } else {
            Err("Node is not a Behavior")
        }
    }
}

impl<'a> Named for Node<'a> {
    fn name(&self) -> &str {
        match self {
            Node::Module(m) => m.name(),
            Node::Instance(i) => i.name(),
            Node::Port(p) => p.name(),
            Node::Param(p) => p.name(),
            Node::Wire(w) => w.name(),
            Node::Behavior(_) => "", // Behaviors do not have a name
        }
    }
}
