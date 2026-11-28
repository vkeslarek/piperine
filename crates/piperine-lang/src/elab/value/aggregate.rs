//! Structs and tuples as IR values (§1.2.4, §1.3.1 to §1.3.4, §1.3.11, §1.6.5; feature
//! `composite-values`, CV-09, CV-10): a struct expression, a tuple-struct call, a unit struct, a
//! tuple expression, a field or tuple-index read, an assignment through field projections, and the
//! tests of tuple, tuple-struct and struct patterns. An enum variant with fields is built and
//! matched the same way (`VariantOf`, `IsVariant`, `VariantField`; CV-11), and an `Option` held as
//! one value (a field) is the pair of value code packed (`OptionPack`) and unpacked (`OptionPart`).
//!
//! A struct or tuple is one IR value (`StructOf`, `Tuple`) whose fields are the parts in declaration
//! order, so assigning, passing or returning it copies it (§1.4.6). The fields of a struct
//! expression are evaluated in the order they are written (§1.3.11): when that is not declaration
//! order and a field is more than a read, each field is bound to a local first, in written order.

use super::ValueLowering;
use piperine_ir::{Expr, ExprId as LoweredExpr, LevelPart, Local, Origin, Place, Projection, Stmt, TypeId, ValueType};

use super::compound::EnumLayout;
use crate::elab::digital::elements::Lookup;
use crate::elab::elaborator::Scope;
use super::patterns::{PatternTest, Subject};
use crate::refusal::forms::ValueForm;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, FieldPatNode, PatId, PatKind, Resolution};
use crate::types::{Nominal, Ty};

/// What a struct expression or a call builds: a struct of the arena, or one variant of an enum.
#[derive(Clone, Copy)]
enum Built {
    /// A struct.
    Struct(TypeId),
    /// The variant at this position of an enum.
    Variant(TypeId, u32),
}

impl Built {
    /// The expression building the value from its parts.
    fn expr(self, parts: Vec<LoweredExpr>) -> Expr {
        match self {
            Built::Struct(ty) => Expr::StructOf { ty, fields: parts },
            Built::Variant(ty, variant) => Expr::VariantOf { ty, variant, fields: parts },
        }
    }
}

