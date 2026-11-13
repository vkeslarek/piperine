//! Stage 4: each behavior block is bound to the module it names. The target
//! must be a module of the same file (`E0401`), not `extern` (`E0402`), and
//! the block's generic names must be the module's (`E0403`).

use std::collections::BTreeMap;

use super::super::body::{GenericArgNode, GenericArgsNode, TypeKind};
use super::super::decl::{BehaviorData, DeclKind};
use super::super::ids::DeclId;
use super::super::resolution::Resolution;
use super::imports::{Prefer, WalkError, path_diagnostic};
use super::{Ctx, Output};
use crate::msl::{Diagnostic, Span};

impl Ctx<'_> {
    /// The module each behavior block belongs to, by block. Records the
    /// resolution of every target path and reports the binding errors.
    pub(super) fn bind_behaviors(&self, out: &mut Output) -> BTreeMap<DeclId, DeclId> {
        let design = self.design;
        let mut links = BTreeMap::new();
        for i in 0..design.decl_count() {
            let id = DeclId(i as u32);
            let data = design.decl_data(id);
            let DeclKind::Behavior(block) = &data.kind else { continue };
            let Some(ns) = self.parent_namespace(id) else { continue };
            let node = design.path_node(block.target);
            let walked = self.walk(&node.segments, ns, Prefer::Types, &mut Vec::new(), &mut |this, name, visiting| this.scope_lookup(ns, name, Prefer::Types, visiting));
            for (index, res) in walked.res.iter().enumerate() {
                out.segments.push((block.target, index, *res));
            }
            let mut report = |diagnostic: Diagnostic| out.diagnostics.push((data.file, diagnostic));
            let target = match (walked.error, walked.res.last()) {
                (Some(WalkError::Missing { .. }), _) => None,
                (error, Some(Resolution::Decl(d))) => {
                    if let Some(private @ WalkError::Private { .. }) = error {
                        report(path_diagnostic(&node.segments, node.span, private));
                    }
                    Some(*d)
                }
                _ => None,
            };
            let text = node.segments.iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::");
            let Some(target) = target else {
                let message = format!("cannot find a module `{text}` for this behavior block");
                report(Diagnostic::new("E0401", message, node.span, "no such module"));
                continue;
            };
            let DeclKind::Module(module) = &design.decl_data(target).kind else {
                let message = format!("`{text}` is not a module");
                report(Diagnostic::new("E0401", message, node.span, "not a module"));
                continue;
            };
            if design.decl_data(target).file != data.file {
                let message = format!("`{text}` is declared in another file: a module and its behavior blocks share one file (§3.4.3)");
                report(Diagnostic::new("E0401", message, node.span, "module in another file"));
                continue;
            }
            if module.abi.is_some() {
                let message = format!("`{text}` is an `extern` module and takes no behavior block");
                report(Diagnostic::new("E0402", message, node.span, "extern module"));
            }
            if !self.names_match(block, target) {
                let head = Span::new(data.span.start, node.span.end);
                let message = format!("the generic names of this block are not those of module `{text}`");
                report(Diagnostic::new("E0403", message, head, "generic names differ"));
            }
            links.insert(id, target);
        }
        links
    }

    /// Are the block's names, and the generic arguments of its target, exactly
    /// the generic parameters of `module`, in order?
    pub(super) fn names_match(&self, block: &BehaviorData, module: DeclId) -> bool {
        let design = self.design;
        let generics: Vec<&str> = design
            .decl_data(module)
            .children
            .iter()
            .map(|c| design.decl_data(*c))
            .filter(|d| matches!(d.kind, DeclKind::GenericParam(_)))
            .filter_map(|d| d.name.as_ref().map(|n| n.text.as_str()))
            .collect();
        let names: Vec<&str> = block.names.iter().map(|n| n.text.as_str()).collect();
        if names != generics {
            return false;
        }
        let args = design.path_node(block.target).segments.last().and_then(|s| s.args.as_ref());
        let written: Option<Vec<&str>> = match args {
            None => Some(Vec::new()),
            Some(GenericArgsNode::Angle { args, .. }) => args
                .iter()
                .map(|a| match a {
                    GenericArgNode::Type(t) => match &design.type_node(*t).kind {
                        TypeKind::Path(p) => match design.path_node(*p).segments.as_slice() {
                            [one] if one.args.is_none() => Some(one.name.text.as_str()),
                            _ => None,
                        },
                        _ => None,
                    },
                    GenericArgNode::Const(_) => None,
                })
                .collect(),
            Some(GenericArgsNode::Paren { .. }) => None,
        };
        written.is_some_and(|w| w == generics)
    }
}
