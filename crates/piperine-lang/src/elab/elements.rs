//! Elements of arrays of carriers read in the elaborated body (composite-values CV-24; part II §5.1.8): `mem[i]` reads
//! one element (`Expr::CarrierElement`), `lines[k].prev()` its previous value (`Expr::ElementPrevious`), `lines[i].changed()`
//! read as a `bool` its change (`Expr::ElementChanged`), and `on lines[k].rise()` is an event of that element only
//! (`EventSource::ElementChanged`, `k` folded). A nested array `g[r][c]` is one element of one carrier, row-major:
//! `r * C + c`. A constant index at or past the count is `E0104` naming it (§1 rule 11); an index in an event head that
//! does not fold is `E0188` naming it (QC4: the user chose the const-expression code).

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
        // a carrier declared inside generation, named inside its loops, is the iteration's element (§8.2.23)
        let generated = self.generated_carrier_levels(scope, at);
        // a handle bound to an element is that element
        if indexes.is_empty() && generated.is_empty() {
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
        let Some(array) = self.consts.typed_carrier_array(decl) else { return Lookup::No };
        let span = self.design.expr_node(expr).span;
        if !indexes.is_empty() && self.frames.get(scope.frame).is_some_and(|f| f.empty_arrays.contains(&decl)) {
            self.element_of_empty_array(scope.file, span);
            return Lookup::Failed;
        }
        let Some(carrier) = self.frame_carrier(scope.frame, decl) else { return Lookup::No };
        let Some(shape) = self.shape_in(&array, scope.frame) else {
            self.unsupported(scope.file, span, "an element of an array whose length is not a constant yet");
            return Lookup::Failed;
        };
        if generated.len() + indexes.len() != shape.dims.len() {
            self.unsupported(scope.file, span, "a part of a nested array of carriers as a value: name an element");
            return Lookup::Failed;
        }
        match self.flat_element(scope, carrier, &shape, (&generated, &indexes), span) {
            Some(found) => Lookup::Found(found),
            None => Lookup::Failed,
        }
    }

    /// The levels of the carrier `root` names that the generation `for`s around the code being lowered fix: the ordinals of
    /// the iterations of the loops that declare it and that `scope` is inside, outermost first (§8.2.23). Empty for a
    /// carrier declared outside generation, or for code outside its loops.
    pub(super) fn generated_carrier_levels(&self, scope: &Scope, root: ExprId) -> Vec<u64> {
        let ExprKind::Path(path) = &self.design.expr_node(root).kind else { return Vec::new() };
        let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return Vec::new() };
        if self.generated.of(decl).is_none() || scope.frame != 0 || !self.frames[0].carriers.contains_key(&decl) {
            return Vec::new();
        }
        self.implicit_ordinals(decl, &scope.ordinals)
    }

    /// The flat element of the array carrier `carrier` named by `(fixed, indexes)`, one per level: the levels generation
    /// fixes, then the written indexes. The index expression and the element when every index folds. `None` after a
    /// diagnostic (`E0104` for a constant past the end).
    pub(super) fn flat_element(&mut self, scope: &mut Scope, carrier: u32, shape: &ArrayShape, (fixed, indexes): (&[u64], &[ExprId]), span: crate::msl::Span) -> Option<ElementRef> {
        let origin = self.locator.origin(scope.file, span);
        let mut lowered = Vec::with_capacity(fixed.len() + indexes.len());
        for ordinal in fixed {
            lowered.push(self.emit(Expr::Int(i128::from(*ordinal)), ValueType::UInt(64), origin.clone()));
        }
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
    /// does not fold (QC4, `E0188`, the code of a non-constant where a constant is required).
    pub(super) fn folded_element(&mut self, scope: &Scope, recv: ExprId, element: ElementRef) -> Option<u64> {
        if element.constant.is_none() {
            let span = self.design.expr_node(recv).span;
            self.not_folded(scope, span, "the element of an event head");
        }
        element.constant
    }
}

