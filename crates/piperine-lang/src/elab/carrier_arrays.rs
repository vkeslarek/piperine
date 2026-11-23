//! Arrays of carriers and of nodes declared (composite-values CV-23, CV-26; part II §5.1.8): a `signal` or a port
//! `[N; K]` of a discrete nature is **one** carrier of the body with `count` elements of its value type (a nested
//! array is one carrier of the product of the lengths, row-major: `x[r][c]` is element `r * C + c`), so a design holds
//! no storage proportional to `K` beyond what is written. Its initial value is `[x; K]` (every element starts at `x`) or
//! `[a, b, c]` (each element its own), kept on the carrier as one element's value or an array constant. An array of
//! nodes `[Electrical; K]` is `K` nodes of the body named `bl[0]` to `bl[K-1]` (an MNA unknown each is inherent), kept
//! under one name by a `NodeArray` (the host reads `Top.bl[2]` and `Top.bl`).

use piperine_ir::{AggregateData, Constant, NodeArray, ValueType};

use super::elaborator::Elaboration;
use crate::pom::{DeclId, ExprId, ExprKind};
use crate::types::{Length, Ty};

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
    /// The shape of the array type `ty` of a declaration of `frame`: a length that reads a param is folded over the
    /// frame's params (§8.1.2, QC2). `None` when a length does not fold.
    pub(super) fn shape_in(&mut self, ty: &Ty, frame: usize) -> Option<ArrayShape> {
        let bindings = self.frame_bindings(frame);
        let mut dims = Vec::new();
        let mut at = ty;
        while let Ty::Array(element, length) = at {
            dims.push(match length {
                Length::Fixed(length) => *length,
                Length::Generic(crate::types::ConstTerm::Expr(expr)) => match self.consts.value(*expr, &bindings) {
                    Ok(piperine_ir::Constant::Int(count)) => u64::try_from(count).ok()?,
                    _ => return None,
                },
                Length::Generic(_) => return None,
            });
            at = element;
        }
        Some(ArrayShape { dims })
    }

    /// The shape and the innermost value type of the array of carriers `decl` declares; `None` after a diagnostic.
    pub(super) fn carrier_array_shape(&mut self, decl: DeclId, frame: usize) -> Option<(ArrayShape, ValueType)> {
        let data = self.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        let Some(array) = self.types.carrier_array(decl).cloned() else { return None };
        let Some(shape) = self.shape_in(&array, frame) else {
            self.unsupported(file, span, "an array of carriers whose length is not a constant yet");
            return None;
        };
        let Ty::Carrier { value, .. } = array.innermost() else { return None };
        match self.stored_type(value) {
            Some(element) => Some((shape, element)),
            None => {
                self.unsupported(file, span, "a carrier whose value type has no value in the IR");
                None
            }
        }
    }

    /// The initial value of the array carrier `decl` (`init` written), folded: `[x; K]` is one element's value, every
    /// element starting at `x`; `[a, b, c]` an array constant of every element's. `None` after a diagnostic.
    pub(super) fn array_init(&mut self, decl: DeclId, init: ExprId, shape: &ArrayShape, element: &ValueType, frame: usize) -> Option<Constant> {
        let file = self.design.decl_data(decl).file;
        let node = self.design.expr_node(init).clone();
        // `[x; K]` of the innermost level: every element starts at `x`
        let mut repeated = init;
        while let ExprKind::ArrayRepeat { elem, .. } = self.design.expr_node(repeated).kind.clone() {
            repeated = elem;
        }
        if repeated != init && !matches!(self.design.expr_node(repeated).kind, ExprKind::Array(_)) {
            let value = self.carrier_init(repeated, frame)?;
            return Some(self.ir_constant(value, element));
        }
        if shape.dims.len() > 1 {
            self.unsupported(file, node.span, "an initial value of a nested array of carriers other than `[x; K]`");
            return None;
        }
        let value = self.carrier_init(init, frame)?;
        Some(match value {
            Constant::Aggregate(id) => Constant::Aggregate(id),
            other => Constant::Aggregate(self.ir.aggregate(AggregateData { ty: ValueType::Array(Box::new(element.clone()), shape.count()), variant: None, parts: vec![other; shape.count() as usize] })),
        })
    }

    /// The nodes of the array `decl` of nodes in `role`, named `name[0]` to `name[K-1]` (row-major for a nested array):
    /// added to the body's nodes with the array's name, `K` consecutive terminals (the first is returned). `None` after
    /// a diagnostic.
    pub(super) fn add_node_array(&mut self, decl: DeclId, role: piperine_ir::NodeRole, prefix: &str, frame: usize) -> Option<(crate::elab::carrier_arrays::ArrayShape, piperine_ir::Terminal)> {
        let data = self.design.decl_data(decl).clone();
        let array = self.types.node_array(decl).cloned()?;
        let Some(shape) = self.shape_in(&array, frame) else {
            self.unsupported(data.file, data.span, "an array of nodes whose length is not a constant yet");
            return None;
        };
        let name = format!("{prefix}{}", data.name.as_ref().map_or("", |n| n.text.as_str()));
        let first = self.structure.nodes.len() as u32;
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
            self.structure.nodes.push((spec, plan));
        }
        self.structure.node_arrays.push(NodeArray { name, first, count: shape.count() as u32, origin: self.locator.decl(decl) });
        Some((shape, piperine_ir::Terminal::Node(first)))
    }

    /// The initial potential of each node of the array `decl`: `[x; K]` gives every node `x`, `[a, b, c]` each its own,
    /// no initial value none. `None` after a diagnostic.
    fn node_array_init(&mut self, decl: DeclId, init: Option<ExprId>, shape: &ArrayShape, frame: usize) -> Option<Vec<Option<piperine_ir::Real>>> {
        let count = shape.count() as usize;
        let Some(init) = init else { return Some(vec![None; count]) };
        let file = self.design.decl_data(decl).file;
        let bindings = self.frame_bindings(frame);
        let real = |this: &mut Self, expr: ExprId| match this.consts.value(expr, &bindings) {
            Ok(Constant::Real(value)) => Some(value),
            Ok(_) => {
                this.unsupported(file, this.design.expr_node(expr).span, "an initial value of an array of nodes that is not an `f64`");
                None
            }
            Err(fault) => {
                this.const_fault(&fault, "a node's initial value");
                None
            }
        };
        match self.design.expr_node(init).kind.clone() {
            ExprKind::ArrayRepeat { elem, .. } => {
                let value = real(self, elem)?;
                Some(vec![Some(value); count])
            }
            ExprKind::Array(items) if items.len() == count => items.iter().map(|item| real(self, *item).map(Some)).collect(),
            _ => {
                self.unsupported(file, self.design.expr_node(init).span, "an initial value of an array of nodes other than `[x; K]` or `[a, b, c]`");
                None
            }
        }
    }
}
