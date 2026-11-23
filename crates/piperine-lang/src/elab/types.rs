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
use crate::types::{FieldType, Length, Nominal, Ty};

/// The entry of the IR's type arena each reached struct and enum declaration has.
#[derive(Debug, Default)]
pub(super) struct TypeArena {
    /// The arena index of each declaration.
    ids: BTreeMap<DeclId, TypeId>,
    /// The arena index of each instance of a generic struct or enum, by its declaration and arguments (`Pair<f64>`).
    instances: BTreeMap<Nominal, TypeId>,
    /// The IR types of the fields of each arena entry, by arena index: one list for a struct, one
    /// per variant for an enum.
    shapes: Vec<Vec<Vec<ValueType>>>,
}

impl TypeArena {
    /// The arena over `decls`, numbered in their order.
    fn over(decls: &[DeclId]) -> TypeArena {
        TypeArena { ids: decls.iter().enumerate().map(|(index, decl)| (*decl, TypeId(index as u32))).collect(), instances: BTreeMap::new(), shapes: Vec::new() }
    }

    /// The entry of the instance `nominal` of a generic struct or enum, `None` before it is registered.
    fn instance_id(&self, nominal: &Nominal) -> Option<TypeId> {
        self.instances.get(nominal).copied()
    }

    /// The entry of the type `nominal` names: the declaration's own, or its instance when it has arguments.
    fn nominal_id(&self, nominal: &Nominal) -> Option<TypeId> {
        if nominal.arguments.is_empty() { self.id_of(nominal.decl) } else { self.instance_id(nominal) }
    }

    /// The IR types of the fields of the arena struct `id`, in declaration order.
    pub(super) fn field_types(&self, id: TypeId) -> &[ValueType] {
        self.shapes.get(id.index()).and_then(|shape| shape.first()).map_or(&[], Vec::as_slice)
    }

    /// The IR types of the fields of variant `variant` of the arena enum `id`.
    /// Whether the enum `id` has no field in any variant: a value of it is its tag.
    pub(super) fn is_unit_only(&self, id: TypeId) -> bool {
        self.shapes.get(id.index()).is_some_and(|variants| variants.iter().all(Vec::is_empty))
    }