impl Elaboration<'_, '_> {
    /// `E0188` for a constant expected where `what` (the source text at `span`) does not fold at
    /// elaboration: it names the expression written.
    pub(super) fn not_folded(&mut self, scope: &Scope, span: crate::msl::Span, what: &str) {
        let text = self.design.file_data(scope.file).text.get(span.start..span.end).unwrap_or("").to_string();
        let message = format!("not a const expression: {what} must fold at elaboration, and `{text}` does not");
        self.report(scope.file, Diagnostic::new("E0188", message, span, "not a const expression"));
    }
}

impl Elaboration<'_, '_> {
    /// The nodes the argument `arg` names in an array of nodes: the run of consecutive nodes `(first, length)` of the whole
    /// array, of a sub-array `g[r]` of a nested one, or of one element (`length` 1, `whole` false). `Some(None)` when `arg`
    /// names no array of nodes; `None` after a diagnostic: an index that does not fold after unrolling (QC4, `E0188`
    /// naming it), or a constant past the end (`E0104`).
    fn node_run(&mut self, scope: &Scope, arg: ExprId) -> Option<Option<(u32, u64, bool)>> {
        let mut at = self.peel_parens(arg);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr: inner, index } = self.design.expr_node(at).kind.clone() {
            indexes.push(index);
            at = self.peel_parens(inner);
        }
        indexes.reverse();
        // a name of this module, or a `pub` array of nodes of a placed child (`c.tap[k]`)
        let (frame, decl) = match self.design.expr_node(at).kind.clone() {
            ExprKind::Path(path) => {
                let Some(Resolution::Decl(decl)) = self.design.path_node(path).segments.last().map(|s| s.res) else { return Some(None) };
                (scope.frame, decl)
            }
            ExprKind::Field { .. } => {
                let (frame, item) = self.reached_item(scope, at)?;
                if !self.reach_node_array(frame, item) {
                    return Some(None);
                }
                (frame, item)
            }
            _ => return Some(None),
        };
        let Some(shape) = self.frames.get(frame).and_then(|f| f.node_shapes.get(&decl)).cloned() else {
            // a node of an arm that was not elaborated (§8.2.24)
            if self.generated.of(decl).is_some_and(Vec::is_empty) {
                self.refer_outside_generation(scope.file, self.design.expr_node(arg).span, decl, None);
                return None;
            }
            return Some(None);
        };
        let span = self.design.expr_node(arg).span;
        let Some(piperine_ir::Terminal::Node(first)) = self.frame_node(frame, decl) else { return Some(None) };
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        // inside the generation `for`s that declare the array, the name is the iteration's element (§8.2.23)
        let implicit = self.implicit_ordinals(decl, &scope.ordinals);
        let generated = self.generated.of(decl).is_some();
        let mut flat = 0u64;
        let mut levels = implicit.len();
        for (ordinal, length) in implicit.iter().zip(&shape.dims) {
            flat = flat * length + ordinal;
        }
        if indexes.len() + implicit.len() > shape.dims.len() {
            self.unsupported(scope.file, span, "an index of an array of nodes past its levels");
            return None;
        }
        for (index, length) in indexes.iter().zip(&shape.dims[implicit.len()..]) {
            let folded = match self.consts.value(*index, &bindings) {
                Ok(piperine_ir::Constant::Int(at)) => u64::try_from(at).ok(),
                _ => None,
            };
            let Some(at) = folded else {
                self.not_folded(scope, self.design.expr_node(*index).span, "the index of an array of nodes");
                return None;
            };
            if at >= *length {
                if generated {
                    self.refer_outside_generation(scope.file, span, decl, Some((at, *length)));
                } else {
                    let message = format!("the element {at} is out of range for an array of {length} nodes (§1 rule 11)");
                    self.report(scope.file, Diagnostic::new("E0104", message, span, "index out of range"));
                }
                return None;
            }
            flat = flat * length + at;
            levels += 1;
        }
        let below: u64 = shape.dims[levels..].iter().product();
        Some(Some((first + (flat * below) as u32, below, levels == shape.dims.len())))
    }

    /// The node `bl[k]` of an array of nodes names (`g[r][c]` of a nested one): its terminal. `Some(None)` when `arg` is no
    /// element of an array of nodes; `None` after a diagnostic (`node_run`).
    pub(super) fn node_element(&mut self, scope: &Scope, arg: ExprId) -> Option<Option<piperine_ir::Terminal>> {
        match self.node_run(scope, arg)? {
            Some((first, _, true)) => Some(Some(piperine_ir::Terminal::Node(first))),
            Some(_) => {
                self.unsupported(scope.file, self.design.expr_node(arg).span, "an array of nodes, or a part of a nested one, as a node argument: name an element");
                None
            }
            None => Some(None),
        }
    }

    /// The terminals an argument at an array port of nodes gives the child: the nodes of the whole array or of a
    /// sub-array, one terminal each. `None` after a diagnostic.
    pub(super) fn node_array_terminals(&mut self, scope: &Scope, arg: ExprId) -> Option<Vec<piperine_ir::Terminal>> {
        match self.node_run(scope, arg)? {
            Some((first, length, false)) => Some((0..length as u32).map(|k| piperine_ir::Terminal::Node(first + k)).collect()),
            _ => {
                self.unsupported(scope.file, self.design.expr_node(arg).span, "an argument at an array port of nodes that is no array of nodes");
                None
            }
        }
    }
}

