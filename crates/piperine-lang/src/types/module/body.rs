//! The pass over one module: its items in three phases, its behaviors, its placements.

use crate::refusal::forms::ValueForm;
use crate::pom::{BehaviorKind, DeclId, DeclKind};

use super::ModuleTyping;
use crate::types::check::Context;
use crate::types::Ty;

/// The order a module body is checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPhase {
    /// Ports, params and signals.
    Declarations,
    /// Named expressions, which read the declarations.
    Lets,
    /// Definitions, which read both.
    Definitions,
}

impl ModuleTyping<'_, '_> {
    /// Whether `decl`, or a declaration above it, is a generation `for`/`if` (a module body item generated per iteration).
    pub(in crate::types) fn inside_generation(&self, decl: DeclId) -> bool {
        let mut at = Some(decl);
        while let Some(current) = at {
            if matches!(self.state.design.decl_data(current).kind, DeclKind::Generation(_)) {
                return true;
            }
            at = self.state.design.decl_data(current).parent;
        }
        false
    }

    /// One item of a module body (or of a generation in it) in `phase`.
    fn module_item(&mut self, child: DeclId, phase: ItemPhase) {
        let item = self.state.design.decl_data(child).clone();
        // a bundle is no value, carrier or node: its fields are the items (`pom/bundles.rs`)
        if self.state.design.is_bundle_owner(child) || self.state.design.is_bundle_group(child) {
            return;
        }
        match (&item.kind, phase) {
            (DeclKind::Port(p), ItemPhase::Declarations) => self.modules().port(child, &p.clone(), item.file),
            (DeclKind::Param(p), ItemPhase::Declarations) => self.modules().param(child, &p.clone()),
            (DeclKind::Signal(s), ItemPhase::Declarations) => self.modules().signal(child, &s.clone()),
            (DeclKind::Let(_), ItemPhase::Lets) => self.modules().type_module_let(child),
            (DeclKind::Definition(d), ItemPhase::Definitions) => self.carriers().definition(&d.clone()),
            (DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_), _) => {}
            (DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error, _) => {}
            // A closure's parameters (`|new, old| ..` of `limexp`, §22.2.4) are children of the
            // module that holds the closure, typed with it, not module items (design D9).
            (DeclKind::Binding(_), _) => {}
            (DeclKind::Generation(_), _) => self.modules().generation(child, phase == ItemPhase::Declarations, |this, inner| this.module_item(inner, phase)),
            (other, ItemPhase::Declarations) => self.refuse_with(item.file, item.span, ValueForm::InsideModule, [other.label().to_lowercase().to_string()]),
            (_, _) => {}
        }
    }

    /// A module of the reachable set: its params, nodes and placements, then its analog blocks.
    /// The modules it places are reached.
    pub(in crate::types) fn module(&mut self, decl: DeclId) {
        let generic_item = self.state.design.decl_data(decl).generics_span.map(|_| decl);
        let outer = self.state.generics.replace_generic_item(generic_item);
        self.module_of(decl);
        self.state.generics.replace_generic_item(outer);
    }

    /// [`ModuleTyping::module`] with the generic item set: a generic module is typed once at its definition, its type and
    /// nature parameters opaque (`Ty::Param`) and its const parameters `ConstTerm::Param` (GN-13, Q1 (c)).
    fn module_of(&mut self, decl: DeclId) {
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Module(m) = &data.kind else { return };
        if data.generics_span.is_some() {
            self.generics().generic_signature(decl);
        }
        if m.abi.is_some() {
            self.refuse(data.file, data.span, ValueForm::ExternModule);
        }
        if let Some(cond) = m.where_ {
            // Part III §8 errors: the `where` after the port list that is not a `bool` const expression.
            self.within(Context::Const, |this| this.expressions().check_as(cond, &Ty::Bool, "E0304"));
        }
        // Ports, params and signals first, then the `let`s in order, then definitions: every
        // name of the body is visible in the whole body (§8.2), and a carrier's type is known
        // before anything reads it.
        self.state.carriers.reset_book();
        // each bundle port or signal is followed by the fields it stands for, typed like ports and signals of their own (§7 rule 14)
        let items = self.state.design.module_items(&data.children);
        for phase in [ItemPhase::Declarations, ItemPhase::Lets, ItemPhase::Definitions] {
            for child in items.iter().copied() {
                self.module_item(child, phase);
            }
        }
        self.carriers().definition_loops();
        self.modules().module_result(decl, m);
        for behavior in m.behaviors.iter().copied() {
            let b = self.state.design.decl_data(behavior).clone();
            let DeclKind::Behavior(bd) = &b.kind else { continue };
            match bd.kind {
                BehaviorKind::Analog => {
                    self.state.values.set_context(Context::Analog);
                    self.expressions().statements(&bd.body, None);
                    self.state.values.set_context(Context::Module);
                }
                BehaviorKind::Digital => self.carriers().digital_block(behavior),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            // a placement inside generation is typed with generation itself (T72); until then its loop variable has no type
            let in_generation = self.inside_generation(self.state.design.expr_node(instance).owner);
            if self.state.result.exprs[instance.index()].is_none() && !in_generation {
                self.expressions().infer(instance);
            }
        }
        // the carriers declared inside generation are the module's too, one array each (§8.2.23)
        let carriers = self.modules().generated_items(&items);
        self.modules().missing_initial_values(&carriers);
        self.carriers().initial_values_under_element_definitions(&carriers);
        self.carriers().defer_unfolded_overlaps();
        self.modules().handed_drives();
    }
}
