//! The type arena's invariants (composite-values T30, CV-20): every field type is well formed, no
//! type contains itself (§1.3.8), the shape of a struct agrees with its form, an enum's tag holds
//! its discriminants (§1.3.7, §2 rule 5) and its derives imply each other as Appendix H.8 says.

use std::collections::BTreeSet;

use super::Checker;
use crate::model::{Derives, FieldsForm, TypeData, TypeKind, ValueType};

impl Checker<'_> {
    /// Checks each entry of the type arena, then the arena's containment as a whole.
    pub(super) fn type_arena(&mut self) {
        let ir = self.ir;
        let mut paths: BTreeSet<&str> = BTreeSet::new();
        for (index, data) in ir.types.iter().enumerate() {
            let item = format!("type t{index} {}", data.name);
            if !paths.insert(data.path.as_str()) {
                self.report(item.clone(), data.origin.clone(), format!("two types have the path {}", data.path));
            }
            self.derive_rules(&item, data);
            match &data.kind {
                TypeKind::Struct { form, fields } => self.fields(&item, data, *form, fields),
                TypeKind::Enum { tag_levels, variants } => {
                    self.tag_holds_discriminants(&item, data, *tag_levels, variants.iter().map(|v| (v.name.as_str(), v.discriminant)));
                    for variant in variants {
                        self.fields(&format!("{item} variant {}", variant.name), data, variant.form, &variant.fields);
                    }
                }
            }
        }
        for index in 0..ir.types.len() {
            if self.contains_itself(index) {
                let data = &ir.types[index];
                self.report(format!("type t{index} {}", data.name), data.origin.clone(), format!("type {} contains itself (§1.3.8)", data.name));
            }
        }
    }

    /// The field types are well formed and the form agrees with the field list.
    fn fields(&mut self, item: &str, data: &TypeData, form: FieldsForm, fields: &[(String, ValueType)]) {
        match form {
            FieldsForm::Unit if !fields.is_empty() => self.report(item.to_string(), data.origin.clone(), "a unit form has no fields".to_string()),
            FieldsForm::Tuple if fields.iter().enumerate().any(|(i, (name, _))| *name != i.to_string()) => {
                self.report(item.to_string(), data.origin.clone(), "the fields of a tuple form are named 0, 1, ... in order".to_string());
            }
            FieldsForm::Named | FieldsForm::Tuple | FieldsForm::Unit => {}
        }
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for (name, ty) in fields {
            if !names.insert(name.as_str()) {
                self.report(item.to_string(), data.origin.clone(), format!("two fields are named {name}"));
            }
            if let Some(what) = self.malformed(ty) {
                self.report(item.to_string(), data.origin.clone(), format!("field {name}: {what}"));
            }
        }
    }

    /// The tag has 1..=64 levels and holds each discriminant, and no two variants share one.
    fn tag_holds_discriminants<'v>(&mut self, item: &str, data: &TypeData, tag_levels: u32, variants: impl Iterator<Item = (&'v str, i128)>) {
        if !(1..=64).contains(&tag_levels) {
            self.report(item.to_string(), data.origin.clone(), format!("a tag of {tag_levels} levels is outside 1..=64"));
            return;
        }
        let mut seen: BTreeSet<i128> = BTreeSet::new();
        for (name, discriminant) in variants {
            if !(0..(1i128 << tag_levels)).contains(&discriminant) {
                self.report(item.to_string(), data.origin.clone(), format!("the discriminant {discriminant} of {name} does not fit a {tag_levels}-level tag"));
            }
            if !seen.insert(discriminant) {
                self.report(item.to_string(), data.origin.clone(), format!("two variants have the discriminant {discriminant}"));
            }
        }
    }

    /// H.8 rules 1 to 4: what a derive requires, and that an enum does not derive `Default`.
    fn derive_rules(&mut self, item: &str, data: &TypeData) {
        let Derives { clone, copy, partial_eq, eq, partial_ord, ord, default, .. } = data.derives;
        let needs = [("Copy", copy, "Clone", clone), ("Eq", eq, "PartialEq", partial_eq), ("PartialOrd", partial_ord, "PartialEq", partial_eq), ("Ord", ord, "Eq", eq), ("Ord", ord, "PartialOrd", partial_ord)];
        for (derived, on, required, present) in needs {
            if on && !present {
                self.report(item.to_string(), data.origin.clone(), format!("derive {derived} requires {required} (H.8)"));
            }
        }
        if default && matches!(data.kind, TypeKind::Enum { .. }) {
            self.report(item.to_string(), data.origin.clone(), "an enum does not derive Default (H.8 rule 4)".to_string());
        }
    }

    /// Whether the type `start` reaches itself through the fields of structs, variants, tuples,
    /// arrays and options.
    fn contains_itself(&self, start: usize) -> bool {
        let mut visited: BTreeSet<usize> = BTreeSet::new();
        let mut stack = self.parts_of_type(start);
        while let Some(ty) = stack.pop() {
            match ty {
                ValueType::Struct(id) | ValueType::Enum(id) => {
                    let index = id.index();
                    if index == start {
                        return true;
                    }
                    if index < self.ir.types.len() && visited.insert(index) {
                        stack.extend(self.parts_of_type(index));
                    }
                }
                ValueType::Tuple(parts) => stack.extend(parts.clone()),
                ValueType::Array(element, _) | ValueType::Option(element) | ValueType::Slice(element) => stack.push(*element.clone()),
                ValueType::Flagged(inner) => stack.push(*inner.clone()),
                ValueType::F64 | ValueType::Bool | ValueType::Unit | ValueType::Fn(..) | ValueType::UInt(_) | ValueType::Int(_) => {}
            }
        }
        false
    }

    /// The field types of the type at `index`, all variants included.
    fn parts_of_type(&self, index: usize) -> Vec<ValueType> {
        match &self.ir.types[index].kind {
            TypeKind::Struct { fields, .. } => fields.iter().map(|(_, ty)| ty.clone()).collect(),
            TypeKind::Enum { variants, .. } => variants.iter().flat_map(|v| v.fields.iter().map(|(_, ty)| ty.clone())).collect(),
        }
    }
}