impl Elaboration<'_, '_> {
    /// The array carrier an argument at a carrier port names through `x[k]` or a sub-array `g[r]`, and the window of it
    /// the child's port is given: one element (`length` `None`) or the run of a sub-array. The outer `None` when `arg`
    /// is no part of an array of carriers; `Some(None)` after a diagnostic (an index that does not fold is `E0188`
    /// naming it, a constant past the end `E0104`).
    pub(super) fn carrier_window(&mut self, scope: &Scope, arg: ExprId) -> Option<Option<(u32, Option<piperine_ir::CarrierWindow>)>> {
        let mut at = self.peel_parens(arg);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr: inner, index } = self.design.expr_node(at).kind.clone() {
            indexes.push(index);
            at = self.peel_parens(inner);
        }
        indexes.reverse();
        let ExprKind::Path(path) = self.design.expr_node(at).kind.clone() else { return None };
        let Some(Resolution::Decl(decl)) = self.design.path_node(path).segments.last().map(|s| s.res) else { return None };
        let array = self.consts.typed_carrier_array(decl)?;
        let span = self.design.expr_node(arg).span;
        if !indexes.is_empty() && self.frames.get(scope.frame).is_some_and(|f| f.empty_arrays.contains(&decl)) {
            self.element_of_empty_array(scope.file, span);
            return Some(None);
        }
        let carrier = self.frame_carrier(scope.frame, decl)?;
        let shape = self.shape_in(&array, scope.frame)?;
        // a carrier declared inside generation, named inside its loops, is the iteration's element (§8.2.23)
        let generated = self.generated_carrier_levels(scope, at);
        let levels = generated.len() + indexes.len();
        if levels > shape.dims.len() {
            self.unsupported(scope.file, span, "an index of an array of carriers past its levels");
            return Some(None);
        }
        if levels == 0 {
            return Some(Some((carrier, None)));
        }
        let prefix = ArrayShape { dims: shape.dims[..levels].to_vec() };
        let mut inner_scope = Self::sub_scope(scope);
        let Some(found) = self.flat_element(&mut inner_scope, carrier, &prefix, (&generated, &indexes), span) else { return Some(None) };
        let Some(first) = found.constant else {
            self.not_folded(scope, span, "the element of a connection");
            return Some(None);
        };
        let below: u64 = shape.dims[levels..].iter().product();
        let window = match levels == shape.dims.len() {
            true => piperine_ir::CarrierWindow { first, length: None },
            false => piperine_ir::CarrierWindow { first: first * below, length: Some(below) },
        };
        Some(Some((carrier, Some(window))))
    }
}
