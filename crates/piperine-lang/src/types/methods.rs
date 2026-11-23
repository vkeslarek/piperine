//! `self`, methods, `Self` and associated items (part I §3.1, §3.7, GN-03): a `self` parameter of
//! an `impl` is the type the `impl` is written for, `Self` names that type, a method call on a value
//! resolves through the inherent `impl` of its type and then through the traits that give the name
//! (§3.7.8), and `Type::f` and `Type::NAME` name an associated function and a constant.

use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, FileId, FnParamData};

use super::bounds::BoundCheck;
use super::check::Checker;
use super::{Callee, GenericValue, Substitution, Ty};

impl Checker<'_> {
    /// The type of the `self` parameter `param` of the function `function`: the type of its `impl`, or
    /// the trait for a function declared in a trait (`Self` there is opaque until an `impl` gives
    /// it). `self` outside an `impl` and a trait, or not the first parameter, is `E0187`.
    pub(super) fn self_parameter(&mut self, function: DeclId, param: DeclId) -> Option<Ty> {
        let data = self.design.decl_data(param);
        let (file, span) = (data.file, data.span);
        let first = self.design.decl_data(function).children.iter().find(|child| matches!(self.design.decl_data(**child).kind, DeclKind::FnParam(_))) == Some(&param);
        let owner = self.design.decl_data(function).parent;
        let Some(owner) = owner.filter(|owner| matches!(self.design.decl_data(*owner).kind, DeclKind::Impl(_) | DeclKind::Trait(_))) else {
            self.coded(file, "E0187", span, "`self` is the first parameter of a function in an `impl` or a trait, and this one is in neither (§3.7)".to_string(), "`self` outside an `impl`");
            return None;
        };
        if !first {
            self.coded(file, "E0187", span, "`self` must be the first parameter of its function (§3.1)".to_string(), "`self` is not first");
            return None;
        }
        self.self_type(owner, file, span)
    }

    /// `Self` in an `impl` or a trait: the type the `impl` is written for; for a trait, the trait's
    /// own opaque type. `None` after a refusal.
    pub(super) fn self_type(&mut self, owner: DeclId, file: FileId, span: Span) -> Option<Ty> {
        match &self.design.decl_data(owner).kind {
            DeclKind::Impl(data) => {
                let self_ty = data.self_ty;
                self.lower_type(self_ty)
            }
            DeclKind::Trait(_) => Some(Ty::Param(owner)),
            _ => {
                self.unsupported(file, span, "`Self` outside an `impl` or a trait");
                None
            }
        }
    }

    /// Whether `function` is a function written in an `impl` or a trait: a method of the project.
    pub(super) fn is_project_method(&self, function: DeclId) -> bool {
        matches!(self.design.decl_data(function).kind, DeclKind::Fn(_))
            && self.design.decl_data(function).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Impl(_) | DeclKind::Trait(_)))
    }

    /// The function `name` that `recv_ty` has as a method: its inherent `impl`'s, else the one a trait
    /// gives it (§3.7.8). `None` when the project writes none for the type.
    pub(super) fn method_of(&mut self, recv_ty: &Ty, name: &str) -> Option<DeclId> {
        if let Some(own) = self.impls.inherent(recv_ty, name) {
            return Some(own);
        }
        let table = std::rc::Rc::clone(&self.impls);
        let visible = table.visible(recv_ty, name, &mut |ty, bound| BoundCheck::new(self).satisfies(ty, bound));
        visible.into_iter().find(|item| matches!(self.design.decl_data(*item).kind, DeclKind::Fn(_)))
    }

    /// A call `recv.name(args)` of the method `function` of the project: the receiver is the `self`
    /// argument, the arguments bind to the parameters after it, and the call records
    /// `Callee::Method(function)`. `None` after a refusal.
    pub(super) fn method_call_of(&mut self, expr: ExprId, recv: ExprId, recv_ty: &Ty, function: DeclId, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.reach(function);
        let name = self.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (params, ret) = self.declared_signature(function)?;
        let takes_self = matches!(self.design.decl_data(function).children.iter().find_map(|child| match &self.design.decl_data(*child).kind {
            DeclKind::FnParam(param) => Some(param.clone()),
            _ => None,
        }), Some(FnParamData::SelfValue));
        if !takes_self {
            self.coded(file, "E0161", span, format!("`{name}` is an associated function with no `self`: call it as `Type::{name}(..)`, not as a method"), "no `self` parameter");
            for arg in args {
                self.infer(arg.value);
            }
            return None;
        }
        let (own, rest) = params.split_first()?;
        // a function a trait declares has `Self` for its receiver: the receiver's type stands for it
        let mut substitution = Substitution::new();
        if let Ty::Param(owner) = own
            && matches!(self.design.decl_data(*owner).kind, DeclKind::Trait(_))
        {
            substitution.bind(*owner, GenericValue::Type(recv_ty.clone()));
        } else if !recv_ty.fits(own) {
            self.mismatch(file, self.design.expr_node(recv).span, own, recv_ty);
        }
        self.callees.insert(expr, Callee::Method(function));
        let rest: Vec<Ty> = rest.iter().map(|param| substitution.apply(param)).collect();
        self.apply(expr, &rest, args)?;
        Some(substitution.apply(&ret))
    }
}
