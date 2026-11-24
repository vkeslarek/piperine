//! `inst.name` (part III §8.2.8): a read of a `pub` item of a placed child, typed as the item
//! itself: a node is a node, a carrier gives its value, a param its type, a `let` the type of its
//! value. An item that is not `pub` is `E0343` (the name resolver reports it); a commit or a
//! definition of a child's carrier is `E0222` (`definition.rs`). A field of anything else keeps
//! the `field access` refusal.

use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The type of the field read `expr`: the type of the child's item it names, or `None` after
    /// a diagnostic (`E0343` was reported at resolution for an item that is not `pub`).
    pub(super) fn field_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Field { expr: receiver, name, res } = &node.kind else { return None };
        match res {
            Resolution::Decl(item) if self.is_child_item(*item) => {
                self.infer_receiver(*receiver)?;
                self.item_type(*item, file, node.span)
            }
            // A field of a value (§1.3.4): the type says whether it exists. A missing or private
            // item of a placed child already has its `E0343`.
            // `bank.c0.sn` where `c0` is placed through a callable param (`cell: C`, `C: Fn(..)`): the module is the
            // template's, known at elaboration, which resolves the item (`E0343` for a missing or private one, §8.4.8); a node
            // until a context wants a value of it (`template_items.rs`)
            _ if self.placed_through_callable(*receiver) => {
                self.infer_receiver(*receiver)?;
                self.template_reads.insert(expr);
                Some(Ty::AnyNode)
            }
            _ => self.field_of_value(expr, *receiver, name),
        }
    }

    /// Whether `receiver` (a name, possibly reached through a child: `c0`, `bank.c0`) is bound by a `let` to a call of a callable
    /// param of a module: an instance of a module only the folded template names (§8.4.8).
    pub(super) fn placed_through_callable(&mut self, receiver: ExprId) -> bool {
        let mut at = receiver;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => at = *inner,
                _ => break,
            }
        }
        let binding = match &self.design.expr_node(at).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => decl,
                _ => return false,
            },
            ExprKind::Field { res: Resolution::Decl(decl), .. } => *decl,
            _ => return false,
        };
        let Some(parent) = self.design.decl_data(binding).parent else { return false };
        let DeclKind::Let(data) = &self.design.decl_data(parent).kind else { return false };
        let mut value = data.value;
        while let ExprKind::Paren(inner) = &self.design.expr_node(value).kind {
            value = *inner;
        }
        let ExprKind::Call { callee, .. } = &self.design.expr_node(value).kind else { return false };
        let ExprKind::Path(path) = &self.design.expr_node(*callee).kind else { return false };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(param)) => matches!(self.design.decl_data(param).kind, DeclKind::Param(_)) && self.module_param_callable(param).is_some(),
            _ => false,
        }
    }

    /// The receiver of `inst.name`: an instance named there is the place a member is read from, not
    /// a value read (§8.2.14, so no `E0333`); any other receiver is inferred as usual.
    fn infer_receiver(&mut self, receiver: ExprId) -> Option<Ty> {
        if let ExprKind::Path(path) = &self.design.expr_node(receiver).kind {
            if let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) {
                self.type_module_let_read(decl);
                // a placement in generation is the array of its instances; inside its loops the name is the iteration's (§8.2.23)
                let local = self.locals.get(&decl).cloned().map(|whole| self.generated_local(receiver, decl, whole));
                if let Some(instance @ Ty::Module(_)) = local {
                    // a template held by a function parameter (`self` of an `impl` on a module type, §3.1.4) is a value read
                    if matches!(self.design.decl_data(decl).kind, DeclKind::FnParam(_)) {
                        self.exprs[receiver.index()] = Some(instance.clone());
                    }
                    return Some(instance);
                }
            }
        }
        self.infer(receiver)
    }

    /// The param `recv` names (through parentheses) and its type: a param of the module being
    /// checked, `f64` or `bool`.
    pub(super) fn param_receiver(&mut self, recv: ExprId) -> Option<(DeclId, Ty)> {
        match &self.design.expr_node(recv).kind {
            ExprKind::Paren(inner) => self.param_receiver(*inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                let DeclKind::Param(p) = self.design.decl_data(decl).kind.clone() else { return None };
                self.lower_type(p.ty).map(|ty| (decl, ty))
            }
            _ => None,
        }
    }

    /// `r.changed()` (an `Event`: the param changed by a host `set`) or `r.prev()` (the param's value
    /// before its latest `set`, its own value before any) on the param `recv` (§5.3.1). A param is
    /// known, so `prev()` has the param's type and is not flagged.
    pub(super) fn param_history(&mut self, expr: ExprId, changed: bool, recv: ExprId, args: &[crate::pom::Arg]) -> Option<Ty> {
        let (_, ty) = self.param_receiver(recv)?;
        self.apply(expr, &[], args)?;
        self.infer(recv);
        self.builtins.insert(expr, if changed { super::Builtin::Changed } else { super::Builtin::Prev });
        Some(if changed { Ty::Event } else { ty })
    }

    /// Whether `item` is a port, a signal, a param or a `let` name of a module: what `inst.name`
    /// may name.
    pub(super) fn is_child_item(&self, item: DeclId) -> bool {
        match &self.design.decl_data(item).kind {
            DeclKind::Port(_) | DeclKind::Signal(_) | DeclKind::Param(_) => true,
            DeclKind::Binding(_) => self.design.decl_data(item).parent.is_some_and(|l| matches!(self.design.decl_data(l).kind, DeclKind::Let(_))),
            _ => false,
        }
    }

    /// The type a read of the child's `item` has.
    fn item_type(&mut self, item: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match self.design.decl_data(item).kind.clone() {
            DeclKind::Port(p) => self.read_type(item, p.ty),
            DeclKind::Signal(s) => self.read_type(item, s.ty),
            DeclKind::Param(p) => self.lower_type(p.ty),
            _ => {
                let module = self.module_of_let(item)?;
                self.check_ahead(module);
                let found = self.locals.get(&item).cloned();
                if found.is_none() && !self.poisoned.contains(&item) {
                    self.unsupported(file, span, "a `let` of a child whose type is not known");
                }
                found
            }
        }
    }

    /// The module whose body holds the `let` that binds `binding`.
    fn module_of_let(&self, binding: DeclId) -> Option<DeclId> {
        let mut at = self.design.decl_data(binding).parent;
        while let Some(decl) = at {
            if matches!(self.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            at = self.design.decl_data(decl).parent;
        }
        None
    }
}
