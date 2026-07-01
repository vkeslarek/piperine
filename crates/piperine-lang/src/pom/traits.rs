//! POM capability traits.
//!
//! Small, orthogonal traits — the "capabilities" of `docs/piperine-hdl-spec.md`
//! §6.6 applied to the object model itself. Each trait is one axis a node may
//! or may not have, so generic code (the future selector, a plugin walking
//! the graph) can be written against the capability instead of the concrete
//! type: `fn print_all(items: &[impl Named])` reads the same whether it's
//! walking ports or instances.
//!
//! A trait is added here only once at least two node types implement it —
//! a trait with one implementor is a rename, not a capability.
//!
//! [`Kinded`] is the "which concrete node type is this" axis of the
//! `Node` capability in `docs/reflection_api.md` §1.1. The other axis,
//! stable identity (`id()`), needs the elaborator to assign [`Id`]s to
//! every node — deferred until the selector work lands (see the refactor
//! plan's out-of-scope list).

use crate::pom::net_type::NetType;
use crate::pom::node::Kind;

/// A node with a plain-text name — the common case for every POM node
/// except value-layer leaves (`Value`, `NetRef`).
pub trait Named {
    fn name(&self) -> &str;
}

/// A node typed by a [`NetType`] — a discipline or net-capable bundle.
/// Implemented by [`Port`][super::Port] and [`Wire`][super::Wire].
pub trait NetTyped {
    fn net_type(&self) -> &NetType;
}

/// A node's discriminant — which concrete POM type it is. See the module
/// doc comment for how this relates to the full `Node` capability.
pub trait Kinded {
    fn kind(&self) -> Kind;
}
