//! The type arena's invariants (composite-values T30, CV-20): every field type is well formed, no
//! type contains itself (§1.3.8), the shape of a struct agrees with its form, an enum's tag holds
//! its discriminants (§1.3.7, §2 rule 5) and its derives imply each other as Appendix H.8 says.

use std::collections::BTreeSet;

use super::traits::Trait;
use super::{Checker, Ctx, Task, Walk};
use crate::ids::ExprId;
use crate::model::{AggregateData, BinaryOp, Constant, Derives, Expr, FieldsForm, LevelPart, Place, Projection, TypeData, TypeKind, ValueType};
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
        for which in Trait::ALL {
            if which.derived(&data.derives) && !self.fields_implement(data, which) {
                self.report(item.to_string(), data.origin.clone(), format!("derive {} requires every field to implement it (H.8 rule 6)", which.name()));
            }
        }
        if default && matches!(data.kind, TypeKind::Enum { .. }) {
            self.report(item.to_string(), data.origin.clone(), "an enum does not derive Default (H.8 rule 4)".to_string());
        }
    }

    /// Whether every field of `data` implements `which` on its own (H.8 rule 6); `Repr` is checked by the elaborator.
    fn fields_implement(&self, data: &TypeData, which: Trait) -> bool {
        let fields: Vec<&ValueType> = match &data.kind {
            TypeKind::Struct { fields, .. } => fields.iter().map(|(_, ty)| ty).collect(),
            TypeKind::Enum { variants, .. } => variants.iter().flat_map(|v| v.fields.iter().map(|(_, ty)| ty)).collect(),
        };
        fields.into_iter().all(|ty| self.implements(ty, which))
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
                Expr::IsVariant { expr, variant } => {
                    self.own(w, &origin, ty, &ValueType::Bool);
                    self.variant_of(w, &origin, *expr, *variant);
                }
                Expr::VariantField { expr, variant, field } => {
                    if let Some(fields) = self.variant_of(w, &origin, *expr, *variant) {
                        match fields.get(*field as usize) {
                            Some(field_ty) => self.own(w, &origin, ty, field_ty),
                            None => self.wrong(w, origin.clone(), format!("field {field} of variant {variant}, which has {} fields", fields.len())),
                        }
                    }
                }
                Expr::OptionPack { is, value } => {
                    self.want(w, *is, &ValueType::Bool, "the `is` half of an option");
                    if let Some(inner) = self.ty(*value) {
                        self.own(w, &origin, ty, &ValueType::Option(Box::new(inner.clone())));
                    }
                }
                Expr::OptionPart { option, part } => match self.ty(*option) {
                    Some(ValueType::Option(inner)) => self.own(w, &origin, ty, if *part == LevelPart::Is { &ValueType::Bool } else { inner }),
                    Some(other) => self.wrong(w, origin.clone(), format!("an option part of a value of type {other}, which is not an Option")),
                    None => {}
                },
                Expr::Default(wanted) => {
                    self.own(w, &origin, ty, wanted);
                    if !self.implements(wanted, Trait::Default) {
                        self.wrong(w, origin.clone(), format!("`default` of type {wanted}, which does not implement Default (H.8 rule 4)"));
                    }
                }
                _ => {}
            }
        }
        next.extend(kind.operands().into_iter().map(|operand| Task::Expr(operand, ctx)));
    }

    /// The field types of variant `variant` of the enum value `expr`; names the mistake otherwise.
    fn variant_of(&mut self, w: &Walk, origin: &crate::model::Origin, expr: ExprId, variant: u32) -> Option<Vec<ValueType>> {
        let ValueType::Enum(id) = self.ty(expr)? else {
            self.wrong(w, origin.clone(), format!("a variant of a value of type {}, which is not an enum", self.ty(expr)?));
            return None;
        };
        let Some(TypeKind::Enum { variants, .. }) = self.ir.types.get(id.index()).map(|t| &t.kind) else { return None };
        match variants.get(variant as usize) {
            Some(v) => Some(v.fields.iter().map(|(_, ty)| ty.clone()).collect()),
            None => {
                self.wrong(w, origin.clone(), format!("variant {variant} out of range of enum t{}", id.0));
                None
            }
        }
    }

    /// Whether `ty` is an aggregate: a tuple, an array, a slice, a struct, an enum or an `Option`.
    pub(super) fn is_aggregate(ty: &ValueType) -> bool {
        matches!(ty, ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Slice(_) | ValueType::Struct(_) | ValueType::Enum(_) | ValueType::Option(_))
    }

    /// Whether `kind` is an operator with an aggregate operand.
    pub(super) fn operates_on_aggregates(&self, kind: &Expr) -> bool {
        matches!(kind, Expr::Binary { lhs, rhs, .. } if [lhs, rhs].iter().any(|e| self.ty(**e).is_some_and(Checker::is_aggregate)))
    }

    /// A comparison of aggregates (§1.3.10): `==` and `!=` need `PartialEq`, `<`, `<=`, `>` and `>=`
    /// need `PartialOrd`, both operands one type; the result is a `bool`. Anything else is refused.
    pub(super) fn aggregate_comparison(&mut self, w: &mut Walk, id: ExprId, first: bool, ctx: Ctx, next: &mut Vec<Task>) {
        let data = &self.ir.exprs[id.index()];
        let Expr::Binary { op, lhs, rhs } = &data.kind else { return };
        let origin = data.origin.clone();
        if first {
            self.own(w, &origin, &data.ty, &ValueType::Bool);
            if let (Some(l), Some(r)) = (self.ty(*lhs), self.ty(*rhs)) {
                let needed = match op {
                    BinaryOp::Eq | BinaryOp::Ne => Some(Trait::PartialEq),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => Some(Trait::PartialOrd),
                    _ => None,
                };
                match needed {
                    None => self.wrong(w, origin.clone(), format!("`{op:?}` does not apply to aggregates")),
                    Some(_) if l != r => self.wrong(w, origin.clone(), format!("compares {l} with {r}")),
                    Some(needed) if !self.implements(l, needed) => self.wrong(w, origin.clone(), format!("`{op:?}` on {l}, which does not implement {} (§1.3.10)", needed.name())),
                    Some(_) => {}
                }
            }
        }
        next.push(Task::Expr(*lhs, ctx));
        next.push(Task::Expr(*rhs, ctx));
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


impl Checker<'_> {
    /// Each composite constant agrees with its own type (§1.5.25), and none holds itself.
    pub(super) fn aggregate_constants(&mut self) {
        let ir = self.ir;
        for (index, data) in ir.aggregates.iter().enumerate() {
            let item = format!("aggregate agg{index}");
            let origin = Checker::unknown();
            if let Some(what) = self.malformed(&data.ty) {
                self.report(item.clone(), origin.clone(), what);
                continue;
            }
            if let Some(what) = self.aggregate_mismatch(data) {
                self.report(item.clone(), origin.clone(), what);
            }
            if self.constant_holds_itself(index) {
                self.report(item, origin, "a composite constant contains itself".to_string());
            }
        }
    }

    /// What is wrong with the parts of `data` against its type, `None` when they fit.
    fn aggregate_mismatch(&self, data: &AggregateData) -> Option<String> {
        let (wanted, variant): (Vec<ValueType>, Option<u32>) = match &data.ty {
            ValueType::Tuple(parts) => (parts.clone(), None),
            ValueType::Array(element, length) => (vec![(**element).clone(); *length as usize], None),
            ValueType::Struct(id) => (self.struct_fields(*id)?, None),
            ValueType::Enum(id) => {
                let Some(TypeKind::Enum { variants, .. }) = self.ir.types.get(id.index()).map(|t| &t.kind) else { return Some("an enum constant of a non-enum type".to_string()) };
                let chosen = data.variant?;
                match variants.get(chosen as usize) {
                    Some(v) => (v.fields.iter().map(|(_, ty)| ty.clone()).collect(), Some(chosen)),
                    None => return Some(format!("variant {chosen} out of range of enum t{}", id.0)),
                }
            }
            ValueType::Option(inner) if data.parts.len() <= 1 => (vec![(**inner).clone(); data.parts.len()], None),
            ValueType::Option(_) => return Some("an Option constant holds at most one part".to_string()),
            other => return Some(format!("a composite constant of type {other}, which is not an aggregate type")),
        };
        if matches!(data.ty, ValueType::Enum(_)) && variant.is_none() {
            return Some("an enum constant names its variant".to_string());
        }
        if !matches!(data.ty, ValueType::Enum(_)) && data.variant.is_some() {
            return Some("only an enum constant names a variant".to_string());
        }
        if wanted.len() != data.parts.len() {
            return Some(format!("the constant of type {} has {} parts, found {}", data.ty, wanted.len(), data.parts.len()));
        }
        wanted.iter().zip(&data.parts).enumerate().find(|(_, (ty, part))| !self.holds(ty, **part)).map(|(i, (ty, part))| format!("part {i} of a composite constant is {part}, not a value of {ty}"))
    }

    /// Whether the constant at `start` reaches itself through its parts.
    fn constant_holds_itself(&self, start: usize) -> bool {
        let mut visited: BTreeSet<usize> = BTreeSet::new();
        let mut stack: Vec<usize> = vec![start];
        while let Some(index) = stack.pop() {
            for part in &self.ir.aggregates[index].parts {
                if let Constant::Aggregate(id) = part {
                    if id.index() == start {
                        return true;
                    }
                    if id.index() < self.ir.aggregates.len() && visited.insert(id.index()) {
                        stack.push(id.index());
                    }
                }
            }
        }
        false
    }
}

impl Checker<'_> {
    /// A param default or an instance's param value that is a composite, or of a composite type, is a
    /// value of the param's type (scalar params keep the checks they always had).
    pub(super) fn composite_params(&mut self) {
        let ir = self.ir;
        let composite = |ty: &ValueType, value: &Constant| Checker::is_aggregate(ty) || matches!(value, Constant::Aggregate(_));
        for body in &ir.bodies {
            let item = format!("body {}", body.name);
            for param in &body.params {
                if let Some(default) = &param.default {
                    if composite(&param.ty, default) && !self.holds(&param.ty, *default) {
                        self.report(item.clone(), param.origin.clone(), format!("the default of param {} is {default}, not a value of {}", param.name, param.ty));
                    }
                }
            }
        }
        for instance in &ir.instances {
            let Some(body) = ir.bodies.get(instance.body.index()) else { continue };
            for (param, value) in body.params.iter().zip(&instance.params) {
                if composite(&param.ty, value) && !self.holds(&param.ty, *value) {
                    self.report(format!("instance {}", instance.path), instance.origin.clone(), format!("param {} is bound to {value}, not a value of {}", param.name, param.ty));
                }
            }
        }
    }
}
