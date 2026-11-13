//! Handles over value items: `Const` here; functions, structs, enums, traits,
//! impls and imports as the item tasks add them.

use super::body::{Expr, Path, Type};
use super::{Decl, Reflect};
use crate::pom::body::{Block, LitKind};
use crate::pom::decl::{ConstData, DeclKind, Dir, FieldData, FieldsForm, FnData, FnParamData, GenericParamData, StructData, VariantData};

/// `const NAME: ty = value;`
#[derive(Clone, Copy)]
pub struct Const<'d> {
    decl: Decl<'d>,
}

impl<'d> Const<'d> {
    fn data(self) -> &'d ConstData {
        match self.decl.kind() {
            DeclKind::Const(c) => c,
            // `Decl::as_const` only builds this handle over a constant.
            _ => unreachable!("Const handle over a non-constant declaration"),
        }
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }

    /// The value expression; none for a trait's constant declaration.
    pub fn value(self) -> Option<Expr<'d>> {
        self.data().value.map(|v| self.decl.design().expr(v))
    }
}

impl<'d> Reflect<'d> for Const<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Const<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Const<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Const({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_const(self) -> Option<Const<'d>> {
        matches!(self.kind(), DeclKind::Const(_)).then_some(Const { decl: self })
    }
}

/// `fn name<..>(params) -> ret where cond { body }`
#[derive(Clone, Copy)]
pub struct Function<'d> {
    decl: Decl<'d>,
}

impl<'d> Function<'d> {
    fn data(self) -> &'d FnData {
        match self.decl.kind() {
            DeclKind::Fn(f) => f,
            // `Decl::as_function` only builds this handle over a function.
            _ => unreachable!("Function handle over a non-function declaration"),
        }
    }

    pub fn is_const(self) -> bool {
        self.data().is_const
    }

    /// The ABI string of `extern "C" fn`, without quotes.
    pub fn abi(self) -> Option<&'d str> {
        match &self.data().abi {
            Some(l) => match &l.kind {
                LitKind::Str(text) => Some(text.as_str()),
                _ => None,
            },
            None => None,
        }
    }

    pub fn generics(self) -> Vec<GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    /// The parameters in source order; a method's `self` comes first.
    pub fn params(self) -> Vec<FnParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_fn_param()).collect()
    }

    pub fn ret(self) -> Option<Type<'d>> {
        self.data().ret.map(|t| self.decl.design().ty(t))
    }

    /// The `where` condition.
    pub fn where_clause(self) -> Option<Expr<'d>> {
        self.data().where_.map(|e| self.decl.design().expr(e))
    }

    /// The body block; none for a declaration ending in `;`.
    pub fn body(self) -> Option<&'d Block> {
        self.data().body.as_ref()
    }
}

/// One parameter of a function.
#[derive(Clone, Copy)]
pub struct FnParam<'d> {
    decl: Decl<'d>,
}

impl<'d> FnParam<'d> {
    fn data(self) -> &'d FnParamData {
        match self.decl.kind() {
            DeclKind::FnParam(p) => p,
            _ => unreachable!("FnParam handle over a non-parameter declaration"),
        }
    }

    pub fn is_self(self) -> bool {
        matches!(self.data(), FnParamData::SelfValue)
    }

    /// The declared type; none for `self` and for a malformed parameter.
    pub fn ty(self) -> Option<Type<'d>> {
        match self.data() {
            FnParamData::Named { ty } => Some(self.decl.design().ty(*ty)),
            FnParamData::SelfValue | FnParamData::Error => None,
        }
    }
}

/// `<T: Bound = Default>` or `<const N: u32 = 4>`.
#[derive(Clone, Copy)]
pub struct GenericParam<'d> {
    decl: Decl<'d>,
}

impl<'d> GenericParam<'d> {
    fn data(self) -> &'d GenericParamData {
        match self.decl.kind() {
            DeclKind::GenericParam(g) => g,
            _ => unreachable!("GenericParam handle over a non-generic declaration"),
        }
    }

    pub fn is_const(self) -> bool {
        matches!(self.data(), GenericParamData::Const { .. })
    }

    /// The bounds of a type parameter, in source order.
    pub fn bounds(self) -> Vec<Path<'d>> {
        match self.data() {
            GenericParamData::Type { bounds, .. } => bounds.iter().map(|b| self.decl.design().path(*b)).collect(),
            GenericParamData::Const { .. } => Vec::new(),
        }
    }

    /// The type of a const parameter.
    pub fn const_type(self) -> Option<Type<'d>> {
        match self.data() {
            GenericParamData::Const { ty, .. } => Some(self.decl.design().ty(*ty)),
            GenericParamData::Type { .. } => None,
        }
    }

    /// The default of a type parameter.
    pub fn default_type(self) -> Option<Type<'d>> {
        match self.data() {
            GenericParamData::Type { default, .. } => default.map(|t| self.decl.design().ty(t)),
            GenericParamData::Const { .. } => None,
        }
    }

    /// The default of a const parameter.
    pub fn default_value(self) -> Option<Expr<'d>> {
        match self.data() {
            GenericParamData::Const { default, .. } => default.map(|e| self.decl.design().expr(e)),
            GenericParamData::Type { .. } => None,
        }
    }
}

