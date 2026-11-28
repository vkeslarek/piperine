//! Arrays of carriers and of nodes declared (composite-values CV-23, CV-26; part II §5.1.8): a `signal` or a port
//! `[N; K]` of a discrete nature is **one** carrier of the body with `count` elements of its value type (a nested
//! array is one carrier of the product of the lengths, row-major: `x[r][c]` is element `r * C + c`), so a design holds
//! no storage proportional to `K` beyond what is written. Its initial value is `[x; K]` (every element starts at `x`) or
//! `[a, b, c]` (each element its own), kept on the carrier as one element's value or an array constant. An array of
//! nodes `[Electrical; K]` is `K` nodes of the body named `bl[0]` to `bl[K-1]` (an MNA unknown each is inherent), kept
//! under one name by a `NodeArray` (the host reads `Top.bl[2]` and `Top.bl`).

use piperine_ir::{AggregateData, Constant, NodeArray, ValueType};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::refusal::forms::DigitalForm;
use crate::pom::{DeclId, ExprId, ExprKind};
use crate::types::{Length, Ty};

/// Why the length of an array type has no value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LengthFault {
    /// The length does not fold to a constant.
    NotConstant,
    /// The length folds to this negative number.
    Negative(i128),
}

/// The shape of a declared array: its lengths from the outermost level in, and the number of elements they make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ArrayShape {
    /// One length per nesting level, outermost first.
    pub(super) dims: Vec<u64>,
}

impl ArrayShape {
    /// The number of elements: the product of the lengths.
    pub(super) fn count(&self) -> u64 {
        self.dims.iter().product()
    }

    /// The text of element `flat`: `[1][0]` for element 2 of a `[2][2]` array.
    pub(super) fn subscripts(&self, flat: u64) -> String {
        let mut rest = flat;
        let mut parts = Vec::new();
        for (level, _) in self.dims.iter().enumerate() {
            let below: u64 = self.dims[level + 1..].iter().product();
            parts.push(format!("[{}]", rest / below));
            rest %= below;
        }
        parts.concat()
    }
}

impl Elaboration<'_, '_> {
    /// Whether `decl` is an array of carriers of length 0 in `frame` (§1.2.3: the length is known and not negative, so
    /// zero is legal): it has no element, so the body gets no carrier for it.
    pub(super) fn is_empty_array(&mut self, frame: usize, decl: DeclId) -> bool {
        let Some(array) = self.state.consts.typed_carrier_array(decl) else { return false };
        self.shape_in(&array, frame).is_some_and(|shape| shape.count() == 0)
    }

