//! The targets of commits and definitions that name an element of an array of carriers or a part of a value
//! (composite-values CV-25; part IV §12.1): `bits[3] <- x`, `mem[a] <- d`, `st.n <- 3`, `word[2] <- true`, `mem[a].f <- v`
//! and the definitions `bits[0] = a`. The commit is `Stmt::Commit { target: Some(CommitTarget { element, path }) }`:
//! `element` the flat element of the array carrier (any integer expression), `path` the fields and constant elements
//! below it. A definition names a constant element (`Drive.element`), so a generation or an unrolled loop gives each
//! element its own drive; one that does not fold is `E0188` naming it (QC4).

use piperine_ir::{CommitTarget, ExprId as LoweredExpr, Projection, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{ExprId, ExprKind};

/// A place a commit or a definition writes, lowered.
pub(super) struct PlaceTarget {
    /// The carrier of the body.
    pub(super) carrier: u32,
    /// The flat element of an array carrier, as an expression.
    pub(super) element: Option<LoweredExpr>,
    /// The element when it folds to a constant.
    pub(super) constant: Option<u64>,
    /// The steps into the element's value.
    pub(super) path: Vec<Projection>,
    /// The type of the levels the place names: what the written value has.
    pub(super) ty: ValueType,
}

impl PlaceTarget {
    /// The IR target of a commit: `None` for the whole carrier.
    pub(super) fn commit_target(&self) -> Option<CommitTarget> {
        (self.element.is_some() || !self.path.is_empty()).then(|| CommitTarget { element: self.element, path: self.path.clone() })
    }
}

/// One step of a place from its root carrier outward.
enum Step {
    /// `[index]`.
    Index(ExprId),
    /// `.name` of the value `inner` evaluates to.
    Field { inner: ExprId, name: String },
}

impl Elaboration<'_, '_> {
    /// The place `place` of a commit or a definition, lowered. `None` after a diagnostic.
    pub(super) fn place_target(&mut self, scope: &mut Scope, place: ExprId) -> Option<PlaceTarget> {
        let (root, steps) = self.place_steps(place);
        if self.root_decl(root).is_some_and(|d| self.frames.get(scope.frame).is_some_and(|f| f.empty_arrays.contains(&d))) {
            self.element_of_empty_array(scope.file, self.design.expr_node(place).span);
            return None;
        }
        let carrier = self.drive_target(scope, root)?;
        let carried = self.structure.carriers[carrier as usize].clone();
        let mut ty = carried.ty.clone();
        let mut steps = steps.into_iter().peekable();
        let (mut element, mut constant) = (None, None);
        if let Some(count) = carried.count {
            let Some(decl) = self.root_decl(root) else { return None };
            let shape = self.carrier_array_shape(decl, scope.frame)?.0;
            let span = self.design.expr_node(place).span;
            // a carrier declared inside generation, named inside its loops, is the iteration's element (§8.2.23)
            let generated = self.generated_carrier_levels(scope, root);
            let mut indexes = Vec::new();
            for _ in generated.len()..shape.dims.len() {
                match steps.next() {
                    Some(Step::Index(index)) => indexes.push(index),
                    _ => {
                        self.unsupported(scope.file, span, "a commit or a definition of a whole array of carriers, or of a part of one: name an element");
                        return None;
                    }
                }
            }
            debug_assert_eq!(shape.count(), count);
            let found = self.flat_element(scope, carrier, &shape, (&generated, &indexes), span)?;
            (element, constant) = (Some(found.index), found.constant);
        }
        let mut path = Vec::new();
        for step in steps {
            match step {
                Step::Index(index) => {
                    path.push(Projection::Index(self.lower_expr(scope, index)?));
                    ty = match ty {
                        ValueType::Array(element, _) => *element,
                        _ => return self.refuse_place(scope, place, "an element of a value that is no array"),
                    };
                }
                Step::Field { inner, name } => {
                    let Some(index) = self.typed(inner).and_then(|inner_ty| self.field_index(&inner_ty, &name)) else {
                        return self.refuse_place(scope, place, "a field of a type that has none");
                    };
                    path.push(Projection::Field(index));
                    ty = match ty {
                        ValueType::Tuple(parts) => parts.get(index as usize)?.clone(),
                        ValueType::Struct(id) => self.type_arena.field_types(id).get(index as usize)?.clone(),
                        _ => return self.refuse_place(scope, place, "a field of a value that is no tuple or struct"),
                    };
                }
            }
        }
        Some(PlaceTarget { carrier, element, constant, path, ty })
    }

    /// The root expression of a place and the steps from it, outermost last.
    fn place_steps(&self, place: ExprId) -> (ExprId, Vec<Step>) {
        let mut steps = Vec::new();
        let mut at = self.peel_parens(place);
        loop {
            match self.design.expr_node(at).kind.clone() {
                ExprKind::Index { expr, index } => {
                    steps.push(Step::Index(index));
                    at = self.peel_parens(expr);
                }
                ExprKind::Field { expr, name, .. } => {
                    steps.push(Step::Field { inner: expr, name: name.text });
                    at = self.peel_parens(expr);
                }
                _ => break,
            }
        }
        steps.reverse();
        (at, steps)
    }

    /// The port or signal declaration a root expression names.
    fn root_decl(&self, root: ExprId) -> Option<crate::pom::DeclId> {
        let ExprKind::Path(path) = &self.design.expr_node(root).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(crate::pom::Resolution::Decl(decl)) => Some(decl),
            _ => None,
        }
    }

    /// The refusal of a place step that does not apply: `P0001` naming it, `None`.
    fn refuse_place(&mut self, scope: &Scope, place: ExprId, form: &str) -> Option<PlaceTarget> {
        self.unsupported(scope.file, self.design.expr_node(place).span, form);
        None
    }
}
