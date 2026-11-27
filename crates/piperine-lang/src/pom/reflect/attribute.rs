//! The handle over an attribute of a declaration.

use super::{Decl, Expr, Path, Reflect};
use crate::msl::Span;
use crate::pom::body::Lit;
use crate::pom::meta::{AttrArgNode, AttrInputNode, AttrNode};

/// An attribute: `#[path]`, `#[path(args)]` or `#[path = expr]`.
#[derive(Clone, Copy)]
pub struct Attr<'d> {
    owner: Decl<'d>,
    node: &'d AttrNode,
}

impl<'d> Attr<'d> {
    /// The attribute `node` written on `owner`.
    pub(super) fn new(owner: Decl<'d>, node: &'d AttrNode) -> Self {
        Attr { owner, node }
    }

    pub fn node(self) -> &'d AttrNode {
        self.node
    }

    /// The path as written, segments joined by `::`.
    pub fn name(self) -> String {
        let path = self.path();
        path.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::")
    }

    /// The attribute's path node; its resolution stays `Unresolved`.
    pub fn path(self) -> Path<'d> {
        self.owner.design().path(self.node.path)
    }

    pub fn span(self) -> Span {
        self.node.span
    }

    /// The authored text, `#[...]` included.
    pub fn text(self) -> &'d str {
        self.owner.file().text().get(self.node.span.range()).unwrap_or("")
    }

    pub fn input(self) -> &'d AttrInputNode {
        &self.node.input
    }

    /// The `= expr` value of `#[path = expr]`.
    pub fn value(self) -> Option<Expr<'d>> {
        match &self.node.input {
            AttrInputNode::Eq(e) => Some(self.owner.design().expr(*e)),
            AttrInputNode::None | AttrInputNode::List(_) => None,
        }
    }

    /// The literal arguments written without a name, in order.
    pub fn positional(self) -> Vec<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Lit(l) => Some(l),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The path arguments, as in `#[derive(Repr, Clone)]`, in order.
    pub fn paths(self) -> Vec<Path<'d>> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Path(p) => Some(self.owner.design().path(*p)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The value of the argument `name = lit`.
    pub fn named(self, name: &str) -> Option<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args.iter().find_map(|a| match a {
                AttrArgNode::NameValue(n, l) if n.text == name => Some(l),
                _ => None,
            }),
            _ => None,
        }
    }
}

impl std::fmt::Debug for Attr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attr({})", self.name())
    }
}
