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