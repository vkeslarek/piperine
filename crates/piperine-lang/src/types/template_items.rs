//! Items of a child read through a template (§8.4.8): `c0.cs`, where `c0` is placed through a callable param (`cell: C`,
//! `C: Fn(..)`), names an item of a module only elaboration knows. A node read needs no type of its own (`v(c0.sn)`); any other
//! item takes the type the context wants of it, the way an open literal does (§1.4.4): a typed `let`, a contribution, a
//! condition, the other operand of an operator, or the use that fixes a `let` bound to the read. That type is recorded
//! (`Types::template_item`), and the elaborator checks the concrete item has it, an instantiation error (§3 rule 9) when it
//! does not. A read nothing gives a type is `E0181`.

use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, ExprId, ExprKind, FileId, Resolution};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The read through a template `expr` stands for, typed yet or not: the read itself (`c0.cs`, an item the resolver could not
    /// name, of a child placed through a callable param), or the read a `let` with no type was bound to (through parentheses).
    /// `None` for any other expression.
    pub(super) fn template_read_of(&mut self, expr: ExprId) -> Option<ExprId> {
        match self.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.template_read_of(inner),
            ExprKind::Path(path) => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(local)) => self.template_locals.get(&local).copied(),
                _ => None,
            },
            ExprKind::Field { expr: receiver, res, .. } if !matches!(res, Resolution::Decl(_)) => {
                let read = self.template_reads.contains(&expr) || self.placed_through_callable(receiver);
                read.then_some(expr)
            }
            _ => None,
        }
    }

    /// Gives the read through a template `expr` stands for the type `expected` the context wants of it, when that is a value
    /// type: recorded for the elaborator, and the `let` bound to it takes it too. `false` when `expr` is no such read or the
    /// context wants a node (the read then stays a node).
    pub(super) fn fix_template_read(&mut self, expr: ExprId, expected: &Ty) -> bool {
        let Some(read) = self.template_read_of(expr) else { return false };
        if !expected.is_value() {
            return false;
        }
        self.template_items.insert(read, expected.clone());
        self.exprs[read.index()] = Some(expected.clone());
        if let Some(local) = self.template_local_of(expr) {
            self.template_locals.remove(&local);
            self.locals.insert(local, expected.clone());
        }
        true
    }

    /// The `let` with no type that `expr` names, bound to a read through a template.
    fn template_local_of(&self, expr: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.template_local_of(*inner),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(local)) if self.template_locals.contains_key(&local) => Some(local),
                _ => None,
            },
            _ => None,
        }
    }

    /// A `let` with no type bound to `init`: when `init` is a read through a template, the first use that wants a value of it
    /// gives it its type.
    pub(super) fn wait_for_template_use(&mut self, binding: DeclId, init: ExprId) {
        if let Some(read) = self.template_read_of(init) {
            self.template_locals.insert(binding, read);
        }
    }

    /// `E0181` at the read through a template of every `let` with no type that no expression names, so no use gives it a type
    /// (§8.4.8).
    pub(super) fn report_unread_template_locals(&mut self) {
        let named: std::collections::BTreeSet<DeclId> = (0..self.design.expr_count())
            .filter_map(|index| match &self.design.expr_node(ExprId(index as u32)).kind {
                ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                    Some(Resolution::Decl(decl)) if self.template_locals.contains_key(&decl) => Some(decl),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let unread: Vec<ExprId> = self.template_locals.iter().filter(|(local, _)| !named.contains(local)).map(|(_, read)| *read).collect();
        for read in unread {
            let (file, span) = (self.file_of_expr(read), self.design.expr_node(read).span);
            self.report_untyped_template_read(file, span);
        }
    }

    /// Whether the receiver `recv` of a method call is a read through a template, which nothing gives a type: `E0181` at the read,
    /// the arguments typed for their own diagnostics.
    pub(super) fn untyped_template_receiver(&mut self, recv: ExprId, args: &[crate::pom::Arg]) -> bool {
        if !self.untyped_template_operand(recv) {
            return false;
        }
        for arg in args {
            self.infer(arg.value);
        }
        true
    }

    /// Whether `operand` of a form that gives it no type (a method's receiver, the operand of `as`) is a read through a template:
    /// `E0181` at the read when it is.
    pub(super) fn untyped_template_operand(&mut self, operand: ExprId) -> bool {
        let Some(read) = self.template_read_of(operand) else { return false };
        self.infer(operand);
        let (file, span) = (self.file_of_expr(read), self.design.expr_node(read).span);
        self.report_untyped_template_read(file, span);
        true
    }

    /// `E0181` at the read through a template at `span` of `file`, which no context gave a type.
    fn report_untyped_template_read(&mut self, file: FileId, span: Span) {
        let message = "the type of an item of a child placed through a template cannot be inferred here: only elaboration names its \
                       module (§8.4.8); give it a type with a typed `let`"
            .to_string();
        self.report(file, Diagnostic::new("E0181", message, span, "type not inferred"));
    }

    /// Whether a mismatch that found `found` at `span` of `file` holds a read through a template that no context gave a type:
    /// `E0181` is reported for it instead of a mismatch.
    pub(super) fn untyped_template_read(&mut self, file: FileId, span: Span, found: &str) -> bool {
        if found != "node" {
            return false;
        }
        let Some(read) = self.template_reads.iter().copied().find(|read| {
            let at = self.design.expr_node(*read).span;
            self.file_of_expr(*read) == file && span.start <= at.start && at.end <= span.end
        }) else {
            return false;
        };
        let at = self.design.expr_node(read).span;
        self.report_untyped_template_read(file, at);
        true
    }
}