impl<'d> Reflect<'d> for Function<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl<'d> Reflect<'d> for FnParam<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl<'d> Reflect<'d> for GenericParam<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Function<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl PartialEq for FnParam<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl PartialEq for GenericParam<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Function<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Function({:?})", self.decl.id())
    }
}

impl std::fmt::Debug for FnParam<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FnParam({:?})", self.decl.id())
    }
}

impl std::fmt::Debug for GenericParam<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GenericParam({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_function(self) -> Option<Function<'d>> {
        matches!(self.kind(), DeclKind::Fn(_)).then_some(Function { decl: self })
    }

    pub fn as_fn_param(self) -> Option<FnParam<'d>> {
        matches!(self.kind(), DeclKind::FnParam(_)).then_some(FnParam { decl: self })
    }

    pub fn as_generic_param(self) -> Option<GenericParam<'d>> {
        matches!(self.kind(), DeclKind::GenericParam(_)).then_some(GenericParam { decl: self })
    }
}

/// `struct S<..> { fields }`
#[derive(Clone, Copy)]
pub struct Struct<'d> {
    decl: Decl<'d>,
}

impl<'d> Struct<'d> {
    fn data(self) -> &'d StructData {
        match self.decl.kind() {
            DeclKind::Struct(s) => s,
            _ => unreachable!("Struct handle over a non-struct declaration"),
        }
    }

    /// Named, tuple or unit, as written.
    pub fn form(self) -> FieldsForm {
        self.data().form
    }

    /// Whether the struct is a bundle; none until classification.
    pub fn bundle(self) -> Option<bool> {
        self.data().bundle
    }

    pub fn generics(self) -> Vec<GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    pub fn fields(self) -> Vec<Field<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_field()).collect()
    }
}

/// A field of a struct or of a variant.
#[derive(Clone, Copy)]
pub struct Field<'d> {
    decl: Decl<'d>,
}

impl<'d> Field<'d> {
    fn data(self) -> &'d FieldData {
        match self.decl.kind() {
            DeclKind::Field(f) => f,
            _ => unreachable!("Field handle over a non-field declaration"),
        }
    }

    pub fn dir(self) -> Option<Dir> {
        self.data().dir
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }
}

/// `enum E<..> { variants }`
#[derive(Clone, Copy)]
pub struct Enum<'d> {
    decl: Decl<'d>,
}

impl<'d> Enum<'d> {
    pub fn generics(self) -> Vec<GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    pub fn variants(self) -> Vec<Variant<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_variant()).collect()
    }
}

/// A variant of an enum.
#[derive(Clone, Copy)]
pub struct Variant<'d> {
    decl: Decl<'d>,
}

impl<'d> Variant<'d> {
    fn data(self) -> &'d VariantData {
        match self.decl.kind() {
            DeclKind::Variant(v) => v,
            _ => unreachable!("Variant handle over a non-variant declaration"),
        }
    }

    pub fn form(self) -> FieldsForm {
        self.data().form
    }

    pub fn fields(self) -> Vec<Field<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_field()).collect()
    }

    /// The `= value` after the variant.
    pub fn discriminant(self) -> Option<Expr<'d>> {
        self.data().discriminant.map(|e| self.decl.design().expr(e))
    }
}

impl<'d> Reflect<'d> for Struct<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl<'d> Reflect<'d> for Field<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl<'d> Reflect<'d> for Enum<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl<'d> Reflect<'d> for Variant<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Struct<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl PartialEq for Field<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl PartialEq for Enum<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl PartialEq for Variant<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Struct<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Struct({:?})", self.decl.id())
    }
}

impl std::fmt::Debug for Field<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Field({:?})", self.decl.id())
    }
}

impl std::fmt::Debug for Enum<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Enum({:?})", self.decl.id())
    }
}

impl std::fmt::Debug for Variant<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Variant({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_struct(self) -> Option<Struct<'d>> {
        matches!(self.kind(), DeclKind::Struct(_)).then_some(Struct { decl: self })
    }

    pub fn as_field(self) -> Option<Field<'d>> {
        matches!(self.kind(), DeclKind::Field(_)).then_some(Field { decl: self })
    }

    pub fn as_enum(self) -> Option<Enum<'d>> {
        matches!(self.kind(), DeclKind::Enum(_)).then_some(Enum { decl: self })
    }

    pub fn as_variant(self) -> Option<Variant<'d>> {
        matches!(self.kind(), DeclKind::Variant(_)).then_some(Variant { decl: self })
    }
}
