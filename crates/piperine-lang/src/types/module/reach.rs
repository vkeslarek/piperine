//! `inst.name` (part III §8.2.8): a read of a `pub` item of a placed child, typed as the item
//! itself: a node is a node, a carrier gives its value, a param its type, a `let` the type of its
//! value. An item that is not `pub` is `E0343` (the name resolver reports it); a commit or a
//! definition of a child's carrier is `E0222` (`definition.rs`). A field of anything else keeps
//! the `field access` refusal.

use crate::refusal::forms::StructureForm;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::ModuleTyping;
use crate::types::carrier::element_target::{DriveTarget, Element};
use crate::types::Ty;

impl ModuleTyping<'_, '_> {
    /// The type of the field read `expr`: the type of the child's item it names, or `None` after
    /// a diagnostic (`E0343` was reported at resolution for an item that is not `pub`).
    pub(in crate::types) fn field_read(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.state.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        let ExprKind::Field { expr: receiver, name, res } = &node.kind else { return None };
        // a field of a bundle port or signal is the carrier or node its leaf stands for (§7 rule 12); the bundle is no value to read
        if let Some(place) = self.state.design.bundle_place(expr).filter(|place| self.state.design.leaf_owner(place.decl).is_some()) {
            if self.state.design.is_bundle_group(place.decl) {
                return self.bundle_as_value(expr);
            }
            // at an element of an array of bundles (`rx[k].valid`) the read is the element of the leaf's array (struct of arrays)
            self.element_indexes(&place);
            return self.item_type(place.decl, file, node.span).and_then(|read| read.element_at(place.elements.len()));
        }
        // a field of a bundle handle (`self.p`): read through the handle (`bundle_handles.rs`)
        if let Some((_, ty)) = self.bundle_handle_field(expr) {
            return if self.is_bundle_type(&ty) && !self.state.design.is_method_receiver(expr) && !self.state.design.is_field_base(expr) { self.bundle_as_value(expr) } else { Some(Self::handle_read_type(&ty)) };
        }
        match res {
            // a node of an instance whose `let` is being typed (`let p = D2(q.sn); let q = D2(p.sn);`, `let p = D2(p.sn);`): a
            // connection is hardware, not a value (§8.2.14), so the cycle it closes holds an unknown, the node (§8.3.5)
            Resolution::Decl(item) if self.is_node_item(*item) && self.names_an_open_instance(*receiver) => {
                self.open_instance(*receiver)?;
                self.item_type(*item, file, node.span)
            }
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
                // through an instance whose `let` is being typed (`let p = Bank::<_>(p.c0.sn) { .. };`) the read is a node, a
                // connection; a context that wants a value of it reads the open `let` itself (`template_items.rs`, `E0332`)
                if let Some(binding) = self.open_head(*receiver) {
                    self.open_instance(*receiver)?;
                    self.state.modules.open_template_reads.insert(expr, binding);
                } else {
                    self.infer_receiver(*receiver)?;
                }
                self.state.modules.template_reads.insert(expr);
                Some(Ty::AnyNode)
            }
            _ => self.expressions().field_of_value(expr, *receiver, name),
        }
    }

    /// Whether `receiver` (a name, possibly reached through a child: `c0`, `bank.c0`) is bound by a `let` to a call of a callable
    /// param of a module: an instance of a module only the folded template names (§8.4.8).
    pub(super) fn placed_through_callable(&mut self, receiver: ExprId) -> bool {
        let mut at = receiver;
        loop {
            match &self.state.design.expr_node(at).kind {
                ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => at = *inner,
                _ => break,
            }
        }
        let binding = match &self.state.design.expr_node(at).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => decl,
                _ => return false,
            },
            ExprKind::Field { res: Resolution::Decl(decl), .. } => *decl,
            _ => return false,
        };
        let Some(parent) = self.state.design.decl_data(binding).parent else { return false };
        let DeclKind::Let(data) = &self.state.design.decl_data(parent).kind else { return false };
        let mut value = data.value;
        while let ExprKind::Paren(inner) = &self.state.design.expr_node(value).kind {
            value = *inner;
        }
        let ExprKind::Call { callee, .. } = &self.state.design.expr_node(value).kind else { return false };
        let ExprKind::Path(path) = &self.state.design.expr_node(*callee).kind else { return false };
        match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(param)) => matches!(self.state.design.decl_data(param).kind, DeclKind::Param(_)) && self.generics().module_param_callable(param).is_some(),
            _ => false,
        }
    }

    /// Whether `item` is a node of a module: a port or a signal whose type is a nature.
    fn is_node_item(&self, item: DeclId) -> bool {
        let ty = match &self.state.design.decl_data(item).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return false,
        };
        self.state.design.nature_of(ty).is_some()
    }

    /// Whether `receiver` names an instance whose `let` is being typed, directly (`p`) or as the head of a chain of children
    /// (`b.c0`, `p.cells[1]`): its placement records the module before it types its arguments (`ModuleTyping::placement`).
    fn names_an_open_instance(&self, receiver: ExprId) -> bool {
        self.open_head(receiver).is_some()
    }

    /// The `let` being typed that `receiver` names as the head of a chain of children, through any depth of reach and the
    /// iterations of a generation (`p`, `b.c0`, `p.cells[1]`, `p.c0.c1`), when its placement records its module already.
    fn open_head(&self, receiver: ExprId) -> Option<DeclId> {
        match &self.state.design.expr_node(receiver).kind {
            ExprKind::Paren(inner) | ExprKind::Index { expr: inner, .. } => self.open_head(*inner),
            ExprKind::Path(path) => {
                self.open_placement(receiver)?;
                match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                    Some(Resolution::Decl(binding)) => Some(binding),
                    _ => None,
                }
            }
            ExprKind::Field { expr: inner, res: Resolution::Decl(item), .. } if self.is_child_item(*item) && !self.is_node_item(*item) => self.open_head(*inner),
            _ => None,
        }
    }

    /// The type of `receiver`, an instance [`ModuleTyping::names_an_open_instance`] accepts, recorded on each step of the chain: the
    /// module its open `let` places, then each child's own type. `None` after a diagnostic.
    fn open_instance(&mut self, receiver: ExprId) -> Option<Ty> {
        let node = self.state.design.expr_node(receiver).clone();
        let ty = match &node.kind {
            ExprKind::Paren(inner) => self.open_instance(*inner)?,
            // an iteration of a placement in generation: the element of the array of its instances (§8.2.23)
            ExprKind::Index { expr: inner, index } => {
                let whole = self.open_instance(*inner)?;
                self.expressions().infer(*index);
                match whole {
                    Ty::Array(element, _) => *element,
                    other => other,
                }
            }
            ExprKind::Path(_) => Ty::module(self.open_placement(receiver)?),
            ExprKind::Field { expr: inner, res: Resolution::Decl(item), .. } => {
                self.open_instance(*inner)?;
                let file = self.file_of_expr(receiver);
                self.item_type(*item, file, node.span)?
            }
            _ => return None,
        };
        self.state.result.exprs[receiver.index()] = Some(ty.clone());
        Some(ty)
    }

    /// The module placed by the `let` the path `receiver` names while that `let` is being typed. `None` for any other path.
    fn open_placement(&self, receiver: ExprId) -> Option<DeclId> {
        let ExprKind::Path(path) = &self.state.design.expr_node(receiver).kind else { return None };
        let Some(Resolution::Decl(binding)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
        let mut value = *self.state.modules.lets_open.get(&binding)?;
        while let ExprKind::Paren(inner) = &self.state.design.expr_node(value).kind {
            value = *inner;
        }
        match self.state.result.callees.get(&value) {
            Some(crate::types::Callee::Module(module)) => Some(*module),
            _ => None,
        }
    }

    /// The target `inst.name` of a definition or a commit when `item` is a resolved carrier of the child that this module may
    /// drive (§5.2.2: any module that sees a `pub` signal drives it, §22.3): the nature has an `impl Resolve`, the item is no
    /// `in` port and holds no array. `None` leaves the field to `E0222`: a carrier with one driver is written by its own module.
    pub(in crate::types) fn reached_drive_target(&mut self, place: ExprId, item: DeclId) -> Option<DriveTarget> {
        let (declared, direction) = match self.state.design.decl_data(item).kind.clone() {
            DeclKind::Port(p) => (p.ty, p.dir),
            DeclKind::Signal(s) => (s.ty, None),
            _ => return None,
        };
        let Some(Ty::Carrier { nature, .. }) = self.carriers().port_type(declared) else { return None };
        if direction == Some(crate::msl::ast::Dir::In) || !crate::types::analog::nature_rules::NatureRules::of(self.state.design, nature).resolved {
            return None;
        }
        let ExprKind::Field { expr: receiver, .. } = self.state.design.expr_node(place).kind.clone() else { return None };
        self.infer_receiver(receiver)?;
        let held = self.carriers().read_type(item, declared)?;
        self.state.result.exprs[place.index()] = Some(held.clone());
        Some(DriveTarget { carrier: item, held, element: Element::Whole, reached: true })
    }

    /// The receiver of `inst.name`: an instance named there is the place a member is read from, not
    /// a value read (§8.2.14, so no `E0333`); any other receiver is inferred as usual.
    pub(super) fn infer_receiver(&mut self, receiver: ExprId) -> Option<Ty> {
        if let ExprKind::Path(path) = &self.state.design.expr_node(receiver).kind {
            if let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                // a placement in generation is the array of its instances; inside its loops the name is the iteration's (§8.2.23)
                let local = self.binding_type(decl).map(|whole| self.generated_local(receiver, decl, whole));
                if let Some(instance @ Ty::Module(_)) = local {
                    // a template held by a function parameter (`self` of an `impl` on a module type, §3.1.4) is a value read
                    if matches!(self.state.design.decl_data(decl).kind, DeclKind::FnParam(_)) {
                        self.state.result.exprs[receiver.index()] = Some(instance.clone());
                    }
                    return Some(instance);
                }
            }
        }
        self.expressions().infer(receiver)
    }

    /// The param `recv` names (through parentheses) and its type: a param of the module being
    /// checked, `f64` or `bool`.
    pub(in crate::types) fn param_receiver(&mut self, recv: ExprId) -> Option<(DeclId, Ty)> {
        match &self.state.design.expr_node(recv).kind {
            ExprKind::Paren(inner) => self.param_receiver(*inner),
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                let DeclKind::Param(p) = self.state.design.decl_data(decl).kind.clone() else { return None };
                self.lower_type(p.ty).map(|ty| (decl, ty))
            }
            _ => None,
        }
    }

    /// `r.changed()` (an `Event`: the param changed by a host `set`) or `r.prev()` (the param's value
    /// before its latest `set`, its own value before any) on the param `recv` (§5.3.1). A param is
    /// known, so `prev()` has the param's type and is not flagged.
    pub(in crate::types) fn param_history(&mut self, expr: ExprId, changed: bool, recv: ExprId, args: &[crate::pom::Arg]) -> Option<Ty> {
        let (_, ty) = self.param_receiver(recv)?;
        self.expressions().apply(expr, &[], args)?;
        self.expressions().infer(recv);
        self.state.result.builtins.insert(expr, if changed { crate::types::Builtin::Changed } else { crate::types::Builtin::Prev });
        Some(if changed { Ty::Event } else { ty })
    }

    /// Whether `item` is a port, a signal, a param or a `let` name of a module: what `inst.name`
    /// may name.
    pub(super) fn is_child_item(&self, item: DeclId) -> bool {
        match &self.state.design.decl_data(item).kind {
            DeclKind::Port(_) | DeclKind::Signal(_) | DeclKind::Param(_) => true,
            DeclKind::Binding(_) => self.state.design.decl_data(item).parent.is_some_and(|l| matches!(self.state.design.decl_data(l).kind, DeclKind::Let(_))),
            _ => false,
        }
    }

    /// The type a read of the child's `item` has.
    fn item_type(&mut self, item: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match self.state.design.decl_data(item).kind.clone() {
            DeclKind::Port(p) => self.carriers().read_type(item, p.ty),
            DeclKind::Signal(s) => self.carriers().read_type(item, s.ty),
            DeclKind::Param(p) => self.lower_type(p.ty),
            _ => {
                let module = self.module_of_let(item)?;
                self.check_ahead(module);
                let found = self.state.result.locals.get(&item).cloned();
                if found.is_none() && !self.state.values.is_poisoned(item) {
                    self.refuse(file, span, StructureForm::LetOfChildWhoseTypeIsNot);
                }
                found
            }
        }
    }

    /// The module whose body holds the `let` that binds `binding`.
    fn module_of_let(&self, binding: DeclId) -> Option<DeclId> {
        let mut at = self.state.design.decl_data(binding).parent;
        while let Some(decl) = at {
            if matches!(self.state.design.decl_data(decl).kind, DeclKind::Module(_)) {
                return Some(decl);
            }
            at = self.state.design.decl_data(decl).parent;
        }
        None
    }
}
