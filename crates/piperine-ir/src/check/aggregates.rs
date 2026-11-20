//! The type arena's invariants (composite-values T30, CV-20): every field type is well formed, no
//! type contains itself (§1.3.8), the shape of a struct agrees with its form, an enum's tag holds
//! its discriminants (§1.3.7, §2 rule 5) and its derives imply each other as Appendix H.8 says.

use std::collections::BTreeSet;

use super::{Checker, Ctx, Task, Walk};
use crate::ids::ExprId;
use crate::model::{Derives, Expr, FieldsForm, Place, Projection, TypeData, TypeKind, ValueType};
use crate::typing::Typing;

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

impl Checker<'_> {
    /// The field types and the tag-independent shape of struct `ty`, `None` when it is not a struct.
    fn struct_fields(&self, ty: crate::ids::TypeId) -> Option<Vec<ValueType>> {
        match &self.ir.types.get(ty.index())?.kind {
            TypeKind::Struct { fields, .. } => Some(fields.iter().map(|(_, ty)| ty.clone()).collect()),
            TypeKind::Enum { .. } => None,
        }
    }

    /// Each of `parts` is a `want`-typed value (a field, an element); names `what` otherwise.
    fn parts_have_types(&mut self, w: &Walk, parts: &[ExprId], want: &[ValueType], what: &str) {
        if parts.len() != want.len() {
            let origin = parts.first().map_or_else(Checker::unknown, |p| self.expr_origin(*p));
            self.wrong(w, origin, format!("{what} takes {} parts, found {}", want.len(), parts.len()));
            return;
        }
        for (part, ty) in parts.iter().zip(want) {
            self.want(w, *part, ty, what);
        }
    }

    /// The checks of one aggregate expression (when `first`), then the walk of its operands.
    pub(super) fn aggregate_form(&mut self, w: &mut Walk, id: ExprId, first: bool, ctx: Ctx, next: &mut Vec<Task>) {
        let data = &self.ir.exprs[id.index()];
        let (kind, ty, origin) = (&data.kind, &data.ty, data.origin.clone());
        if first {
            match kind {
                Expr::Tuple(parts) => {
                    let types: Option<Vec<ValueType>> = parts.iter().map(|p| self.ty(*p).cloned()).collect();
                    if let Some(types) = types {
                        self.own(w, &origin, ty, &ValueType::Tuple(types));
                    }
                }
                Expr::Array(parts) => match ty {
                    ValueType::Array(element, length) => {
                        if *length != parts.len() as u64 {
                            self.wrong(w, origin.clone(), format!("an array of {length} elements is written with {}", parts.len()));
                        }
                        let wanted = vec![(**element).clone(); parts.len()];
                        self.parts_have_types(w, parts, &wanted, "an element of an array");
                    }
                    other => self.wrong(w, origin.clone(), format!("an array expression is typed {other}, an array type is required")),
                },
                Expr::ArrayRepeat { element, count } => {
                    if let Some(element_ty) = self.ty(*element) {
                        self.own(w, &origin, ty, &ValueType::Array(Box::new(element_ty.clone()), *count));
                    }
                }
                Expr::StructOf { ty: id, fields } => match self.struct_fields(*id) {
                    Some(wanted) => {
                        self.own(w, &origin, ty, &ValueType::Struct(*id));
                        self.parts_have_types(w, fields, &wanted, "a field of a struct expression");
                    }
                    None => self.wrong(w, origin.clone(), format!("struct expression names type t{}, which is not a struct of the arena", id.0)),
                },
                Expr::VariantOf { ty: id, variant, fields } => match self.ir.types.get(id.index()).map(|t| &t.kind) {
                    Some(TypeKind::Enum { variants, .. }) => match variants.get(*variant as usize) {
                        Some(v) => {
                            let wanted: Vec<ValueType> = v.fields.iter().map(|(_, ty)| ty.clone()).collect();
                            self.own(w, &origin, ty, &ValueType::Enum(*id));
                            self.parts_have_types(w, fields, &wanted, "a field of a variant");
                        }
                        None => self.wrong(w, origin.clone(), format!("variant {variant} out of range of enum t{}", id.0)),
                    },
                    _ => self.wrong(w, origin.clone(), format!("variant expression names type t{}, which is not an enum of the arena", id.0)),
                },
                Expr::Field { expr, index } => {
                    if let Some(base) = self.ty(*expr) {
                        match self.field_type(base, *index) {
                            Some(field) => self.own(w, &origin, ty, &field),
                            None => self.wrong(w, origin.clone(), format!("field {index} of a value of type {base}, which has no such field")),
                        }
                    }
                }
                Expr::Index { expr, index } => {
                    self.integer_index(w, *index, "an index");
                    if let Some(base) = self.ty(*expr) {
                        match Checker::element_type(base) {
                            Some(element) => self.own(w, &origin, ty, element),
                            None => self.wrong(w, origin.clone(), format!("an index applied to a value of type {base}, which is not an array or a slice")),
                        }
                    }
                }
                Expr::SliceOf { expr, low, high, .. } => {
                    for bound in low.iter().chain(high) {
                        self.integer_index(w, *bound, "a slice bound");
                    }
                    if let Some(base) = self.ty(*expr) {
                        match Checker::element_type(base) {
                            Some(element) => self.own(w, &origin, ty, &ValueType::Slice(Box::new(element.clone()))),
                            None => self.wrong(w, origin.clone(), format!("a slice of a value of type {base}, which is not an array or a slice")),
                        }
                    }
                }
                Expr::Length(expr) => {
                    if let Some(base) = self.ty(*expr) {
                        if Checker::element_type(base).is_none() {
                            self.wrong(w, origin.clone(), format!("`len` of a value of type {base}, which is not an array or a slice"));
                        }
                    }
                    self.own(w, &origin, ty, &ValueType::UInt(64));
                }
                _ => {}
            }
        }
        next.extend(kind.operands().into_iter().map(|operand| Task::Expr(operand, ctx)));
    }

    /// The element type of an array or a slice.
    fn element_type(ty: &ValueType) -> Option<&ValueType> {
        match ty {
            ValueType::Array(element, _) | ValueType::Slice(element) => Some(element),
            _ => None,
        }
    }

    /// An index or a slice bound has an integer type.
    fn integer_index(&mut self, w: &Walk, index: ExprId, what: &str) {
        if let Some(ty) = self.ty(index) {
            if !Typing::base(ty).is_integer() {
                let origin = self.expr_origin(index);
                self.wrong(w, origin, format!("{what} must have an integer type, found {ty}"));
            }
        }
    }

    /// The type of field `index` of a struct or a tuple.
    fn field_type(&self, base: &ValueType, index: u32) -> Option<ValueType> {
        match base {
            ValueType::Tuple(parts) => parts.get(index as usize).cloned(),
            ValueType::Struct(id) => self.struct_fields(*id)?.get(index as usize).cloned(),
            _ => None,
        }
    }

    /// The type `place` reaches inside a local of type `target`: a field of a struct or a tuple, an
    /// element of an array (an index of an integer type). A step that does not fit is named.
    pub(super) fn place_type(&mut self, w: &Walk, origin: &crate::model::Origin, place: &Place, target: ValueType) -> Option<ValueType> {
        let mut current = target;
        for step in &place.path {
            let reached = match step {
                Projection::Field(k) => self.field_type(&current, *k),
                Projection::Index(index) => {
                    self.integer_index(w, *index, "an index");
                    match &current {
                        ValueType::Array(element, _) => Some((**element).clone()),
                        _ => None,
                    }
                }
            };
            match reached {
                Some(next) => current = next,
                None => {
                    self.wrong(w, origin.clone(), format!("a projection of local {} reaches into a value of type {current}, which has no such part", place.local));
                    return None;
                }
            }
        }
        Some(current)
    }

    /// A slice is a view: it stands only as a call argument or a method receiver (`len` is one).
    pub(super) fn slice_positions(&mut self) {
        let ir = self.ir;
        let is_slice = |id: &ExprId| matches!(ir.exprs.get(id.index()).map(|e| &e.kind), Some(Expr::SliceOf { .. }));
        for (i, data) in ir.exprs.iter().enumerate() {
            let views_allowed = matches!(data.kind, Expr::Call { .. } | Expr::CallValue { .. } | Expr::Method { .. } | Expr::Length(_));
            if !views_allowed && data.kind.operands().iter().any(is_slice) {
                self.report(format!("expr {i}"), data.origin.clone(), "a slice stands only as a call argument or a method receiver".to_string());
            }
        }
        for (i, stmt) in ir.stmts.iter().enumerate() {
            if stmt.operands().iter().any(is_slice) {
                let first = stmt.operands().into_iter().find(is_slice).map_or_else(Checker::unknown, |e| self.expr_origin(e));
                self.report(format!("stmt {i}"), first, "a slice stands only as a call argument or a method receiver".to_string());
            }
        }
    }
}

