//! Value items: functions and the generic parameters every item kind shares.

use super::Lowerer;
use crate::msl::ast;
use crate::pom::decl::{DeclKind, EnumData, FieldData, FieldsForm, FnData, FnParamData, GenericParamData, StructData, VariantData};

impl Lowerer<'_> {
    /// One `GenericParam` declaration per parameter, as children of the
    /// current owner, in source order. Shared by every item that has
    /// generics.
    pub(crate) fn generics(&mut self, generics: &Option<ast::Generics>) {
        let Some(g) = generics else { return };
        for p in &g.params {
            let id = self.new_decl(DeclKind::Error, Some(&p.name), p.span.0, false);
            let data = self.within(id, |lw| match &p.kind {
                ast::GenericParamKind::Type { bounds, default } => GenericParamData::Type {
                    bounds: bounds.iter().map(|b| lw.path(b)).collect(),
                    default: default.as_ref().map(|t| lw.ty(t)),
                },
                ast::GenericParamKind::Const { ty, default } => GenericParamData::Const {
                    ty: lw.ty(ty),
                    default: default.as_ref().map(|e| lw.expr(e)),
                },
            });
            self.design.decl_mut(id).kind = DeclKind::GenericParam(data);
        }
    }

    /// A function: generics, then parameters, then the body, all children of
    /// the function declaration (the current owner).
    pub(crate) fn fn_decl(&mut self, f: &ast::FnDecl) -> DeclKind {
        self.generics(&f.generics);
        for p in &f.params {
            self.fn_param(p);
        }
        let ret = f.ret.as_ref().map(|t| self.ty(t));
        let where_ = f.where_.as_ref().map(|e| self.expr(e));
        let body = f.body.as_ref().map(|b| self.block(b));
        DeclKind::Fn(FnData { is_const: f.is_const, abi: f.abi.clone(), ret, where_, body })
    }

    fn fn_param(&mut self, p: &ast::FnParam) {
        match p {
            ast::FnParam::SelfValue(loc) => {
                let name = ast::Ident { name: "self".to_string(), span: *loc };
                self.new_decl(DeclKind::FnParam(FnParamData::SelfValue), Some(&name), loc.0, false);
            }
            ast::FnParam::Named { name, ty, span } => {
                let id = self.new_decl(DeclKind::Error, Some(name), span.0, false);
                let ty = self.within(id, |lw| lw.ty(ty));
                self.design.decl_mut(id).kind = DeclKind::FnParam(FnParamData::Named { ty });
            }
            ast::FnParam::Error(loc) => {
                self.new_decl(DeclKind::FnParam(FnParamData::Error), None, loc.0, false);
            }
        }
    }

    /// A struct: generics, then fields, as children of the struct.
    pub(crate) fn struct_decl(&mut self, s: &ast::StructDecl) -> DeclKind {
        self.generics(&s.generics);
        let form = self.fields(&s.fields);
        DeclKind::Struct(StructData { form, bundle: None })
    }

    /// An enum: generics, then one `Variant` per variant.
    pub(crate) fn enum_decl(&mut self, e: &ast::EnumDecl) -> DeclKind {
        self.generics(&e.generics);
        for v in &e.variants {
            let id = self.new_decl(DeclKind::Error, Some(&v.name), v.span.0, false);
            let (kind, meta) = self.within(id, |lw| {
                let form = lw.fields(&v.fields);
                let discriminant = v.discriminant.as_ref().map(|d| lw.expr(d));
                (DeclKind::Variant(VariantData { form, discriminant }), lw.meta(&v.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
        DeclKind::Enum(EnumData::default())
    }

    /// One `Field` child of the current owner per field; the form as written.
    fn fields(&mut self, fields: &ast::Fields) -> FieldsForm {
        let (list, form) = match fields {
            ast::Fields::Named(f) => (f.as_slice(), FieldsForm::Named),
            ast::Fields::Tuple(f) => (f.as_slice(), FieldsForm::Tuple),
            ast::Fields::Unit => return FieldsForm::Unit,
        };
        for f in list {
            let id = self.new_decl(DeclKind::Error, f.name.as_ref(), f.span.0, f.public);
            let (kind, meta) = self.within(id, |lw| {
                (DeclKind::Field(FieldData { dir: f.dir, ty: lw.ty(&f.ty) }), lw.meta(&f.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
        form
    }
}
