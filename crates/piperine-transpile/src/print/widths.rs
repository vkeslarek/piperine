//! The number widths as Rust (composite-values phase D): how `f32`, `f16`, `f128` and an integer
//! wider than 64 bits are spelled in generated code. `f32` is Rust's own `f32`; the others are the
//! types of the embedded numeric library (`rt::Float16`, `rt::Float128`, `rt::WideUInt<N, L>`,
//! `rt::WideInt<N, L>`, with `L = ceil(N/64)` limbs).

use std::collections::BTreeSet;

use piperine_ir::{Ir, TypeKind, ValueType};

/// The Rust spelling of the number-width types.
pub struct NumberTypes;

impl NumberTypes {
    /// The Rust type of `ty`, which is one of the number widths (`f32`, `f16`, `f128`, an integer
    /// over 64 bits); any other type is spelled as the IR spells it (callers dispatch on the
    /// number widths first).
    pub fn spelled(ty: &ValueType) -> String {
        match ty {
            ValueType::F32 => "f32".to_string(),
            ValueType::F16 => "rt::Float16".to_string(),
            ValueType::F128 => "rt::Float128".to_string(),
            ValueType::UInt(width) if *width > 64 => format!("rt::WideUInt<{width}, {}>", width.div_ceil(64)),
            ValueType::Int(width) if *width > 64 => format!("rt::WideInt<{width}, {}>", width.div_ceil(64)),
            other => other.to_string(),
        }
    }
}

impl NumberTypes {
    /// Whether any type the design writes is a number width needing the embedded library: `f16`,
    /// `f128` or an integer wider than 64 bits, anywhere in a param, local, carrier, stream field,
    /// function or expression, through aggregates. (`f32` is Rust's own type and needs nothing.)
    pub fn used_by(ir: &Ir) -> bool {
        let mut seen = BTreeSet::new();
        let mut types: Vec<&ValueType> = ir.expression_types().collect();
        for body in ir.bodies() {
            types.extend(body.params().iter().map(|param| &param.ty));
            types.extend(body.locals().iter().map(|local| &local.ty));
            types.extend(body.carriers().iter().map(|carrier| &carrier.ty));
            types.extend(body.streams().iter().flat_map(|stream| stream.fields.iter().map(|field| &field.ty)));
            types.extend(body.reactions().iter().flat_map(|reaction| reaction.locals.iter().map(|local| &local.ty)));
        }
        types.into_iter().any(|ty| Self::within(ir, ty, &mut seen))
    }

    /// Whether `ty` is, or holds through tuples, arrays, options and the fields of arena types
    /// (each walked once through `seen`), a type that needs the embedded library.
    fn within(ir: &Ir, ty: &ValueType, seen: &mut BTreeSet<usize>) -> bool {
        match ty {
            ValueType::F16 | ValueType::F128 => true,
            ValueType::UInt(width) | ValueType::Int(width) => *width > 64,
            ValueType::Tuple(parts) => parts.iter().any(|part| Self::within(ir, part, seen)),
            ValueType::Array(element, _) | ValueType::Option(element) => Self::within(ir, element, seen),
            ValueType::Fn(params, ret) => params.iter().chain(std::iter::once(&**ret)).any(|part| Self::within(ir, part, seen)),
            ValueType::Struct(id) | ValueType::Enum(id) if seen.insert(id.index()) => match &ir.type_data(*id).kind {
                TypeKind::Struct { fields, .. } => fields.iter().any(|(_, field)| Self::within(ir, field, seen)),
                TypeKind::Enum { variants, .. } => variants.iter().any(|variant| variant.fields.iter().any(|(_, field)| Self::within(ir, field, seen))),
            },
            ValueType::Struct(_) | ValueType::Enum(_) | ValueType::F64 | ValueType::F32 | ValueType::Bool | ValueType::Unit => false,
        }
    }
}
