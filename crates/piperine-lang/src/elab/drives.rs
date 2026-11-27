//! Drives (part II §5.2, part III §8.2.9): each definition `place = expr [after d];` of a module
//! is one `Drive` of its body, with its `after` when written (DIG-09 AC 2). An expression given
//! at an `in` carrier port of a placement is an anonymous definition: a signal carrier of the
//! parent named `<instance>.<port>`, its drive, and the child's port bound to it (DIG-09 AC 3).

use piperine_ir::{Carrier, CarrierRole, Drive, ExprId as LoweredExpr, ValueType};

use super::drive_overlaps::{DriveSite, DriveWay, ElementReach};
use super::element_commits::PlaceTarget;
use super::elaborator::Elaboration;
use super::generation::Iteration;
use super::lower::Scope;
use crate::refusal::forms::DigitalForm;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

impl Elaboration<'_, '_> {
    /// A scope for code of the body outside its `analog` block and its functions (a drive, a
    /// reaction): the names of `scope` (params, frame), with no local of its own.
    pub(super) fn sub_scope(scope: &Scope) -> Scope {
        let mut sub = Scope::new(scope.file);
        sub.params = scope.params.clone();
        sub.keys = scope.keys.clone();
        sub.frame = scope.frame;
        sub.unrolled = scope.unrolled.clone();
        sub.ordinals = scope.ordinals.clone();
        sub
    }

    /// The definitions of the module of the active frame, in source order, as drives of the body; read in `scope`. A definition written in
    /// generation is one drive per iteration that elaborates it, its loop variables the iteration's constants (§8.2.23).
    pub(super) fn lower_definitions(&mut self, scope: &Scope) {
        let items: Vec<(DeclId, Iteration)> = self.generated.unrolled().to_vec();
        for (child, iteration) in items {
            let data = self.design.decl_data(child).clone();
            let DeclKind::Definition(definition) = &data.kind else { continue };
            let mut iteration_scope = Self::sub_scope(scope);
            for (variable, value) in &iteration.values {
                iteration_scope.unrolled.bind(*variable, piperine_ir::Constant::Int(*value));
            }
            iteration_scope.ordinals = iteration.ordinals.clone();
            let scope = &iteration_scope;
            let mut place_scope = Self::sub_scope(scope);
            let Some(target) = self.place_target(&mut place_scope, definition.place) else { continue };
            // a definition names a constant element: each element is its own drive (`element_commits.rs`)
            if target.element.is_some() && target.constant.is_none() {
                self.not_folded(scope, self.design.expr_node(definition.place).span, "the element of a definition");
                continue;
            }
            if target.element.is_some() && definition.after.is_some() {
                self.refuse(scope.file, self.design.expr_node(definition.place).span, DigitalForm::DelayedDefinitionOfElementOfArrayOf);
                continue;
            }
            let origin = self.locator.decl(child);
            self.record_drive_site(&target, DriveWay::Definition, scope.file, self.design.expr_node(definition.place).span);
            if let Some((value, after)) = self.lower_drive_value(scope, definition.value, definition.after, Some(&target.ty)) {
                self.structure.drives.push(Drive { carrier: target.carrier, value, after, origin, element: target.constant, path: target.path });
            }
        }
    }

    /// Records where the definition or commit of `target` is written (`drive_overlaps.rs`): a part that indexes into the
    /// element's value is left to the checker.
    pub(super) fn record_drive_site(&mut self, target: &PlaceTarget, way: DriveWay, file: crate::pom::FileId, span: crate::msl::Span) {
        let Some(fields) = DriveSite::fields_of(&target.path) else { return };
        let reach = ElementReach::of_element(target.element.is_some(), target.constant);
        self.structure.drive_sites.push(DriveSite { carrier: target.carrier, reach, fields, way, file, span });
    }

    /// The carrier of the body a definition or a commit writes. `None` after a diagnostic (the
    /// checker refuses every other target first).
    pub(super) fn drive_target(&mut self, scope: &Scope, place: ExprId) -> Option<u32> {
        let node = self.design.expr_node(place).clone();
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.frames.get(scope.frame).and_then(|f| f.carriers.get(&decl))
        {
            return Some(*index);
        }
        // a field of a bundle port or signal is the carrier its leaf stands for (§7 rule 12)
        if let Some(leaf) = self.bundle_leaf(scope, place)
            && let Some(index) = self.frames.get(scope.frame).and_then(|f| f.carriers.get(&leaf.decl))
        {
            return Some(*index);
        }
        self.refuse(scope.file, node.span, DigitalForm::TargetThatIsNotCarrierOfThis);
        None
    }

    /// The value and the delay of a drive, each lowered in a scope of its own: a drive has no
    /// local (the IR's `Drive` holds expressions only), so a `match` reads its scrutinee at each
    /// test and a `let` inside one is `P0001`.
    pub(super) fn lower_drive_value(&mut self, scope: &Scope, value: ExprId, after: Option<ExprId>, wanted: Option<&ValueType>) -> Option<(LoweredExpr, Option<LoweredExpr>)> {
        let mut sub = Self::sub_scope(scope);
        sub.no_locals = true;
        let lowered = match wanted {
            Some(ty) => self.lower_stored(&mut sub, value, ty),
            None => self.lower_expr(&mut sub, value),
        };
        let delay = match after {
            Some(after) => Some(self.lower_expr(&mut sub, after)),
            None => None,
        };
        if !sub.locals.is_empty() {
            let span = self.design.expr_node(value).span;
            self.refuse(scope.file, span, DigitalForm::LetInsideDefinition);
            return None;
        }
        if lowered.is_none() && self.diagnostics.is_empty() {
            let span = self.design.expr_node(value).span;
            self.refuse_silent_failure(scope.file, span, DigitalForm::DriveValue);
        }
        let delay = match delay {
            Some(d) => Some(d?),
            None => None,
        };
        Some((lowered?, delay))
    }

    /// An expression at the `in` carrier port `port` of the placement named `instance`: the
    /// signal carrier `<instance>.<port>` of the parent and its undelayed drive. The carrier's
    /// index, or `None` after a diagnostic.
    pub(super) fn anonymous_definition(&mut self, scope: &Scope, instance: &str, port: DeclId, value: ExprId) -> Option<u32> {
        let ty = self.carried_type(port)?;
        let port_name = self.design.decl_data(port).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let origin = self.expr_origin(value);
        let (lowered, _) = self.lower_drive_value(scope, value, None, Some(&ty))?;
        let name = format!("{}{instance}.{port_name}", self.frames[scope.frame].prefix);
        let carrier = self.add_carrier(Carrier { name, ty, role: CarrierRole::Signal, init: None, origin: origin.clone(), count: None });
        self.structure.drives.push(Drive { carrier, value: lowered, after: None, origin, element: None, path: vec![] });
        Some(carrier)
    }
}