    /// Keeps the name of the array `decl` of no element in the body's record, under the prefix of its `frame` (a hoisted
    /// placement's array is `p.none`, like its carriers): a host that names it reads the empty array (CV-28).
    pub(super) fn record_empty_array(&mut self, frame: usize, decl: DeclId) {
        let declared = self.state.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str());
        let name = format!("{}{declared}", self.state.structure.frames[frame].prefix);
        self.state.digital.structure.empty_arrays.push(piperine_ir::EmptyArray { name, origin: self.state.locator.decl(decl) });
    }

    /// `E0104` for an element of the empty array `decl`: no index is in range of a length of 0 (§1 rule 11).
    pub(super) fn element_of_empty_array(&mut self, file: crate::pom::FileId, span: crate::msl::Span) {
        let message = "the element is out of range for an array of 0 carriers (§1 rule 11)".to_string();
        self.report(file, crate::msl::Diagnostic::new("E0104", message, span, "index out of range"));
    }

    /// The shape of the array type `ty` of a declaration of `frame`: a length that reads a param is folded over the
    /// frame's params (§8.1.2, QC2). `None` when a length does not fold.
    pub(super) fn shape_in(&mut self, ty: &Ty, frame: usize) -> Option<ArrayShape> {
        let bindings = self.frame_bindings(frame);
        self.shape_over(ty, &bindings).ok()
    }

    /// The shape of the array type `ty` of the declaration `decl` in `frame`, or the diagnostic of why it has none:
    /// `E0104` for a length that folds to a negative number (§1.2.3 makes a length known and not negative, so a
    /// panic of the const expression the length is, §1 rule 11), `P0001` naming `what` for one that does not fold.
    pub(super) fn declared_shape(&mut self, ty: &Ty, frame: usize, decl: DeclId, what: &str) -> Option<ArrayShape> {
        let bindings = self.frame_bindings(frame);
        let fault = match self.shape_over(ty, &bindings) {
            Ok(shape) => return Some(shape),
            Err(fault) => fault,
        };
        let data = self.state.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        match fault {
            LengthFault::Negative(length) => {
                let message = format!("the length of an array of {what} is {length}: a length is known and not negative (§1.2.3)");
                self.report(file, crate::msl::Diagnostic::new("E0104", message, span, "negative length"));
            }
            LengthFault::NotConstant => self.refuse_with(file, span, DigitalForm::ArrayOfWhoseLengthIsNotConstant, [what.to_string()]),
        }
        None
    }

    /// The shape of the array type `ty` with the lengths that read params folded over `bindings`. The fault when a length
    /// does not fold, or folds to a negative number (reported where the length is declared, `declared_shape`).
    pub(super) fn shape_over(&mut self, ty: &Ty, bindings: &Bindings) -> Result<ArrayShape, LengthFault> {
        let mut dims = Vec::new();
        let mut at = ty;
        while let Ty::Array(element, length) = at {
            dims.push(match length {
                Length::Fixed(length) => *length,
                // the length of an array a generation `for` makes: the trips of its range (§8.2.23)
                Length::Generic(crate::types::ConstTerm::Expr(expr)) if let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.state.design.expr_node(*expr).kind => {
                    let bound = |this: &mut Self, bound| match this.state.consts.value(bound, bindings) {
                        Ok(piperine_ir::Constant::Int(value)) => Ok(value),
                        _ => Err(LengthFault::NotConstant),
                    };
                    let (first, last) = (bound(self, lo)?, bound(self, hi)?);
                    let count = last + i128::from(inclusive) - first;
                    u64::try_from(count.max(0)).map_err(|_| LengthFault::Negative(count))?
                }
                Length::Generic(crate::types::ConstTerm::Expr(expr)) => match self.state.consts.value(*expr, bindings) {
                    Ok(piperine_ir::Constant::Int(count)) => u64::try_from(count).map_err(|_| LengthFault::Negative(count))?,
                    _ => return Err(LengthFault::NotConstant),
                },
                Length::Generic(_) => return Err(LengthFault::NotConstant),
            });
            at = element;
        }
        Ok(ArrayShape { dims })
    }

    /// `E0313` when the array `argument` handed to the array port `port` of a placement is not of the port's shape,
    /// the port's lengths folded over the placed module's params `keys` (CV-23 AC 3, §8.1.10). A shape that does not
    /// fold, or an argument that is no declared array, is left to the bind. Whether it reported.
    pub(super) fn check_array_argument(&mut self, scope: &Scope, port: DeclId, argument: ExprId, keys: &Bindings) -> bool {
        let Some(wanted) = self.state.consts.typed_carrier_array(port).or_else(|| self.state.consts.typed_node_array(port)) else { return true };
        let Some(wanted_shape) = self.shape_over(&wanted, keys).ok() else { return true };
        let Some(found) = self.typed(argument).filter(|ty| matches!(ty, Ty::Array(..))) else { return true };
        let Some(found_shape) = self.shape_over(&found, &self.frame_bindings(scope.values.frame)).ok() else { return true };
        if found_shape == wanted_shape {
            return true;
        }
        let name = |shape: &ArrayShape| shape.dims.iter().map(|length| format!("[{length}]")).collect::<String>();
        let message = format!("an array of {} is connected to a port of {}: the lengths or the shape differ (§8.1.10)", name(&found_shape), name(&wanted_shape));
        let node = self.state.design.expr_node(argument);
        let file = self.state.design.decl_data(node.owner).file;
        self.report(file, crate::msl::Diagnostic::new("E0313", message, node.span, "another array shape"));
        false
    }

    /// The shape and the innermost value type of the array of carriers `decl` declares; `None` after a diagnostic.
    pub(super) fn carrier_array_shape(&mut self, decl: DeclId, frame: usize) -> Option<(ArrayShape, ValueType)> {
        let data = self.state.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        let Some(array) = self.state.consts.typed_carrier_array(decl) else { return None };
        let shape = self.declared_shape(&array, frame, decl, "carriers")?;
        let Ty::Carrier { value, .. } = array.innermost() else { return None };
        match self.stored_type(value) {
            Some(element) => Some((shape, element)),
            None => {
                self.refuse(file, span, DigitalForm::CarrierWhoseValueTypeHasNoValue);
                None
            }
        }
    }

    /// The initial value of the array carrier `decl` (`init` written), folded: `[x; K]` is one element's value, every
    /// element starting at `x`; `[a, b, c]` an array constant of every element's. `None` after a diagnostic.
    pub(super) fn array_init(&mut self, decl: DeclId, init: ExprId, shape: &ArrayShape, element: &ValueType, frame: usize) -> Option<Constant> {
        let file = self.state.design.decl_data(decl).file;
        let node = self.state.design.expr_node(init).clone();
        // `[x; K]` of the innermost level: every element starts at `x`
        let mut repeated = init;
        while let ExprKind::ArrayRepeat { elem, .. } = self.state.design.expr_node(repeated).kind.clone() {
            repeated = elem;
        }
        if repeated != init && !matches!(self.state.design.expr_node(repeated).kind, ExprKind::Array(_)) {
            let value = self.carrier_init(repeated, frame)?;
            return Some(self.ir_constant(value, element));
        }
        if shape.dims.len() > 1 {
            self.refuse(file, node.span, DigitalForm::InitialValueOfNestedArrayOfCarriers);
            return None;
        }
        let value = self.carrier_init(init, frame)?;
        Some(match value {
            Constant::Aggregate(id) => Constant::Aggregate(id),
            other => Constant::Aggregate(self.state.builder.aggregate(AggregateData { ty: ValueType::Array(Box::new(element.clone()), shape.count()), variant: None, parts: vec![other; shape.count() as usize] })),
        })
    }

    /// The initial value of the array carrier `decl` declared inside generation (shape `shape`, element type `element`): the
    /// value written is one generated element's, folded for each iteration with its loop variables bound, so every element
    /// starts at its own (§8.2.23). An element that is itself an array takes `[x; K]` or `[a, b, c]` as an array declared
    /// outside generation does. `None` after a diagnostic.
    pub(super) fn generated_array_init(&mut self, decl: DeclId, init: ExprId, shape: &ArrayShape, element: &ValueType) -> Option<Constant> {
        let iterations = self.state.structure.generated.of(decl)?.clone();
        let loops = self.generation_loops(decl).len();
        let inner = ArrayShape { dims: shape.dims[loops.min(shape.dims.len())..].to_vec() };
        let mut parts = Vec::with_capacity(shape.count() as usize);
        for iteration in &iterations {
            let bindings = self.iteration_bindings(Some(iteration));
            parts.extend(self.element_init_parts(decl, init, &inner, element, &bindings)?);
        }
        Some(Constant::Aggregate(self.state.builder.aggregate(AggregateData { ty: ValueType::Array(Box::new(element.clone()), parts.len() as u64), variant: None, parts })))
    }

    /// The initial values of the `inner.count()` carriers one generated element of `decl` holds, `init` folded over
    /// `bindings`: one value, or each element of an array element's own. `None` after a diagnostic.
    fn element_init_parts(&mut self, decl: DeclId, init: ExprId, inner: &ArrayShape, element: &ValueType, bindings: &Bindings) -> Option<Vec<Constant>> {
        let count = inner.count() as usize;
        let mut repeated = init;
        while let ExprKind::ArrayRepeat { elem, .. } = self.state.design.expr_node(repeated).kind.clone() {
            repeated = elem;
        }
        if inner.dims.is_empty() || (repeated != init && !matches!(self.state.design.expr_node(repeated).kind, ExprKind::Array(_))) {
            let value = self.carrier_init_over(repeated, bindings)?;
            return Some(vec![self.ir_constant(value, element); count]);
        }
        if inner.dims.len() > 1 {
            let span = self.state.design.expr_node(init).span;
            self.refuse(self.state.design.decl_data(decl).file, span, DigitalForm::InitialValueOfNestedArrayOfCarriers);
            return None;
        }
        match self.carrier_init_over(init, bindings)? {
            Constant::Aggregate(id) => Some(self.state.builder.aggregate_at(id).parts.clone()),
            other => Some(vec![self.ir_constant(other, element); count]),
        }
    }

    /// The nodes of the array `decl` of nodes in `role`, named `name[0]` to `name[K-1]` (row-major for a nested array):
    /// added to the body's nodes with the array's name, `K` consecutive terminals (the first is returned). `None` after
    /// a diagnostic.
    pub(super) fn add_node_array(&mut self, decl: DeclId, role: piperine_ir::NodeRole, prefix: &str, frame: usize) -> Option<(crate::elab::carrier_arrays::ArrayShape, piperine_ir::Terminal)> {
        let data = self.state.design.decl_data(decl).clone();
        let array = self.state.consts.typed_node_array(decl)?;
        let shape = self.declared_shape(&array, frame, decl, "nodes")?;
        let name = format!("{prefix}{}", data.name.as_ref().map_or("", |n| n.text.as_str()));
        let first = self.state.digital.structure.nodes.len() as u32;
        let init = match &data.kind {
            crate::pom::DeclKind::Signal(s) => s.init,
            _ => None,
        };
        let inits = self.node_array_init(decl, init, &shape, frame)?;
        for flat in 0..shape.count() {
            let element = format!("{name}{}", shape.subscripts(flat));
            let (mut spec, mut plan) = self.lower_node_named(decl, role, &element, frame)?;
            spec.init = inits[flat as usize];
            plan.init = inits[flat as usize];
            self.state.digital.structure.nodes.push((spec, plan));
        }
        // an array of no node names no node: only its name is kept (§1.2.3)
        if shape.count() == 0 {
            self.record_empty_array(frame, decl);
        } else {
            self.state.digital.structure.node_arrays.push(NodeArray { name, first, count: shape.count() as u32, origin: self.state.locator.decl(decl) });
        }
        Some((shape, piperine_ir::Terminal::Node(first)))
    }

    /// The initial potential of each node of the array `decl`: `[x; K]` gives every node `x`, `[a, b, c]` each its own,
    /// no initial value none. `None` after a diagnostic.
    fn node_array_init(&mut self, decl: DeclId, init: Option<ExprId>, shape: &ArrayShape, frame: usize) -> Option<Vec<Option<piperine_ir::Real>>> {
        let count = shape.count() as usize;
        let Some(init) = init else { return Some(vec![None; count]) };
        let file = self.state.design.decl_data(decl).file;
        let bindings = self.frame_bindings(frame);
        let real = |this: &mut Self, expr: ExprId| match this.state.consts.value(expr, &bindings) {
            Ok(Constant::Real(value)) => Some(value),
            Ok(_) => {
                this.refuse(file, this.state.design.expr_node(expr).span, DigitalForm::InitialValueOfArrayOfNodesThat);
                None
            }
            Err(fault) => {
                this.const_fault(&fault, "a node's initial value");
                None
            }
        };
        match self.state.design.expr_node(init).kind.clone() {
            ExprKind::ArrayRepeat { elem, .. } => {
                let value = real(self, elem)?;
                Some(vec![Some(value); count])
            }
            ExprKind::Array(items) if items.len() == count => items.iter().map(|item| real(self, *item).map(Some)).collect(),
            _ => {
                self.refuse(file, self.state.design.expr_node(init).span, DigitalForm::InitialValueOfArrayOfNodesOther);
                None
            }
        }
    }
}
