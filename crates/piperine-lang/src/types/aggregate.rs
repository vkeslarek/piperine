//! Struct, tuple and array types (§1.2.4, §1.2.5, §1.3).
//!
//! A `struct` is a nominal value type ([`Ty::Struct`]): its fields are typed once, on first use,
//! into [`FieldType`]s kept in `Checker::layouts`. A type that contains itself, directly or
//! through structs, enums, tuples, arrays and `Option`, is `E0186` (§1.3.8).

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FileId, TypeId};

use super::check::Checker;
use super::Ty;

/// One typed field of a struct or of an enum variant, in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FieldType {
    /// The field's name; `None` for a positional field.
    pub(super) name: Option<String>,
    /// The field's declaration.
    pub(super) decl: DeclId,
    /// The field's type.
    pub(super) ty: Ty,
}

impl Checker<'_> {
    // ---- types as written ------------------------------------------------------------------

    /// The type of the struct `decl` named at `span` of `file`: [`Ty::Struct`] once its fields are
    /// typed, else a diagnostic and `None` (a generic struct or a bundle is outside the subset, a
    /// duplicate field or a self-containing type is `E0186`).
    pub(super) fn struct_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if data.generics_span.is_some() {
            self.unsupported(file, span, &format!("generic struct `{name}`"));
            return None;
        }
        if self.broken.contains(&decl) {
            return None;
        }
        if self.layouts.contains_key(&decl) {
            return Some(Ty::Struct(decl));
        }
        if self.declaring.contains(&decl) {
            return self.cyclic_type(decl, file, span);
        }
        self.declaring.insert(decl);
        let declared = self.declare_struct(decl, file, span);
        self.declaring.remove(&decl);
        if declared {
            Some(Ty::Struct(decl))
        } else {
            self.broken.insert(decl);
            None
        }
    }

    /// A type met again while its own fields are being typed: `E0186` (§1.3.8).
    pub(super) fn cyclic_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let kind = &self.design.decl_data(decl).kind;
        let ty = if matches!(kind, DeclKind::Enum(_)) { Ty::Enum(decl) } else { Ty::Struct(decl) };
        let name = ty.name(self.design);
        self.coded(file, "E0186", span, format!("{name} contains itself: a value has a fixed size and there is no indirection (§1.3.8)"), "contains itself");
        None
    }

    /// Types the fields of the struct `decl`: a struct with a carrier field is a bundle, not a
    /// value type (§1.3), and a duplicate field name is `E0186` (§1.3.2).
    fn declare_struct(&mut self, decl: DeclId, file: FileId, span: Span) -> bool {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or("", |n| n.text.as_str());
        let carries = data.children.iter().any(|c| matches!(&self.design.decl_data(*c).kind, DeclKind::Field(f) if f.dir.is_some()));
        if matches!(&data.kind, DeclKind::Struct(s) if s.bundle == Some(true)) || carries {
            self.unsupported(file, span, &format!("struct `{name}` with a carrier field (a bundle) as a value type"));
            return false;
        }
        self.fields_of(decl).is_some()
    }

    /// The typed fields of `owner`, a struct or an enum variant, typed on the first call and kept.
    pub(super) fn fields_of(&mut self, owner: DeclId) -> Option<Vec<FieldType>> {
        if let Some(found) = self.layouts.get(&owner) {
            return Some(found.clone());
        }
        let mut fields: Vec<FieldType> = Vec::new();
        let mut ok = true;
        for child in self.design.decl_data(owner).children.clone() {
            let field = self.design.decl_data(child).clone();
            let DeclKind::Field(data) = &field.kind else { continue };
            let name = field.name.as_ref().map(|n| n.text.clone());
            if let Some(name) = &name
                && fields.iter().any(|f| f.name.as_deref() == Some(name))
            {
                self.coded(field.file, "E0186", field.span, format!("the field `{name}` is declared twice (§1.3.2)"), "declared twice");
                ok = false;
                continue;
            }
            match self.lower_type(data.ty) {
                Some(ty) if self.is_field_type(&ty) => fields.push(FieldType { name, decl: child, ty }),
                Some(ty) => {
                    let what = ty.name(self.design);
                    self.unsupported(field.file, field.span, &format!("a field of type `{what}` in a value type"));
                    ok = false;
                }
                None => ok = false,
            }
        }
        ok.then(|| {
            self.layouts.insert(owner, fields.clone());
            fields
        })
    }

    /// Whether a value type may be held by a field: what a value is (§1.2).
    fn is_field_type(&self, ty: &Ty) -> bool {
        ty.is_value()
    }

    /// `(A, B)` and `(A,)` (§1.2.4).
    pub(super) fn tuple_type(&mut self, items: &[TypeId]) -> Option<Ty> {
        let types: Option<Vec<Ty>> = items.iter().map(|t| self.lower_type(*t)).collect();
        let types = types?;
        for (ty, node) in types.iter().zip(items) {
            if !ty.is_value() {
                let (file, span) = self.type_place(*node);
                self.unsupported(file, span, &format!("a `{}` inside a tuple type", ty.name(self.design)));
                return None;
            }
        }
        Some(Ty::Tuple(types))
    }

    /// `[T; N]` (§1.2.5): `N` is a const expression the checker folds, else `E0151`.
    pub(super) fn array_type(&mut self, file: FileId, span: Span, elem: TypeId, len: ExprId) -> Option<Ty> {
        let element = self.lower_type(elem);
        let Some(length) = self.fold_length(len) else {
            let what = self.expression_name(len);
            self.coded(file, "E0151", span, format!("the length of an array type is a constant, and {what} is not one the checker folds (§1.2.5)"), "not a constant");
            return None;
        };
        let element = element?;
        if !element.is_value() {
            self.unsupported(file, span, &format!("an array of `{}`", element.name(self.design)));
            return None;
        }
        Some(Ty::Array(Box::new(element), length))
    }

    /// The file and span of a type node.
    fn type_place(&self, ty: TypeId) -> (FileId, Span) {
        let node = self.design.type_node(ty);
        (self.design.decl_data(node.owner).file, node.span)
    }

    /// The name of a const expression for a message: a path as written, else a plain word.
    pub(super) fn expression_name(&self, expr: ExprId) -> String {
        match &self.design.expr_node(expr).kind {
            ExprKind::Path(path) => {
                let segments: Vec<&str> = self.design.path_node(*path).segments.iter().map(|s| s.name.text.as_str()).collect();
                format!("`{}`", segments.join("::"))
            }
            ExprKind::Paren(inner) => self.expression_name(*inner),
            _ => "this expression".to_string(),
        }
    }
}
