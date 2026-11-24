//! The number widths the interpreter does not run (R1, CV-34): `f16`, `f32`, `f128` and an integer
//! wider than 64 bits. The interpreter is frozen at the M1 analog subset, so
//! [`Interpreter::new`](crate::Interpreter::new) names each such type a body holds and refuses the
//! design instead of running it wrong. The check comes before `CompositeForms`.

use std::collections::BTreeSet;

use piperine_ir::{Body, Ir, TypeKind, ValueType};

use crate::composite::CompositeForms;

/// The types of one body the interpreter cannot hold.
pub(crate) struct NumberWidths;

impl NumberWidths {
    /// One refusal per number-width type `body` holds in a param, a local, a carrier, a stream
    /// field, a reaction local or any expression it evaluates, each type once, naming the type and
    /// the compiled backend that runs it.
    pub(crate) fn refused(body: Body<'_>) -> Vec<String> {
        let ir = body.ir();
        let mut types: Vec<&ValueType> = Vec::new();
        types.extend(body.params().iter().map(|param| &param.ty));
        types.extend(body.locals().iter().map(|local| &local.ty));
        types.extend(body.carriers().iter().map(|carrier| &carrier.ty));
        types.extend(body.streams().iter().flat_map(|stream| stream.fields.iter().map(|field| &field.ty)));
        types.extend(body.reactions().iter().flat_map(|reaction| reaction.locals.iter().map(|local| &local.ty)));
        types.extend(CompositeForms::reached_expressions(body).into_iter().map(|id| ir.expr_type(id)));
        let mut named: BTreeSet<String> = BTreeSet::new();
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        for ty in types {
            Self::collect(ty, ir, &mut named, &mut seen);
        }
        named.into_iter().map(|name| format!("body {} holds a value of type {name}: the interpreter runs the M1 analog subset only, and `f16`, `f32`, `f128` and integers wider than 64 bits are not in it (use the transpiled backend, `--backend rust`)", body.name())).collect()
    }

    /// Adds the name of every number-width type `ty` is or contains (through tuples, arrays,
    /// options, function types and the fields of structs and enums, each arena type once through
    /// `seen`) to `named`.
    fn collect(ty: &ValueType, ir: &Ir, named: &mut BTreeSet<String>, seen: &mut BTreeSet<usize>) {
        match ty {
            ValueType::F32 | ValueType::F16 | ValueType::F128 => {
                named.insert(ty.to_string());
            }
            ValueType::UInt(_) | ValueType::Int(_) if ty.is_wide_integer() => {
                named.insert(ty.to_string());
            }
            ValueType::Tuple(parts) => parts.iter().for_each(|part| Self::collect(part, ir, named, seen)),
            ValueType::Array(element, _) | ValueType::Option(element) => Self::collect(element, ir, named, seen),
            ValueType::Fn(params, ret) => {
                params.iter().for_each(|part| Self::collect(part, ir, named, seen));
                Self::collect(ret, ir, named, seen);
            }
            ValueType::Struct(id) | ValueType::Enum(id) if seen.insert(id.index()) => {
                let fields: Vec<&ValueType> = match &ir.type_data(*id).kind {
                    TypeKind::Struct { fields, .. } => fields.iter().map(|(_, field)| field).collect(),
                    TypeKind::Enum { variants, .. } => variants.iter().flat_map(|variant| variant.fields.iter().map(|(_, field)| field)).collect(),
                };
                fields.into_iter().for_each(|field| Self::collect(field, ir, named, seen));
            }
            _ => {}
        }
    }
}
