//! The type arena of one run: every struct and enum the check reached becomes one
//! [`TypeData`] of the IR, so a source type prints as one Rust type (CV-20 AC 4) and a host reads
//! its variant names (CV-22).
//!
//! The arena is built once, before any body is lowered, in the declaration order of the design, so
//! [`ValueTypes::of`](super::lower::ValueTypes) only looks a declaration up. A unit enum is an IR
//! enum like any other: its tag is its `Repr` layout (§2 rule 5), so carried bytes do not change.

use std::collections::BTreeMap;

use piperine_ir::{AggregateData, Constant, Derives, FieldsForm, TypeData, TypeId, TypeKind, ValueType, VariantData};

use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, FieldsForm as SourceForm, Reflect};
use crate::types::{FieldType, Ty};

/// The entry of the IR's type arena each reached struct and enum declaration has.
#[derive(Debug, Default)]
pub(super) struct TypeArena {
    /// The arena index of each declaration.
    ids: BTreeMap<DeclId, TypeId>,
    /// The IR types of the fields of each arena entry, by arena index: one list for a struct, one
    /// per variant for an enum.
    shapes: Vec<Vec<Vec<ValueType>>>,
}

impl TypeArena {
    /// The arena over `decls`, numbered in their order.
    fn over(decls: &[DeclId]) -> TypeArena {
        TypeArena { ids: decls.iter().enumerate().map(|(index, decl)| (*decl, TypeId(index as u32))).collect(), shapes: Vec::new() }
    }

    /// The IR types of the fields of the arena struct `id`, in declaration order.
    pub(super) fn field_types(&self, id: TypeId) -> &[ValueType] {
        self.shapes.get(id.index()).and_then(|shape| shape.first()).map_or(&[], Vec::as_slice)
    }

    /// The IR types of the fields of variant `variant` of the arena enum `id`.
    pub(super) fn variant_field_types(&self, id: TypeId, variant: u32) -> &[ValueType] {
        self.shapes.get(id.index()).and_then(|shape| shape.get(variant as usize)).map_or(&[], Vec::as_slice)
    }

    /// The entry of the struct or enum `decl`, `None` when the check did not reach it.
    pub(super) fn id_of(&self, decl: DeclId) -> Option<TypeId> {
        self.ids.get(&decl).copied()
    }

