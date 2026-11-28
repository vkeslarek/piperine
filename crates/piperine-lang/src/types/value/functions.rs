//! Functions and constants of the reachable set: the declared signature of a function, its body against its result, and
//! the initializer of a `const`.

use crate::refusal::forms::ValueForm;
use crate::pom::{BlockNode, DeclData, DeclId, DeclKind, FileId, FnParamData};

use super::ExpressionTyping;
use super::flow::ExitFrame;
use crate::types::check::Context;
use crate::types::{Obligation, Ty};

/// The parts of a function signature, each typed on its own.
pub(in crate::types) struct SignatureParts {
    params: Vec<(DeclId, Option<Ty>)>,
    ret: Option<Ty>,
    ok: bool,
}

impl ExpressionTyping<'_, '_> {
    /// `const NAME: T = value;`: typed by its declared type, its initializer checked once.
    pub(in crate::types) fn constant(&mut self, decl: DeclId) {
        let DeclKind::Const(data) = self.state.design.decl_data(decl).kind.clone() else { return };
        let Some(ty) = self.lower_type(data.ty) else { return };
        match data.value {
            Some(value) => self.within(Context::Const, |this| this.expressions().check(value, &ty)),
            None => {
                let d = self.state.design.decl_data(decl);
                let (file, span) = (d.file, d.span);
                let name = d.name.as_ref().map_or_else(String::new, |name| name.text.clone());
                let in_trait = d.parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_)));
                if in_trait {
                    self.refuse(file, span, ValueForm::ConstWithoutValue);
                } else {
                    // §3.9.2: a `const` has a value everywhere but in a trait.
                    self.coded(file, "E0188", span, format!("the `const` `{name}` has no value"), "a `const` outside a trait has a value");
                }
            }
        }
    }

    /// The declared parameter types and result of a function, or `None` after a `P0001`.
    pub(in crate::types) fn signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
        let data = self.state.design.decl_data(decl);
        if data.generics_span.is_some() {
            // a use of a generic function binds its arguments after inference (T20, T21): until then
            // a call or a function value of one is outside the subset
            let (file, span) = (data.file, data.span);
            self.refuse(file, span, ValueForm::GenericFunction);
            return None;
        }
        let parts = self.signature_parts(decl)?;
        let params: Option<Vec<Ty>> = parts.params.iter().map(|(_, ty)| ty.clone()).collect();
        match (parts.ok, params, parts.ret) {
            (true, Some(params), Some(ret)) => Some((params, ret)),
            _ => None,
        }
    }

    /// The declared parameter types and result of a function whatever its generics: a generic
    /// function's mention its type parameters as `Ty::Param`. `None` after a refusal.
    pub(in crate::types) fn declared_signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
        let parts = self.signature_parts(decl)?;
        let params: Option<Vec<Ty>> = parts.params.iter().map(|(_, ty)| ty.clone()).collect();
        match (parts.ok, params, parts.ret) {
            (true, Some(params), Some(ret)) => Some((params, ret)),
            _ => None,
        }
    }

    /// The signature piece by piece: each parameter with its type if it has one, the result if
    /// it has one, and whether the signature as a whole is inside the subset. A refused piece
    /// does not stop the others from being typed, so the body can still be checked.
    pub(in crate::types) fn signature_parts(&mut self, decl: DeclId) -> Option<SignatureParts> {
        // what the types of a generic signature defer (`[T; N + 1]`) is recorded under the function
        let generic_item = self.state.design.decl_data(decl).generics_span.map(|_| decl);
        let outer = self.state.generics.replace_generic_item(generic_item);
        let parts = self.signature_parts_of(decl);
        self.state.generics.replace_generic_item(outer);
        parts
    }

    /// [`ExpressionTyping::signature_parts`] with the generic item set.
    fn signature_parts_of(&mut self, decl: DeclId) -> Option<SignatureParts> {
        let data: DeclData = self.state.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let mut ok = true;
        if data.generics_span.is_some() {
            // the list is held to §3.8; the parameters below are typed with `Ty::Param`, so the body
            // is checked once at its definition (Q1 (c))
            self.generics().generic_signature(decl);
        }
        // a generic `extern "intrinsic"` function (the library's `logic`, `matches_bits`, `parse_table`) is typed through
        // the generic path like any generic function; the other `extern` functions keep their refusal
        if f.abi.is_some() && data.generics_span.is_none() {
            self.refuse(data.file, data.span, ValueForm::ExternFunction);
            ok = false;
        }
        let mut params = Vec::new();
        for child in data.children.iter().copied() {
            let DeclKind::FnParam(p) = &self.state.design.decl_data(child).kind else { continue };
            let ty = match p {
                FnParamData::Named { ty } => self.lower_type(*ty),
                FnParamData::SelfValue => self.self_parameter(decl, child),
                FnParamData::Error => {
                    let d = self.state.design.decl_data(child);
                    self.refuse(d.file, d.span, ValueForm::SelfParameter);
                    None
                }
            };
            ok &= ty.is_some();
            params.push((child, ty));
        }
        let ret = match f.ret {
            Some(r) => self.lower_type(r),
            None => Some(Ty::Unit),
        };
        ok &= ret.is_some();
        Some(SignatureParts { params, ret, ok })
    }

    /// A function of the reachable set: its `where`, then its body against its declared result.
    /// A call to it is typed from the signature alone, so a recursive function never loops. A
    /// signature outside the subset is refused once and the body is still checked, with the
    /// parameters that could not be typed silent.
    pub(in crate::types) fn function(&mut self, decl: DeclId) {
        let generic_item = self.state.design.decl_data(decl).generics_span.map(|_| decl);
        let outer = self.state.generics.replace_generic_item(generic_item);
        self.function_of(decl);
        self.state.generics.replace_generic_item(outer);
    }

    /// [`ExpressionTyping::function`] with the generic item set.
    fn function_of(&mut self, decl: DeclId) {
        let Some(parts) = self.signature_parts(decl) else { return };
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        // §3 rule 8: the `where` ranges over const generics and associated consts, which the
        // subset has none of. Checked before the parameters are typed, a read of a value
        // parameter in it is refused (`P0001`) and the specification gives a non-`bool`
        // condition no code of its own, so that is the plain mismatch.
        if let Some(cond) = f.where_ {
            if self.state.generics.generic_item() == Some(decl) {
                self.generics().defer(Obligation::Where { item: decl, condition: cond });
            }
            self.state.values.set_fn_where(true);
            self.within(Context::Const, |this| this.expressions().check(cond, &Ty::Bool));
            self.state.values.set_fn_where(false);
        }
        for (param, ty) in parts.params {
            match ty {
                // QC3: §3 rule 7 makes a nature parameter a handle and says nothing of an array of them
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => {
                    let (file, span) = (self.state.design.decl_data(param).file, self.state.design.decl_data(param).span);
                    self.refuse(file, span, ValueForm::ArrayOfCarriersAsFunctionParameter);
                    self.state.values.poison(param);
                }
                Some(ty) => {
                    self.modules().note_handle(param, &ty);
                    self.state.result.locals.insert(param, ty);
                }
                None => {
                    self.state.values.poison(param);
                }
            }
        }
        // before the body: a handle kept is `E0133`, not the mismatch the same span would also report
        self.modules().check_handle_result(decl, parts.ret.as_ref());
        self.modules().check_handle_uses(decl);
        self.state.values.begin_function(ExitFrame::new(parts.ret.clone(), false));
        match (&f.body, parts.ret) {
            (Some(body), Some(ret)) => self.function_body(body, &ret, data.file),
            (Some(body), None) => {
                self.statements(&body.stmts, None);
            }
            // a function a trait declares has no body; its implementations do
            (None, _) if data.parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_))) => {}
            // an `extern` function is implemented outside the language: its signature is all there is to check
            (None, _) if f.abi.is_some() => {}
            (None, _) => self.refuse(data.file, data.span, ValueForm::FunctionWithoutBody),
        }
        self.state.values.set_context(Context::Module);
    }

    /// The statements of a body against the declared result. A missing result is reported
    /// whatever else the body got wrong, since it is its own mistake.
    fn function_body(&mut self, body: &BlockNode, ret: &Ty, file: FileId) {
        if let Some(found) = self.statements(&body.stmts, Some(ret))
            && !self.generics().compatible_at(file, body.span, &found, ret)
            && found != Ty::Never
        {
            self.mismatch(file, body.span, ret, &found);
        }
    }
}
