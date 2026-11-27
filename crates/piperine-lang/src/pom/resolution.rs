//! The values later stages write into the slots of body nodes. Every slot
//! starts as `Unresolved` / `Unclassified` / `Unknown`: lowering decides
//! nothing about names.

use serde::{Deserialize, Serialize};

use super::builtin::BuiltinId;
use super::ids::DeclId;

/// What a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Resolution {
    /// Not looked at yet, or nothing has that name (then `E0161`).
    #[default]
    Unresolved,
    /// A declaration of this design.
    Decl(DeclId),
    /// An entry of the built-in table.
    Builtin(BuiltinId),
    /// Needs type inference; no diagnostic.
    Deferred(DeferReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeferReason {
    ReceiverTypeUnknown,
    AmbiguousAccess,
    /// An associated name written through a generic bounded by a catalogue trait.
    BuiltinTraitMember,
}

/// What a call expression is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CallClass {
    #[default]
    Unclassified,
    Function,
    /// A module instantiation; `via` is the template it was written through.
    Instance { module: Option<DeclId>, via: Option<DeclId> },
    /// A call whose callee is a nature quantity (`v(p, n)`) or the built-in
    /// `potential` / `flow`.
    Probe,
}

/// What a `Path { ... }` expression is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StructClass {
    #[default]
    Unclassified,
    StructLiteral,
    BundleLiteral,
    Template(DeclId),
}

/// The kind of a module port, from its declared type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PortKind {
    #[default]
    Unknown,
    Carrier,
    Bundle,
    ValueInput,
}

/// The kind of a nature, from the fields it declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NatureKind {
    Discrete,
    Conservative,
    SignalFlow,
    Invalid,
}
