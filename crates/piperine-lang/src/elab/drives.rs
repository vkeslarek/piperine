//! Drives (part II §5.2, part III §8.2.9): each definition `place = expr [after d];` of a module
//! is one `Drive` of its body, with its `after` when written (DIG-09 AC 2). An expression given
//! at an `in` carrier port of a placement is an anonymous definition: a signal carrier of the
//! parent named `<instance>.<port>`, its drive, and the child's port bound to it (DIG-09 AC 3).

use piperine_ir::{Carrier, CarrierRole, Drive, ExprId as LoweredExpr, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

impl Elaboration<'_, '_> {
    /// A scope for code of the body outside its `analog` block and its functions (a drive, a
    /// reaction): the names of `scope` (params, frame), with no local of its own.
    pub(super) fn sub_scope(scope: &Scope) -> Scope {
        let mut sub = Scope::new(scope.file);
        sub.params = scope.params.clone();
        sub.frame = scope.frame;
        sub
    }

    /// The definitions of `module`, in source order, as drives of the body; read in `scope`.
    pub(super) fn lower_definitions(&mut self, scope: &Scope, module: DeclId) {
        let children = self.design.decl_data(module).children.clone();
        for child in children {
            let data = self.design.decl_data(child).clone();
            let DeclKind::Definition(definition) = &data.kind else { continue };
            let Some(carrier) = self.drive_target(scope, definition.place) else { continue };
            let origin = self.locator.decl(child);
            let wanted = self.structure.carriers.get(carrier as usize).map(|c| c.ty.clone());
            if let Some((value, after)) = self.lower_drive_value(scope, definition.value, definition.after, wanted.as_ref()) {
                self.structure.drives.push(Drive { carrier, value, after, origin });
            }
        }
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
        self.unsupported(scope.file, node.span, "a target that is not a carrier of this module");
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
            self.unsupported(scope.file, span, "a `let` inside a definition");
            return None;
        }
        if lowered.is_none() && self.diagnostics.is_empty() {
            let span = self.design.expr_node(value).span;
            self.refuse_silent_failure(scope.file, span, "a drive value");
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
        self.structure.drives.push(Drive { carrier, value: lowered, after: None, origin });
        Some(carrier)
    }
}
