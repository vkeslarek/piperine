//! The values of generic structs and enums (part I §3.8, §1.3, GN-06): a constructor of a generic
//! type finds the type's arguments from the fields it is given and the type it is expected to
//! have, a field read and a pattern see the field types with the arguments substituted, and a
//! derive holds for the arguments that implement it (H.8 rule 6).

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, FileId};

use super::aggregate::FieldType;
use super::check::Checker;
use super::inference::Inference;
use super::{GenericSignature, GenericValue, Nominal, Substitution, Ty};

impl Checker<'_> {
    /// The type of the generic declaration `decl` written over its own parameters (`Pair<T, U>`), which
    /// the field types of its layout mention; the type itself, plain, for a declaration that is not generic.
    pub(super) fn open_type(&mut self, decl: DeclId) -> Ty {
        let signature = GenericSignature::of(self.design, decl);
        let arguments = signature
            .parameters()
            .iter()
            .map(|parameter| match parameter.kind {
                super::ParameterKind::Const(_) => GenericValue::Const(super::ConstTerm::Param(parameter.decl)),
                _ => GenericValue::Type(Ty::Param(parameter.decl)),
            })
            .collect();
        let nominal = Nominal { decl, arguments };
        if matches!(self.design.decl_data(decl).kind, DeclKind::Enum(_)) { Ty::Enum(nominal) } else { Ty::Struct(nominal) }
    }

    /// What the parameters of the type of `ty` are replaced by: its own arguments.
    pub(super) fn nominal_substitution(&self, nominal: &Nominal) -> Substitution {
        Substitution::of(&GenericSignature::of(self.design, nominal.decl), &nominal.arguments)
    }

    /// The fields of `owner` (a struct, or a variant) as the instance `scrutinee` has them: the declared
    /// field types with the arguments of the type substituted.
    pub(super) fn instance_layout(&mut self, owner: DeclId, scrutinee: &Ty) -> Option<Vec<FieldType>> {
        let layout = self.fields_of(owner)?;
        let (Ty::Struct(nominal) | Ty::Enum(nominal)) = scrutinee else { return Some(layout) };
        if nominal.arguments.is_empty() {
            return Some(layout);
        }
        let substitution = self.nominal_substitution(nominal);
        Some(layout.into_iter().map(|field| FieldType { ty: self.substitute(&substitution, &field.ty), ..field }).collect())
    }

    /// The type a constructor builds once its arguments are found, and the field types under them. A
    /// type that is not generic is `open` and its `layout` as they are. For a generic one each
    /// `(field position, value)` of `values` gives what its type says about the arguments, the type the
    /// expression is expected to have (`call_expected`) the rest, a default fills a parameter left
    /// open, and a parameter nothing gives is `E0181`; each argument must satisfy its bounds (`E0187`).
    pub(super) fn close_constructor(&mut self, expr: ExprId, open: Ty, layout: Vec<FieldType>, values: &[(usize, ExprId)], file: FileId, span: Span) -> Option<(Ty, Vec<FieldType>)> {
        let (Ty::Struct(nominal) | Ty::Enum(nominal)) = &open else { return Some((open, layout)) };
        if nominal.arguments.is_empty() {
            return Some((open, layout));
        }
        let type_decl = nominal.decl;
        let signature = self.generic_signature(type_decl);
        let mut inference = Inference::new(&signature);
        for (position, value) in values {
            if let (Some(field), Some(given)) = (layout.get(*position), self.infer(*value)) {
                inference.unify(&field.ty, &given);
            }
        }
        if let Some(expected) = self.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty) {
            inference.unify(&open, &expected);
        }
        let substitution = self.complete_arguments(&mut inference, &signature, type_decl, type_decl, file, span)?;
        let arguments: Vec<GenericValue> = signature.parameters().iter().filter_map(|parameter| substitution.get(parameter.decl).cloned()).collect();
        self.arguments_satisfy_bounds(&signature, &arguments, type_decl, file, span)?;
        let closed = Nominal { decl: type_decl, arguments };
        let ty = if matches!(self.design.decl_data(type_decl).kind, DeclKind::Enum(_)) { Ty::Enum(closed) } else { Ty::Struct(closed) };
        let fields = layout.into_iter().map(|field| FieldType { ty: self.substitute(&substitution, &field.ty), ..field }).collect();
        Some((ty, fields))
    }
}