impl ValueLowering<'_, '_, '_> {
    /// The struct or variant declaration `callee` names, when it builds one.
    pub(super) fn constructed_decl(&self, callee: ExprId) -> Option<DeclId> {
        match &self.state.design.expr_node(callee).kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) if matches!(self.state.design.decl_data(d).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) => Some(d),
                _ => None,
            },
            ExprKind::Paren(inner) => self.constructed_decl(*inner),
            _ => None,
        }
    }

    /// What building the struct or variant `owner` as a value of `ty` makes, and the types of its
    /// fields.
    fn built_by(&self, owner: DeclId, ty: &ValueType) -> Option<(Built, Vec<ValueType>)> {
        match (&self.state.design.decl_data(owner).kind, ty) {
            (DeclKind::Struct(_), ValueType::Struct(id)) => Some((Built::Struct(*id), self.state.values.type_arena.field_types(*id).to_vec())),
            (DeclKind::Variant(_), ValueType::Enum(id)) => {
                let position = EnumLayout::variant_tag(self.state.design, owner)? as u32;
                Some((Built::Variant(*id, position), self.state.values.type_arena.variant_field_types(*id, position).to_vec()))
            }
            _ => None,
        }
    }

    /// `Name { field: value, .. }` of a struct or an enum variant: its fields in declaration order,
    /// evaluated in the order written (§1.3.11). `None` after a diagnostic.
    pub(super) fn lower_struct_expr(&mut self, scope: &mut Scope, expr: ExprId, path: crate::pom::PathId, fields: &[FieldInit], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let owner = match self.typed(expr) {
            Some(Ty::Struct(Nominal { decl, .. })) => decl,
            _ => match self.state.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(variant)) if matches!(self.state.design.decl_data(variant).kind, DeclKind::Variant(_)) => variant,
                _ => {
                    self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, ValueForm::StructExpressionThatNamesNeitherStructNor);
                    return None;
                }
            },
        };
        let (built, wanted) = self.built_by(owner, &ty)?;
        let layout: Vec<DeclId> = self.state.types.fields(owner)?.iter().map(|f| f.decl).collect();
        let mut written = Vec::with_capacity(fields.len());
        for init in fields {
            let index = match init.res {
                Resolution::Decl(field) => layout.iter().position(|f| *f == field),
                _ => None,
            };
            written.push((index?, init.value));
        }
        self.assemble(scope, &written, &wanted, &ty, origin, |parts| built.expr(parts))
    }

    /// `Name(a, b)`: a tuple struct or a variant with positional fields built by a call, its arguments
    /// the fields in order.
    pub(super) fn lower_constructor_call(&mut self, scope: &mut Scope, args: &[ExprId], owner: DeclId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let (built, wanted) = self.built_by(owner, &ty)?;
        let written: Vec<(usize, ExprId)> = args.iter().copied().enumerate().collect();
        self.assemble(scope, &written, &wanted, &ty, origin, |parts| built.expr(parts))
    }

    /// A unit struct named as a value: the one value of its type.
    pub(super) fn lower_unit_struct(&mut self, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let ValueType::Struct(id) = &ty else { return None };
        let id = *id;
        Some(self.emit(Expr::StructOf { ty: id, fields: Vec::new() }, ty, origin))
    }

    /// `(a, b, ..)`: a tuple of its operands, lowest field first.
    pub(super) fn lower_tuple(&mut self, scope: &mut Scope, items: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let written: Vec<(usize, ExprId)> = items.iter().copied().enumerate().collect();
        let wanted = self.part_types(&ty);
        self.assemble(scope, &written, &wanted, &ty, origin, Expr::Tuple)
    }

    /// The value `make` builds from the `count` parts of `written`, each `(declared index, expression)`
    /// listed in the order they are written and lowered in it. In declaration order, or when no
    /// part is more than a read, the parts are written where they are used; otherwise each is bound
    /// to a local in written order first, so the order they run in is the order they are written in.
    fn assemble(&mut self, scope: &mut Scope, written: &[(usize, ExprId)], wanted: &[ValueType], ty: &ValueType, origin: Origin, make: impl FnOnce(Vec<LoweredExpr>) -> Expr) -> Option<LoweredExpr> {
        let count = wanted.len();
        let mut parts: Vec<Option<LoweredExpr>> = vec![None; count];
        let mut ok = true;
        for (index, value) in written {
            match self.lower_part(scope, *value, wanted.get(*index)) {
                Some(lowered) => parts[*index] = Some(lowered),
                None => ok = false,
            }
        }
        if !ok {
            return None;
        }
        let in_order = written.iter().enumerate().all(|(position, (index, _))| position == *index);
        if in_order || written.iter().all(|(_, value)| self.simple_read(*value)) {
            let parts: Vec<LoweredExpr> = parts.into_iter().collect::<Option<_>>()?;
            return Some(self.emit(make(parts), ty.clone(), origin));
        }
        let mut stmts = Vec::with_capacity(count);
        let mut reads: Vec<Option<LoweredExpr>> = vec![None; count];
        for (index, _) in written {
            let init = parts[*index]?;
            let part_ty = self.type_of(init);
            let local = scope.values.locals.len() as u32;
            scope.values.locals.push(Local { name: format!("field${local}"), ty: part_ty.clone(), mutable: false });
            stmts.push(self.stmt(Stmt::Let { local, init }));
            reads[*index] = Some(self.emit(Expr::Local(local), part_ty, origin.clone()));
        }
        let reads: Vec<LoweredExpr> = reads.into_iter().collect::<Option<_>>()?;
        let built = self.emit(make(reads), ty.clone(), origin.clone());
        Some(self.emit(Expr::Block { stmts, value: Some(built) }, ty.clone(), origin))
    }

    /// The type of each part of the struct or tuple type `ty`, in order.
    fn part_types(&self, ty: &ValueType) -> Vec<ValueType> {
        match ty {
            ValueType::Tuple(parts) => parts.clone(),
            ValueType::Struct(id) => self.state.values.type_arena.field_types(*id).to_vec(),
            _ => Vec::new(),
        }
    }

    /// `value` lowered as a value of the type `wanted` held as one value (a carrier, a param, a
    /// record field): an `Option` is the pair of value code packed (D8), anything else is the
    /// expression itself.
    pub(in crate::elab) fn lower_stored(&mut self, scope: &mut Scope, value: ExprId, wanted: &ValueType) -> Option<LoweredExpr> {
        if let ValueType::Option(_) = wanted {
            let pair = self.lower_option(scope, value)?;
            let origin = self.expr_origin(value);
            return Some(self.emit(Expr::OptionPack { is: pair.is, value: pair.val }, wanted.clone(), origin));
        }
        self.lower_expr(scope, value)
    }

    /// One part lowered.
    fn lower_part(&mut self, scope: &mut Scope, value: ExprId, wanted: Option<&ValueType>) -> Option<LoweredExpr> {
        // An `Option` held as one value is the pair of value code packed (D8).
        if let Some(option @ ValueType::Option(_)) = wanted {
            return self.lower_stored(scope, value, option);
        }
        self.lower_expr(scope, value)
    }

    /// `recv.name`, the field of a struct or the component of a tuple, as a value of type `ty`.
    pub(super) fn lower_field_value(&mut self, scope: &mut Scope, recv: ExprId, name: &str, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let index = self.field_index(&self.typed(recv)?, name)?;
        let base = self.lower_expr(scope, recv)?;
        Some(self.emit(Expr::Field { expr: base, index }, ty, origin))
    }

    /// The position of the field `name` of the struct or tuple type `ty`: a name for a named field, the
    /// digits for a positional one.
    pub(in crate::elab) fn field_index(&self, ty: &Ty, name: &str) -> Option<u32> {
        match ty {
            Ty::Tuple(_) => name.parse().ok(),
            Ty::Struct(Nominal { decl, .. }) => {
                let fields = self.state.types.fields(*decl)?;
                match name.parse::<u32>() {
                    Ok(position) if fields.iter().all(|f| f.name.is_none()) => Some(position),
                    _ => fields.iter().position(|f| f.name.as_deref() == Some(name)).map(|p| p as u32),
                }
            }
            _ => None,
        }
    }

    /// The place an assignment's target names: a local and the fields and elements below it. `None`
    /// after a diagnostic.
    pub(super) fn place_of(&mut self, scope: &mut Scope, target: ExprId) -> Option<Place> {
        let peeled = self.peel_parens(target);
        let node = self.state.design.expr_node(peeled).clone();
        match &node.kind {
            ExprKind::Field { expr, name, .. } => {
                let index = self.field_index(&self.typed(*expr)?, &name.text);
                let Some(index) = index else {
                    self.refuse(scope.values.file, node.span, ValueForm::AssignmentToFieldOfTypeThatHas);
                    return None;
                };
                let mut place = self.place_of(scope, *expr)?;
                place.path.push(Projection::Field(index));
                Some(place)
            }
            ExprKind::Index { expr, index } => {
                if matches!(self.state.design.expr_node(*index).kind, ExprKind::Range { .. }) {
                    self.refuse(scope.values.file, node.span, ValueForm::AssignmentToRangeIndex);
                    return None;
                }
                let mut place = self.place_of(scope, *expr)?;
                let index = self.lower_expr(scope, *index)?;
                place.path.push(Projection::Index(index));
                Some(place)
            }
            _ => self.assigned_local(scope, peeled).map(|local| Place { local, path: Vec::new() }),
        }
    }

    /// The type of the place `place` of the local it starts at: the local's type, then through each
    /// field and element. `None` when a step does not apply.
    pub(super) fn place_type(&self, scope: &Scope, place: &Place) -> Option<ValueType> {
        let mut ty = scope.values.locals.get(place.local as usize)?.ty.clone();
        for step in &place.path {
            ty = match (ty, step) {
                (ValueType::Array(element, _), Projection::Index(_)) => *element,
                (ValueType::Tuple(parts), Projection::Field(index)) => parts.get(*index as usize)?.clone(),
                (ValueType::Struct(id), Projection::Field(index)) => self.state.values.type_arena.field_types(id).get(*index as usize)?.clone(),
                _ => return None,
            };
        }
        Some(ty)
    }

    /// `[a, b, ..]`: an array of its operands.
    pub(super) fn lower_array(&mut self, scope: &mut Scope, items: &[ExprId], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let mut parts = Vec::with_capacity(items.len());
        for item in items {
            parts.push(self.lower_expr(scope, *item)?);
        }
        let element = match parts.first() {
            Some(first) => self.type_of(*first),
            None => match &ty {
                ValueType::Array(element, _) => (**element).clone(),
                _ => return None,
            },
        };
        Some(self.emit(Expr::Array(parts), ValueType::Array(Box::new(element), items.len() as u64), origin))
    }

    /// `[x; N]`: `N` copies of `x`, which is evaluated once; `N` is a constant.
    pub(super) fn lower_array_repeat(&mut self, scope: &mut Scope, element: ExprId, length: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let count = self.const_count(scope, length)?;
        let value = self.lower_expr(scope, element)?;
        let element_type = self.type_of(value);
        Some(self.emit(Expr::ArrayRepeat { element: value, count }, ValueType::Array(Box::new(element_type), count), origin))
    }

    /// The value of the constant `expr` as a count. `None` after a diagnostic.
    fn const_count(&mut self, scope: &Scope, expr: ExprId) -> Option<u64> {
        match self.state.consts.integer(expr).and_then(|count| u64::try_from(count).ok()) {
            Some(count) => Some(count),
            None => {
                self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, ValueForm::LengthOrBoundThatIsNotInteger);
                None
            }
        }
    }

    /// `base[index]`: the element, or, for a range index with constant bounds, the array of the
    /// elements in range.
    pub(super) fn lower_index(&mut self, scope: &mut Scope, expr: ExprId, base: ExprId, index: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
        // one element of an array of carriers is read through the element, never through the array (`elements.rs`)
        match self.digital().element_lookup(scope, expr) {
            Lookup::Found(element) => return Some(self.digital().carrier_element_read(scope, element, origin)),
            Lookup::Failed => return None,
            Lookup::No => {}
        }
        let whole = self.lower_expr(scope, base)?;
        let ValueType::Array(element, length) = self.type_of(whole) else {
            self.refuse(scope.values.file, span, ValueForm::IndexOfValueThatIsNotArray);
            return None;
        };
        if let ExprKind::Range { lo, hi, inclusive } = self.state.design.expr_node(index).kind.clone() {
            return self.lower_range_index(scope, span, whole, (*element).clone(), length, (lo, hi, inclusive), origin);
        }
        let at = self.lower_expr(scope, index)?;
        Some(self.emit(Expr::Index { expr: whole, index: at }, *element, origin))
    }

    /// `base[lo..hi]` as `[base[lo], .., base[hi - 1]]`: bounds are constants, a missing one is the
    /// array's own end, and a range outside the array is a panic at elaboration (§1 rule 11).
    fn lower_range_index(&mut self, scope: &Scope, span: crate::msl::Span, whole: LoweredExpr, element: ValueType, length: u64, range: (Option<ExprId>, Option<ExprId>, bool), origin: Origin) -> Option<LoweredExpr> {
        let (lo, hi, inclusive) = range;
        let low = match lo {
            Some(lo) => self.const_count(scope, lo)?,
            None => 0,
        };
        let high = match hi {
            Some(hi) => self.const_count(scope, hi)? + u64::from(inclusive),
            None => length,
        };
        if low > high || high > length {
            let message = format!("the range index {low}..{high} is outside an array of {length} elements");
            let file = scope.values.file;
            self.report(file, crate::msl::Diagnostic::new("E0104", message, span, "range index out of bounds"));
            return None;
        }
        let parts: Vec<LoweredExpr> = (low..high)
            .map(|k| {
                let at = self.emit(Expr::Int(i128::from(k)), ValueType::UInt(64), origin.clone());
                self.emit(Expr::Index { expr: whole, index: at }, element.clone(), origin.clone())
            })
            .collect();
        Some(self.emit(Expr::Array(parts), ValueType::Array(Box::new(element), high - low), origin))
    }

    /// The tests of a tuple, tuple-struct or struct pattern over `subject` of the aggregate type
    /// `base`: each sub-pattern over the field it names, the tests joined and the names kept.
    pub(super) fn test_aggregate_pattern(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId, base: &ValueType, origin: Origin) -> Option<PatternTest> {
        let node = self.state.design.pat_node(pat).clone();
        let count = self.part_types(base).len();
        let fields: Vec<(u32, Option<PatId>, Option<DeclId>)> = match &node.kind {
            PatKind::Tuple(pats) | PatKind::TupleStruct { pats, .. } => self.positional(pats, count),
            PatKind::Struct { fields, .. } => self.named_fields(fields)?,
            _ => return None,
        };
        let mut test: Option<LoweredExpr> = None;
        let mut bindings = Vec::new();
        for (index, inner, binding) in fields {
            let whole = self.read_subject(scope, subject, origin.clone())?;
            let part_ty = self.part_types(base).get(index as usize)?.clone();
            let field = self.emit(Expr::Field { expr: whole, index }, part_ty.clone(), origin.clone());
            if let Some(binding) = binding {
                bindings.push((binding, field));
            }
            if let Some(inner) = inner {
                let field_subject = self.field_subject(field, &part_ty, &origin);
                let found = self.test_pattern(scope, &field_subject, inner)?;
                bindings.extend(found.bindings);
                if let Some(inner_test) = found.test {
                    test = Some(match test {
                        Some(earlier) => self.both(earlier, inner_test, origin.clone()),
                        None => inner_test,
                    });
                }
            }
        }
        Some(PatternTest { test, bindings })
    }

    /// The subject of a pattern over the field `field` of type `ty` of an aggregate: an `Option` field (held packed) is the pair of
    /// its flag and its payload, any other field the value itself.
    fn field_subject(&mut self, field: LoweredExpr, ty: &ValueType, origin: &Origin) -> Subject {
        let ValueType::Option(payload) = ty else { return Subject::Value(field) };
        let is = self.emit(Expr::OptionPart { option: field, part: LevelPart::Is }, ValueType::Bool, origin.clone());
        let val = self.emit(Expr::OptionPart { option: field, part: LevelPart::Value }, (**payload).clone(), origin.clone());
        Subject::Shared { is, val }
    }

    /// `Variant(p, ..)` and `Variant { f: p, .. }` over an enum value: the value is that variant and each
    /// sub-pattern holds of the variant's field it names (§1.6.5). The fields are read only where
    /// the variant test holds, as the `&&` that joins them runs them. `None` after a diagnostic.
    pub(super) fn test_variant_pattern(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId, base: &ValueType, origin: Origin) -> Option<PatternTest> {
        let node = self.state.design.pat_node(pat).clone();
        let ValueType::Enum(id) = base else { return None };
        let path = match &node.kind {
            PatKind::TupleStruct { path, .. } | PatKind::Struct { path, .. } => *path,
            _ => return None,
        };
        let variant = match self.state.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) if matches!(self.state.design.decl_data(d).kind, DeclKind::Variant(_)) => d,
            _ => {
                self.refuse(scope.values.file, node.span, ValueForm::PatternWithFieldsThatNamesNoVariant);
                return None;
            }
        };
        let position = EnumLayout::variant_tag(self.state.design, variant)? as u32;
        let whole = self.read_subject(scope, subject, origin.clone())?;
        let part_types = self.state.values.type_arena.variant_field_types(*id, position).to_vec();
        let fields = match &node.kind {
            PatKind::TupleStruct { pats, .. } => self.positional(pats, part_types.len()),
            PatKind::Struct { fields, .. } => self.named_fields(fields)?,
            _ => return None,
        };
        let mut test = self.emit(Expr::IsVariant { expr: whole, variant: position }, ValueType::Bool, origin.clone());
        let mut bindings = Vec::new();
        for (index, inner, binding) in fields {
            let again = self.read_subject(scope, subject, origin.clone())?;
            let field = self.emit(Expr::VariantField { expr: again, variant: position, field: index }, part_types.get(index as usize)?.clone(), origin.clone());
            if let Some(binding) = binding {
                bindings.push((binding, field));
            }
            if let Some(inner) = inner {
                let found = self.test_pattern(scope, &Subject::Value(field), inner)?;
                bindings.extend(found.bindings);
                if let Some(inner_test) = found.test {
                    test = self.both(test, inner_test, origin.clone());
                }
            }
        }
        Some(PatternTest { test: Some(test), bindings })
    }

    /// `[a, b]`, `[first, .., last]` and `[head, rest @ ..]` over an array (§1.6.7): each sub-pattern
    /// over the element it names, `..` standing for the elements between and `rest @ ..` binding them
    /// as an array.
    pub(super) fn test_array_pattern(&mut self, scope: &mut Scope, subject: &Subject, pats: &[PatId], base: &ValueType, origin: Origin) -> Option<PatternTest> {
        let ValueType::Array(element, length) = base else { return None };
        let arity = usize::try_from(*length).ok()?;
        let rest_at = pats.iter().position(|p| self.is_rest(*p));
        let after = rest_at.map_or(0, |at| pats.len() - at - 1);
        let mut test: Option<LoweredExpr> = None;
        let mut bindings = Vec::new();
        for (position, pat) in pats.iter().enumerate() {
            if Some(position) == rest_at {
                let (from, to) = (position, arity - after);
                if let PatKind::Ident { binding, .. } = self.state.design.pat_node(*pat).kind {
                    let whole = self.read_subject(scope, subject, origin.clone())?;
                    let parts: Vec<LoweredExpr> = (from..to).map(|k| self.element_read(whole, k as u64, element, &origin)).collect();
                    let rest = self.emit(Expr::Array(parts), ValueType::Array(element.clone(), (to - from) as u64), origin.clone());
                    bindings.push((binding, rest));
                }
                continue;
            }
            let index = match rest_at {
                Some(rest) if position > rest => arity - (pats.len() - position),
                _ => position,
            };
            let whole = self.read_subject(scope, subject, origin.clone())?;
            let field = self.element_read(whole, index as u64, element, &origin);
            let found = self.test_pattern(scope, &Subject::Value(field), *pat)?;
            bindings.extend(found.bindings);
            if let Some(inner) = found.test {
                test = Some(match test {
                    Some(earlier) => self.both(earlier, inner, origin.clone()),
                    None => inner,
                });
            }
        }
        Some(PatternTest { test, bindings })
    }

    /// Element `index` of the array value `whole`.
    fn element_read(&mut self, whole: LoweredExpr, index: u64, element: &ValueType, origin: &Origin) -> LoweredExpr {
        let at = self.emit(Expr::Int(i128::from(index)), ValueType::UInt(64), origin.clone());
        self.emit(Expr::Index { expr: whole, index: at }, element.clone(), origin.clone())
    }

    /// Whether `pat` is `..` or `name @ ..`.
    fn is_rest(&self, pat: PatId) -> bool {
        match &self.state.design.pat_node(pat).kind {
            PatKind::Rest => true,
            PatKind::Ident { sub: Some(sub), .. } => matches!(self.state.design.pat_node(*sub).kind, PatKind::Rest),
            _ => false,
        }
    }

    /// `let pat = init;` of a tuple, tuple-struct or struct pattern: the value is bound once, each
    /// name takes its field (§1.7). `false` after a diagnostic.
    pub(super) fn lower_destructuring_let(&mut self, scope: &mut Scope, out: &mut Vec<piperine_ir::StmtId>, pat: PatId, init: ExprId) -> bool {
        let Some((subject, prelude)) = self.subject_of(scope, init, false) else { return false };
        let Some(matched) = self.test_pattern(scope, &subject, pat) else { return false };
        let Some(names) = self.bind_names(scope, &matched.bindings) else { return false };
        out.extend(prelude);
        out.extend(self.binding_lets(&names));
        true
    }

    /// The `(field index, sub-pattern, binding)` of the sub-patterns of a positional pattern over
    /// `count` fields: `..` stands for the fields between the ones before and after it (§1.6.5).
    fn positional(&self, pats: &[PatId], count: usize) -> Vec<(u32, Option<PatId>, Option<DeclId>)> {
        let rest = pats.iter().position(|p| matches!(self.state.design.pat_node(*p).kind, PatKind::Rest));
        let mut out = Vec::new();
        for (position, pat) in pats.iter().enumerate() {
            let index = match rest {
                Some(rest) if position > rest => count - (pats.len() - position),
                _ => position,
            };
            if !matches!(self.state.design.pat_node(*pat).kind, PatKind::Rest) {
                out.push((index as u32, Some(*pat), None));
            }
        }
        out
    }

    /// The `(field index, sub-pattern, binding)` of the fields a struct pattern names: `x` alone
    /// binds the field `x`, `x: p` tests it with `p`.
    fn named_fields(&self, fields: &[FieldPatNode]) -> Option<Vec<(u32, Option<PatId>, Option<DeclId>)>> {
        let mut out = Vec::new();
        for field in fields {
            let Resolution::Decl(decl) = field.res else { return None };
            let owner = self.state.design.decl_data(decl).parent?;
            let index = self.state.types.fields(owner)?.iter().position(|f| f.decl == decl)? as u32;
            out.push((index, field.pat, field.binding));
        }
        Some(out)
    }
}