    /// The types of the fields of variant `variant` of the enum `id`, in order; none for a variant of no
    /// field or an entry the arena does not hold.
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
            Ty::Struct(nominal) => self.nominal_id(nominal).map(ValueType::Struct),
            Ty::Enum(nominal) => self.nominal_id(nominal).map(ValueType::Enum),
            Ty::Tuple(items) => items.iter().map(|item| self.of(item, scalar).or_else(|| scalar(item))).collect::<Option<Vec<_>>>().map(ValueType::Tuple),
            Ty::Array(element, Length::Fixed(length)) => self.of(element, scalar).or_else(|| scalar(element)).map(|element| ValueType::Array(Box::new(element), *length)),
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
        // a generic declaration has no entry of its own: its fields mention parameters; each instance has one
        let decls: Vec<DeclId> = self.types.aggregate_types().into_iter().filter(|decl| self.design.decl_data(*decl).generics_span.is_none()).collect();
        self.type_arena = TypeArena::over(&decls);
        for decl in decls {
            let data = self.type_data(decl, &crate::types::Substitution::new());
            self.add_type_entry(data);
        }
        for nominal in self.types.generic_instances() {
            self.register_instance(&nominal);
        }
    }

    /// Adds the arena entry `data`, after the ones before it.
    fn add_type_entry(&mut self, data: TypeData) -> TypeId {
        let shape = match &data.kind {
            TypeKind::Struct { fields, .. } => vec![fields.iter().map(|(_, ty)| ty.clone()).collect()],
            TypeKind::Enum { variants, .. } => variants.iter().map(|v| v.fields.iter().map(|(_, ty)| ty.clone()).collect()).collect(),
        };
        self.type_arena.shapes.push(shape);
        self.ir.type_data(data)
    }

    /// The arena entry of every instance of a generic struct or enum the type `ty` holds, an instance's arguments
    /// before it, so a field of an instance finds its own type (T38). A type that still mentions a parameter has none.
    pub(super) fn register_instances(&mut self, ty: &Ty) {
        let mut found = Vec::new();
        ty.generic_instances(&mut found);
        for nominal in found {
            self.register_instance(&nominal);
        }
    }

    /// The arena entry of the instance `nominal`, named `Pair<f64>` and recording `instance`, its fields the
    /// declaration's with the arguments put in the place of the parameters; nothing when it is registered.
    fn register_instance(&mut self, nominal: &Nominal) {
        if self.type_arena.instance_id(nominal).is_some() {
            return;
        }
        let substitution = crate::types::Substitution::of(&crate::types::GenericSignature::of(self.design, nominal.decl), &nominal.arguments);
        let mut data = self.type_data(nominal.decl, &substitution);
        let ty = if matches!(self.design.decl_data(nominal.decl).kind, DeclKind::Enum(_)) { Ty::Enum(nominal.clone()) } else { Ty::Struct(nominal.clone()) };
        data.name = ty.source_name(self.design);
        data.path = format!("{}{}", data.path, nominal.arguments_name(self.design));
        data.instance = Some(piperine_ir::GenericInstance { item: self.design.decl_data(nominal.decl).name.as_ref().map_or_else(String::new, |n| n.text.clone()), arguments: nominal.arguments.iter().map(|argument| argument.name(self.design)).collect() });
        let id = self.add_type_entry(data);
        self.type_arena.instances.insert(nominal.clone(), id);
    }

    /// The arena entry of the declaration `decl`: its name, path, origin, fields or variants and the
    /// traits it derives.
    fn type_data(&self, decl: DeclId, substitution: &crate::types::Substitution) -> TypeData {
        let data = self.design.decl_data(decl);
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let kind = match &data.kind {
            DeclKind::Struct(s) => TypeKind::Struct { form: Self::fields_form(s.form), fields: self.field_list(decl, substitution) },
            _ => self.enum_kind(decl, substitution),
        };
        TypeData { name, path: self.design.decl(decl).qualified_name(), origin: self.locator.decl(decl), kind, derives: self.derives_of(decl), instance: None, repr: Default::default() }
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
    fn field_list(&self, owner: DeclId, substitution: &crate::types::Substitution) -> Vec<(String, ValueType)> {
        let fields: &[FieldType] = self.types.fields(owner).unwrap_or(&[]);
        fields.iter().enumerate().map(|(index, field)| (field.name.clone().unwrap_or_else(|| index.to_string()), self.field_type(&substitution.apply(&field.ty)))).collect()
    }

    /// The IR type of a field: the type a value of it has, `Option` held as one value.
    fn field_type(&self, ty: &Ty) -> ValueType {
        match ty {
            Ty::Option(inner) => ValueType::Option(Box::new(self.field_type(inner))),
            other => self.value_type(other).unwrap_or(ValueType::Unit),
        }
    }

    /// The IR type of a value held as one value, which a carrier, a param or a record field is: as
    /// `value_type`, with an `Option` kept as `ValueType::Option` (the pair of value code is for
    /// code that reads its halves) at any depth of a tuple or an array. `None` for a type no
    /// value has.
    pub(super) fn stored_type(&self, ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::Option(inner) => self.stored_type(inner).map(|inner| ValueType::Option(Box::new(inner))),
            Ty::Tuple(items) => items.iter().map(|item| self.stored_type(item)).collect::<Option<Vec<_>>>().map(ValueType::Tuple),
            Ty::Array(element, Length::Fixed(length)) => self.stored_type(element).map(|element| ValueType::Array(Box::new(element), *length)),
            other => self.value_type(other),
        }
    }

    /// The tag and the variants of the enum `decl`.
    fn enum_kind(&self, decl: DeclId, substitution: &crate::types::Substitution) -> TypeKind {
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
                VariantData { name: data.name.as_ref().map_or_else(String::new, |n| n.text.clone()), discriminant, form, fields: self.field_list(variant, substitution) }
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
