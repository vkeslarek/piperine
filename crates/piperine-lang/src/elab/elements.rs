//! Elements of arrays of carriers read in the elaborated body (composite-values CV-24; part II §5.1.8): `mem[i]` reads
//! one element (`Expr::CarrierElement`), `lines[k].prev()` its previous value (`Expr::ElementPrevious`), `lines[i].changed()`
//! read as a `bool` its change (`Expr::ElementChanged`), and `on lines[k].rise()` is an event of that element only
//! (`EventSource::ElementChanged`, `k` folded). A nested array `g[r][c]` is one element of one carrier, row-major:
//! `r * C + c`. A constant index at or past the count is `E0104` naming it (§1 rule 11); an index in an event head that
//! does not fold is the mismatch `E0180` naming it (QC4, amended: PI-06 forbids spelling `P0002`).

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::carrier_arrays::ArrayShape;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::{ExprId, ExprKind, Resolution};

/// One element of an array carrier a source form names, lowered.
#[derive(Clone, Copy, Debug)]
pub(super) struct ElementRef {
    /// The array carrier of the body.
    pub(super) carrier: u32,
    /// The element's index as an IR integer expression.
    pub(super) index: LoweredExpr,
    /// The element when the index folds to a constant.
    pub(super) constant: Option<u64>,
}

/// What looking for an element in a source form found.
pub(super) enum Lookup {
    /// The form is no element of an array of carriers.
    No,
    /// A diagnostic was reported.
    Failed,
    /// The element.
    Found(ElementRef),
}