    /// The IR type of `ty` where it is built of arena entries and scalars: a struct, an enum, a tuple
    /// or an array of them. `None` for an `Option` (a pair of values in value code, §2 of
    /// `compound.rs`) and for a type no value has.
    pub(super) fn of(&self, ty: &Ty, scalar: impl Fn(&Ty) -> Option<ValueType> + Copy) -> Option<ValueType> {
        match ty {
            Ty::Struct(decl) => self.id_of(*decl).map(ValueType::Struct),
            Ty::Enum(decl) => self.id_of(*decl).map(ValueType::Enum),
            Ty::Tuple(items) => items.iter().map(|item| self.of(item, scalar).or_else(|| scalar(item))).collect::<Option<Vec<_>>>().map(ValueType::Tuple),
            Ty::Array(element, length) => self.of(element, scalar).or_else(|| scalar(element)).map(|element| ValueType::Array(Box::new(element), *length)),
            _ => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// A folded value as the IR holds one of type `ty`: the constant evaluator names a unit variant
    /// by its position, which the IR holds as a composite constant of the variant.
    pub(super) fn ir_constant(&mut self, value: Constant, ty: &ValueType) -> Constant {
        match (value, ty) {
            (Constant::Int(position), ValueType::Enum(_)) => {
                Constant::Aggregate(self.ir.aggregate(AggregateData { ty: ty.clone(), variant: Some(position as u32), parts: Vec::new() }))
            }
            (other, _) => other,
        }
    }

    /// Adds every struct and enum the check reached to the IR's type arena, in the order of the
    /// design (`TypeArena::over`), before any body is lowered.
    pub(super) fn build_type_arena(&mut self) {
        let decls = self.types.aggregate_types();
        self.type_arena = TypeArena::over(&decls);
        for decl in decls {
            let data = self.type_data(decl);
            let shape = match &data.kind {
                TypeKind::Struct { fields, .. } => vec![fields.iter().map(|(_, ty)| ty.clone()).collect()],
                TypeKind::Enum { variants, .. } => variants.iter().map(|v| v.fields.iter().map(|(_, ty)| ty.clone()).collect()).collect(),
            };
            self.type_arena.shapes.push(shape);
            self.ir.type_data(data);
        }
    }

    /// The arena entry of the declaration `decl`: its name, path, origin, fields or variants and the
    /// traits it derives.
    fn type_data(&self, decl: DeclId) -> TypeData {
        let data = self.design.decl_data(decl);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let kind = match &data.kind {
            DeclKind::Struct(s) => TypeKind::Struct { form: Self::fields_form(s.form), fields: self.field_list(decl) },
            _ => self.enum_kind(decl),
        };
        TypeData { name, path: self.design.decl(decl).qualified_name(), origin: self.locator.decl(decl), kind, derives: self.derives_of(decl) }
    }

    /// The source's way of writing fields as the IR's.
    fn fields_form(form: SourceForm) -> FieldsForm {
        match form {
            SourceForm::Named => FieldsForm::Named,
            SourceForm::Tuple => FieldsForm::Tuple,
            SourceForm::Unit => FieldsForm::Unit,
        }
    }

    /// The fields of the struct or variant `owner` with their IR types; a positional field is named by
    /// its index.
    fn field_list(&self, owner: DeclId) -> Vec<(String, ValueType)> {
        let fields: &[FieldType] = self.types.fields(owner).unwrap_or(&[]);
        fields.iter().enumerate().map(|(index, field)| (field.name.clone().unwrap_or_else(|| index.to_string()), self.field_type(&field.ty))).collect()
    }

    /// The IR type of a field: the type a value of it has, `Option` held as one value.
    fn field_type(&self, ty: &Ty) -> ValueType {
        match ty {
            Ty::Option(inner) => ValueType::Option(Box::new(self.field_type(inner))),
            other => self.value_type(other).unwrap_or(ValueType::Unit),
        }
    }

    /// The tag and the variants of the enum `decl`.
    fn enum_kind(&self, decl: DeclId) -> TypeKind {
        let tag = self.types.enum_tag(decl);
        let variants = self.design.decl_data(decl).children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Variant(_)));
        let variants = variants
            .enumerate()
            .map(|(index, variant)| {
                let data = self.design.decl_data(variant);
                let form = match &data.kind {
                    DeclKind::Variant(v) => Self::fields_form(v.form),
                    _ => FieldsForm::Unit,
                };
                let discriminant = tag.and_then(|t| t.discriminants.get(index).copied()).unwrap_or(index as i128);
                VariantData { name: data.name.as_ref().map_or_else(String::new, |n| n.text.clone()), discriminant, form, fields: self.field_list(variant) }
            })
            .collect();
        TypeKind::Enum { tag_levels: tag.map_or(0, |t| t.tag_levels), variants }
    }

    /// The traits `decl` derives. Every value of the language is copied where it is used (§1.4.6),
    /// so the generated Rust type always is `Clone` and `Copy`; a carried type needs `PartialEq`
    /// for its encoding, so `Repr` implies it.
    fn derives_of(&self, decl: DeclId) -> Derives {
        let listed = |name: &str| self.types.derives(decl, name);
        let repr = listed("Repr");
        Derives {
            clone: true,
            copy: true,
            partial_eq: listed("PartialEq") || repr,
            eq: listed("Eq"),
            partial_ord: listed("PartialOrd"),
            ord: listed("Ord"),
            default: listed("Default"),
            debug: listed("Debug"),
            hash: listed("Hash"),
            repr,
        }
    }
}
