//! The array pin (composite-values CV-23, CV-25, CV-27; abi.md §3): what an array of discrete carriers is in the
//! shape. An array carrier (`Carrier::count`) is **one** `DISCRETE` pin with a `count` and the layout of one element;
//! its pin's `init` is one element (`[x; K]`) or every element (`[a, b, c]`). A read of the whole array or of a
//! run-time element is a `WHOLE` cell (`Source::Whole`, a reaction's), a read of a constant element a cell naming the
//! element (`Source::Element`). A commit names an element at run time (`Host.commit_element`); its statement declares
//! the byte range inside one element, which is the whole element or the levels of a field or of a value array's element
//! (`Statement.offset`, `Statement.len`).

use piperine_ir::{Carrier, Constant, Expr, Ir, Projection, TypeKind, ValueType};

use crate::pin::PinLayout;

/// The array pin of an array carrier: how its initial value and the byte range of a commit are decided.
pub struct ArrayPin;

impl ArrayPin {
    /// The bytes of the initial value of array carrier `carrier` (`init` is `Some`): one element's, or every element's
    /// when the constant is an aggregate of the array type `[T; count]`. `None` when it is a value of neither.
    pub fn init(ir: &Ir, carrier: &Carrier, init: Constant) -> Option<Vec<u8>> {
        let count = carrier.count?;
        let every = ValueType::Array(Box::new(carrier.ty.clone()), count);
        match init {
            Constant::Aggregate(id) if ir.aggregate(id).ty == every => PinLayout::bytes_in(ir, &every, init),
            _ => PinLayout::bytes_in(ir, &carrier.ty, init),
        }
    }

    /// The byte range `(offset, len)` inside one element of `carrier` that a commit through `path` writes: the whole
    /// element for an empty path, else the levels the field and constant element steps name. `None` for a step that
    /// is no field, no constant index, or leaves a type no pin holds (`Ir::check` refuses those targets).
    pub fn range(ir: &Ir, element: &ValueType, path: &[Projection]) -> Option<(u32, u32)> {
        let mut ty = element.clone();
        let mut offset = 0u32;
        for step in path {
            let (next, skipped) = Self::step(ir, &ty, step)?;
            offset += skipped;
            ty = next;
        }
        Some((offset, PinLayout::in_ir(ir, &ty)?.size()))
    }

    /// The type a step into `ty` names and the bytes of the parts before it.
    fn step(ir: &Ir, ty: &ValueType, step: &Projection) -> Option<(ValueType, u32)> {
        match (step, ty) {
            (Projection::Field(k), ValueType::Tuple(parts)) => Self::field(ir, parts.iter().cloned().collect(), *k),
            (Projection::Field(k), ValueType::Struct(id)) => {
                let TypeKind::Struct { fields, .. } = &ir.type_data(*id).kind else { return None };
                Self::field(ir, fields.iter().map(|(_, ty)| ty.clone()).collect(), *k)
            }
            (Projection::Index(index), ValueType::Array(element, _)) => {
                let Expr::Int(n) = ir.expr(*index) else { return None };
                let width = PinLayout::in_ir(ir, element)?.size();
                Some(((**element).clone(), u32::try_from(*n).ok()?.checked_mul(width)?))
            }
            _ => None,
        }
    }

    /// Field `k` of parts of types `parts`: its type and the width of the fields before it.
    fn field(ir: &Ir, parts: Vec<ValueType>, k: u32) -> Option<(ValueType, u32)> {
        let mut skipped = 0;
        for part in parts.iter().take(k as usize) {
            skipped += PinLayout::in_ir(ir, part)?.size();
        }
        Some((parts.get(k as usize)?.clone(), skipped))
    }
}