impl Elaboration<'_, '_> {
    /// The element `expr` names when it is `x[i]` (or `g[r][c]`) of an array of carriers of this module, or the handle
    /// bound to one.
    pub(super) fn element_lookup(&mut self, scope: &mut Scope, expr: ExprId) -> Lookup {
        let mut at = self.peel_parens(expr);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr: inner, index } = self.design.expr_node(at).kind.clone() {
            if matches!(self.design.expr_node(index).kind, ExprKind::Range { .. }) {
                return Lookup::No;
            }
            indexes.push(index);
            at = self.peel_parens(inner);
        }
        // a handle bound to an element is that element
        if indexes.is_empty() {
            let bound = match &self.design.expr_node(at).kind {
                ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                    Some(Resolution::Decl(decl)) => scope.handles.get(&decl).copied(),
                    _ => None,
                },
                _ => None,
            };
            return match bound {
                Some(bound) if bound != at => self.element_lookup(scope, bound),
                _ => Lookup::No,
            };
        }
        indexes.reverse();
        let ExprKind::Path(path) = self.design.expr_node(at).kind.clone() else { return Lookup::No };
        let Some(Resolution::Decl(decl)) = self.design.path_node(path).segments.last().map(|s| s.res) else { return Lookup::No };
        let Some(array) = self.types.carrier_array(decl).cloned() else { return Lookup::No };
        let Some(carrier) = self.frame_carrier(scope.frame, decl) else { return Lookup::No };
        let span = self.design.expr_node(expr).span;
        let Some(shape) = ArrayShape::of(&array) else {
            self.unsupported(scope.file, span, "an element of an array whose length is not a constant yet");
            return Lookup::Failed;
        };
        if indexes.len() != shape.dims.len() {
            self.unsupported(scope.file, span, "a part of a nested array of carriers as a value: name an element");
            return Lookup::Failed;
        }
        match self.flat_element(scope, carrier, &shape, &indexes, span) {
            Some(found) => Lookup::Found(found),
            None => Lookup::Failed,
        }
    }

    /// The flat element of the array carrier `carrier` named by `indexes`, one per level: the index expression and the
    /// element when every index folds. `None` after a diagnostic (`E0104` for a constant past the end).
    fn flat_element(&mut self, scope: &mut Scope, carrier: u32, shape: &ArrayShape, indexes: &[ExprId], span: crate::msl::Span) -> Option<ElementRef> {
        let origin = self.locator.origin(scope.file, span);
        let mut lowered = Vec::with_capacity(indexes.len());
        for index in indexes {
            lowered.push(self.lower_expr(scope, *index)?);
        }
        let constants: Vec<Option<u64>> = lowered.iter().map(|e| self.constant_index(*e)).collect();
        for (constant, length) in constants.iter().zip(&shape.dims) {
            if let Some(at) = constant.filter(|at| at >= length) {
                let message = format!("the element {at} is out of range for an array of {length} carriers (§1 rule 11)");
                self.report(scope.file, Diagnostic::new("E0104", message, span, "index out of range"));
                return None;
            }
        }
        if let [only] = lowered[..] {
            return Some(ElementRef { carrier, index: only, constant: constants[0] });
        }
        if constants.iter().all(Option::is_some) {
            let flat = constants.iter().zip(&shape.dims).fold(0, |flat, (at, length)| flat * length + at.unwrap_or(0));
            let index = self.emit(Expr::Int(i128::from(flat)), ValueType::UInt(64), origin);
            return Some(ElementRef { carrier, index, constant: Some(flat) });
        }
        // `r * C + c` over the levels, in 64 bits
        let wide = ValueType::UInt(64);
        let mut flat: Option<LoweredExpr> = None;
        for (index, length) in lowered.iter().zip(&shape.dims) {
            let level = self.emit(Expr::Cast { expr: *index, to: wide.clone() }, wide.clone(), origin.clone());
            flat = Some(match flat {
                None => level,
                Some(before) => {
                    let stride = self.emit(Expr::Int(i128::from(*length)), wide.clone(), origin.clone());
                    let scaled = self.emit(Expr::Binary { op: BinaryOp::Mul, lhs: before, rhs: stride }, wide.clone(), origin.clone());
                    self.emit(Expr::Binary { op: BinaryOp::Add, lhs: scaled, rhs: level }, wide.clone(), origin.clone())
                }
            });
        }
        Some(ElementRef { carrier, index: flat?, constant: None })
    }

    /// The value of a lowered index when it is a constant.
    fn constant_index(&self, index: LoweredExpr) -> Option<u64> {
        match self.ir.expr_kind(index) {
            Some(Expr::Int(n)) => u64::try_from(*n).ok(),
            _ => None,
        }
    }

    /// `x[i]`: one element's value, typed as the carrier's value type.
    pub(super) fn carrier_element_read(&mut self, element: ElementRef, origin: Origin) -> LoweredExpr {
        let ty = self.structure.carriers[element.carrier as usize].ty.clone();
        self.emit(Expr::CarrierElement { carrier: element.carrier, index: element.index }, ty, origin)
    }

    /// `x[i].prev()`: the value the element had before its last change.
    pub(super) fn element_previous(&mut self, element: ElementRef, origin: Origin) -> LoweredExpr {
        let ty = self.structure.carriers[element.carrier as usize].ty.clone();
        self.emit(Expr::ElementPrevious { carrier: element.carrier, index: element.index }, ty, origin)
    }

    /// `x[i].changed()` read as a `bool`: whether the element changed in this microstep.
    pub(super) fn element_changed(&mut self, element: ElementRef, origin: Origin) -> LoweredExpr {
        self.emit(Expr::ElementChanged { carrier: element.carrier, index: element.index }, ValueType::Bool, origin)
    }

    /// The element of an event head's receiver, folded: the mismatch of a constant expected naming the index when it
    /// does not fold (QC4; `P0002` is no code any source spells, PI-06, so it is `E0180` as a non-constant array length is).
    pub(super) fn folded_element(&mut self, scope: &Scope, recv: ExprId, element: ElementRef) -> Option<u64> {
        if element.constant.is_none() {
            let span = self.design.expr_node(recv).span;
            self.not_folded(scope, span, "the element of an event head");
        }
        element.constant
    }
}

impl Elaboration<'_, '_> {
    /// The mismatch `E0180` of a constant expected where `what` (the source text at `span`) does not fold at
    /// elaboration: it names the expression written.
    pub(super) fn not_folded(&mut self, scope: &Scope, span: crate::msl::Span, what: &str) {
        let text = self.design.file_data(scope.file).text.get(span.start..span.end).unwrap_or("").to_string();
        let message = format!("mismatched types: expected a constant that folds at elaboration for {what}, found `{text}`");
        self.report(scope.file, Diagnostic::new("E0180", message, span, "does not fold"));
    }
}
