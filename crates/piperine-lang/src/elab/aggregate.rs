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
//! A part that may be unknown is refused by name: a struct or a tuple is not a value a flag rides on.

use piperine_ir::{Expr, ExprId as LoweredExpr, Local, Origin, Place, Projection, Stmt, TypeId, ValueType};

use super::compound::EnumLayout;
use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use super::patterns::{PatternTest, Subject};
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, FieldPatNode, PatId, PatKind, Resolution};
use crate::types::Ty;

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

impl Elaboration<'_, '_> {
    /// The struct or variant declaration `callee` names, when it builds one.
    pub(super) fn constructed_decl(&self, callee: ExprId) -> Option<DeclId> {
        match &self.design.expr_node(callee).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) => Some(d),
                _ => None,
            },
            ExprKind::Paren(inner) => self.constructed_decl(*inner),
            _ => None,
        }
    }

    /// What building the struct or variant `owner` as a value of `ty` makes, and the types of its
    /// fields.
    fn built_by(&self, owner: DeclId, ty: &ValueType) -> Option<(Built, Vec<ValueType>)> {
        match (&self.design.decl_data(owner).kind, ty) {
            (DeclKind::Struct(_), ValueType::Struct(id)) => Some((Built::Struct(*id), self.type_arena.field_types(*id).to_vec())),
            (DeclKind::Variant(_), ValueType::Enum(id)) => {
                let position = EnumLayout::variant_tag(self.design, owner)? as u32;
                Some((Built::Variant(*id, position), self.type_arena.variant_field_types(*id, position).to_vec()))
            }
            _ => None,
        }
    }

    /// `Name { field: value, .. }` of a struct or an enum variant: its fields in declaration order,
    /// evaluated in the order written (§1.3.11). `None` after a diagnostic.
    pub(super) fn lower_struct_expr(&mut self, scope: &mut Scope, expr: ExprId, path: crate::pom::PathId, fields: &[FieldInit], ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let owner = match self.types.ty(expr) {
            Some(Ty::Struct(decl)) => *decl,
            _ => match self.design.path_node(path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(variant)) if matches!(self.design.decl_data(variant).kind, DeclKind::Variant(_)) => variant,
                _ => {
                    self.unsupported(scope.file, self.design.expr_node(expr).span, "a struct expression that names neither a struct nor a variant");
                    return None;
                }
            },
        };
        let (built, wanted) = self.built_by(owner, &ty)?;
        let layout: Vec<DeclId> = self.types.fields(owner)?.iter().map(|f| f.decl).collect();
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
            let local = scope.locals.len() as u32;
            scope.locals.push(Local { name: format!("field${local}"), ty: part_ty.clone(), mutable: false });
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
            ValueType::Struct(id) => self.type_arena.field_types(*id).to_vec(),
            _ => Vec::new(),
        }
    }

    /// One part lowered: a value that may be unknown cannot be a part, which is a plain value.
    fn lower_part(&mut self, scope: &mut Scope, value: ExprId, wanted: Option<&ValueType>) -> Option<LoweredExpr> {
        // An `Option` held as one value is the pair of value code packed (D8).
        if let Some(ValueType::Option(_)) = wanted {
            let pair = self.lower_option(scope, value)?;
            let origin = self.expr_origin(value);
            return Some(self.emit(Expr::OptionPack { is: pair.is, value: pair.val }, wanted?.clone(), origin));
        }
        let lowered = self.lower_expr(scope, value)?;
        if self.flag(lowered) == Flag::Flagged && wanted.is_none_or(|w| Flag::of(w) == Flag::Plain) {
            self.unsupported(scope.file, self.design.expr_node(value).span, "a field of a struct or a tuple whose value may be unknown");
            return None;
        }
        Some(lowered)
    }

    /// `recv.name`, the field of a struct or the component of a tuple, as a value of type `ty`.
    pub(super) fn lower_field_value(&mut self, scope: &mut Scope, recv: ExprId, name: &str, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let index = self.field_index(self.types.ty(recv)?, name)?;
        let base = self.lower_expr(scope, recv)?;
        Some(self.emit(Expr::Field { expr: base, index }, ty, origin))
    }

    /// The position of the field `name` of the struct or tuple type `ty`: a name for a named field, the
    /// digits for a positional one.
    pub(super) fn field_index(&self, ty: &Ty, name: &str) -> Option<u32> {
        match ty {
            Ty::Tuple(_) => name.parse().ok(),
            Ty::Struct(decl) => {
                let fields = self.types.fields(*decl)?;
                match name.parse::<u32>() {
                    Ok(position) if fields.iter().all(|f| f.name.is_none()) => Some(position),
                    _ => fields.iter().position(|f| f.name.as_deref() == Some(name)).map(|p| p as u32),
                }
            }
            _ => None,
        }
    }

    /// The place an assignment's target names: a local and the fields below it. `None` after a
    /// diagnostic.
    pub(super) fn place_of(&mut self, scope: &Scope, target: ExprId) -> Option<Place> {
        let peeled = self.peel_parens(target);
        let node = self.design.expr_node(peeled).clone();
        match &node.kind {
            ExprKind::Field { expr, name, .. } => {
                let index = self.field_index(self.types.ty(*expr)?, &name.text);
                let Some(index) = index else {
                    self.unsupported(scope.file, node.span, "an assignment to a field of a type that has none");
                    return None;
                };
                let mut place = self.place_of(scope, *expr)?;
                place.path.push(Projection::Field(index));
                Some(place)
            }
            ExprKind::Index { .. } => {
                self.unsupported(scope.file, node.span, "an assignment to an element");
                None
            }
            _ => self.assigned_local(scope, peeled).map(|local| Place { local, path: Vec::new() }),
        }
    }

    /// The tests of a tuple, tuple-struct or struct pattern over `subject` of the aggregate type
    /// `base`: each sub-pattern over the field it names, the tests joined and the names kept.
    pub(super) fn test_aggregate_pattern(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId, base: &ValueType, origin: Origin) -> Option<PatternTest> {
        let node = self.design.pat_node(pat).clone();
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
            let field = self.emit(Expr::Field { expr: whole, index }, part_ty, origin.clone());
            if let Some(binding) = binding {
                bindings.push((binding, field));
            }
            if let Some(inner) = inner {
                let found = self.test_pattern(scope, &Subject::Value(field), inner)?;
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

    /// `Variant(p, ..)` and `Variant { f: p, .. }` over an enum value: the value is that variant and each
    /// sub-pattern holds of the variant's field it names (§1.6.5). The fields are read only where
    /// the variant test holds, as the `&&` that joins them runs them. `None` after a diagnostic.
    pub(super) fn test_variant_pattern(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId, base: &ValueType, origin: Origin) -> Option<PatternTest> {
        let node = self.design.pat_node(pat).clone();
        let ValueType::Enum(id) = base else { return None };
        let path = match &node.kind {
            PatKind::TupleStruct { path, .. } | PatKind::Struct { path, .. } => *path,
            _ => return None,
        };
        let variant = match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Variant(_)) => d,
            _ => {
                self.unsupported(scope.file, node.span, "a pattern with fields that names no variant");
                return None;
            }
        };
        let position = EnumLayout::variant_tag(self.design, variant)? as u32;
        let whole = self.read_subject(scope, subject, origin.clone())?;
        if matches!(self.type_of(whole), ValueType::Flagged(_)) {
            self.unsupported(scope.file, node.span, "a variant with fields of a value that may be unknown");
            return None;
        }
        let part_types = self.type_arena.variant_field_types(*id, position).to_vec();
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
        let rest = pats.iter().position(|p| matches!(self.design.pat_node(*p).kind, PatKind::Rest));
        let mut out = Vec::new();
        for (position, pat) in pats.iter().enumerate() {
            let index = match rest {
                Some(rest) if position > rest => count - (pats.len() - position),
                _ => position,
            };
            if !matches!(self.design.pat_node(*pat).kind, PatKind::Rest) {
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
            let owner = self.design.decl_data(decl).parent?;
            let index = self.types.fields(owner)?.iter().position(|f| f.decl == decl)? as u32;
            out.push((index, field.pat, field.binding));
        }
        Some(out)
    }
}
