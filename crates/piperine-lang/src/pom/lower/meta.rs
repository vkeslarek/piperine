//! Doc comments and attributes: one helper for every construct that carries
//! them. Attribute paths and arguments are lowered like any path or
//! expression and are never resolved, except the names in `#[derive(..)]`, which `resolve` looks up as traits (E0161).

use super::Lowerer;
use crate::msl::ast;
use crate::pom::ids::FileId;
use crate::pom::meta::{AttrArgNode, AttrInputNode, AttrNode, Doc, Meta};

impl Lowerer<'_> {
    /// A doc comment, as an inner (`//!`) or outer one of `file`.
    pub(super) fn doc(d: &ast::Doc, inner: bool, file: FileId) -> Doc {
        Doc { text: d.text.clone(), block: d.block, inner, file, span: d.span.0 }
    }

    /// The docs and attributes of one construct; nodes inside an attribute
    /// are owned by the current owner.
    pub(crate) fn meta(&mut self, meta: &ast::Meta) -> Meta {
        let attrs = meta
            .attrs
            .iter()
            .map(|a| {
                let path = self.path(&a.path);
                let input = match &a.input {
                    ast::AttrInput::None => AttrInputNode::None,
                    ast::AttrInput::Eq(e) => AttrInputNode::Eq(self.expr(e)),
                    ast::AttrInput::List(args) => AttrInputNode::List(
                        args.iter()
                            .map(|arg| match arg {
                                ast::AttrArg::Path(p) => AttrArgNode::Path(self.path(p)),
                                ast::AttrArg::Lit(l) => AttrArgNode::Lit(l.clone()),
                                ast::AttrArg::NameValue(n, l) => AttrArgNode::NameValue(Self::name(n), l.clone()),
                            })
                            .collect(),
                    ),
                };
                AttrNode { path, input, span: a.span.0 }
            })
            .collect();
        Meta { docs: meta.docs.iter().map(|d| Self::doc(d, false, self.file)).collect(), attrs }
    }
}
