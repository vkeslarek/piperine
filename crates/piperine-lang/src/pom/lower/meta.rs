//! Doc comments and attributes: one helper for every construct that carries
//! them. Attribute paths and arguments are lowered like any path or
//! expression and are never resolved.

use super::Lowerer;
use super::body::name;
use crate::msl::ast;
use crate::pom::meta::{AttrArgNode, AttrInputNode, AttrNode, Doc, Meta};

pub(super) fn doc(d: &ast::Doc) -> Doc {
    Doc { text: d.text.clone(), block: d.block, span: d.span.0 }
}

impl Lowerer<'_> {
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
                                ast::AttrArg::NameValue(n, l) => AttrArgNode::NameValue(name(n), l.clone()),
                            })
                            .collect(),
                    ),
                };
                AttrNode { path, input, span: a.span.0 }
            })
            .collect();
        Meta { docs: meta.docs.iter().map(doc).collect(), attrs }
    }
}
