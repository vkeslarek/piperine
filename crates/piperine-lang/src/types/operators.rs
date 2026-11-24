//! Operators that are the function of a trait (Appendix H.10): `a & b`, `a | b`, `a ^ b`, `!a`,
//! `a << k`, `a >> k`, `a == b` and `a != b` on a struct or an enum are the function of the one
//! `impl` of the operator trait whose type is the left operand's and whose `Rhs` is the right
//! operand's, and take the `Out` that `impl` gives. When none exists the operator is `E0180`. The
//! built-in operand types keep the §1.5 rules and never reach this module.

use crate::msl::Span;
use crate::pom::{Arg, BinOp, DeclId, DeclKind, ExprId, FileId, Resolution, TypeId};

use super::check::Checker;
use super::inference::Inference;
use super::Ty;

/// An operator whose meaning on a library type is a trait function (H.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OperatorTrait {
    /// `a & b`.
    BitAnd,
    /// `a | b`.
    BitOr,
    /// `a ^ b`.
    BitXor,
    /// `!a`.
    Not,
    /// `a << k`.
    Shl,
    /// `a >> k`.
    Shr,
    /// `a == b`.
    Eq,
    /// `a != b`.
    Ne,
}

impl OperatorTrait {
    /// The trait of a binary operator, `None` for an operator no trait names (§1.5, H.10 rule 5).
    pub(super) fn of_binary(op: BinOp) -> Option<OperatorTrait> {
        match op {
            BinOp::BitAnd => Some(Self::BitAnd),
            BinOp::BitOr => Some(Self::BitOr),
            BinOp::BitXor => Some(Self::BitXor),
            BinOp::Shl => Some(Self::Shl),
            BinOp::Shr => Some(Self::Shr),
            BinOp::Eq => Some(Self::Eq),
            BinOp::Ne => Some(Self::Ne),
            _ => None,
        }
    }

    /// The operator whose trait is called `trait_name` and has the one function of that operator: `None` for any other trait
    /// (`PartialEq` has two functions, `==` and `!=`).
    pub(super) fn named(trait_name: &str) -> Option<OperatorTrait> {
        [Self::BitAnd, Self::BitOr, Self::BitXor, Self::Not, Self::Shl, Self::Shr].into_iter().find(|operator| operator.trait_name() == trait_name)
    }

    /// The trait's name, as the table of built-ins spells it.
    pub(super) fn trait_name(self) -> &'static str {
        match self {
            Self::BitAnd => "BitAnd",
            Self::BitOr => "BitOr",
            Self::BitXor => "BitXor",
            Self::Not => "Not",
            Self::Shl => "Shl",
            Self::Shr => "Shr",
            Self::Eq | Self::Ne => "PartialEq",
        }
    }

    /// The name of the trait function the operator calls.
    pub(super) fn function(self) -> &'static str {
        match self {
            Self::BitAnd => "bitand",
            Self::BitOr => "bitor",
            Self::BitXor => "bitxor",
            Self::Not => "not",
            Self::Shl => "shl",
            Self::Shr => "shr",
            Self::Eq => "eq",
            Self::Ne => "ne",
        }
    }

    /// The operator as written, for messages.
    fn symbol(self) -> &'static str {
        match self {
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Not => "!",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::Eq => "==",
            Self::Ne => "!=",
        }
    }
}

impl Checker<'_> {
    /// Whether an operator on a value of `ty` is a trait function: `ty` is a struct or an enum.
    pub(super) fn takes_operator_traits(ty: &Ty) -> bool {
        matches!(ty, Ty::Struct(_) | Ty::Enum(_))
    }

    /// Whether a project's own `impl` of the trait of `operator` is written for `ty`.
    pub(super) fn has_operator_impl(&self, operator: OperatorTrait, ty: &Ty) -> bool {
        !self.impls.builtin_functions(operator.trait_name(), operator.function(), ty).is_empty()
    }

    /// The type of `whole`, `lhs op rhs` (or `op lhs` when `rhs` is `None`), where `lhs` has the type
    /// `lhs_ty`: the result of the one `impl` function of the trait for `lhs_ty` and the type of `rhs`,
    /// recorded as the call `whole` makes. `E0180` when no `impl` serves the two operand types; `None`
    /// after a diagnostic.
    pub(super) fn operator_call(&mut self, whole: ExprId, operator: OperatorTrait, lhs: ExprId, lhs_ty: &Ty, rhs: Option<ExprId>) -> Option<Ty> {
        let file = self.file_of_expr(whole);
        let span = self.design.expr_node(whole).span;
        let table = std::rc::Rc::clone(&self.impls);
        let candidates = table.builtin_functions(operator.trait_name(), operator.function(), lhs_ty);
        let chosen = match (candidates.as_slice(), rhs) {
            ([only], _) => Some(*only),
            ([], _) => None,
            (several, Some(rhs)) => {
                let rhs_ty = self.infer(rhs);
                several.iter().copied().find(|function| rhs_ty.as_ref().is_some_and(|rhs_ty| self.accepts_operand(*function, lhs_ty, rhs_ty)))
            }
            (_, None) => None,
        };
        let Some(function) = chosen else {
            self.operator_refused(file, span, operator, lhs_ty, rhs, candidates.is_empty());
            return None;
        };
        let args: Vec<Arg> = rhs.map(|value| Arg { name: None, value, res: Resolution::Unresolved, span: self.design.expr_node(value).span }).into_iter().collect();
        self.method_call_of(whole, lhs, lhs_ty, function, &args)
    }

    /// Whether the second parameter of the `impl` function `function` takes a value of type `rhs_ty` once the
    /// `impl`'s parameters are read from `lhs_ty` and `rhs_ty`.
    fn accepts_operand(&mut self, function: DeclId, lhs_ty: &Ty, rhs_ty: &Ty) -> bool {
        let Some((params, _)) = self.declared_signature(function) else { return false };
        let [own, wanted, ..] = params.as_slice() else { return false };
        let generic_owner = self.design.decl_data(function).parent.filter(|owner| matches!(self.design.decl_data(*owner).kind, DeclKind::Impl(_)) && self.design.decl_data(*owner).generics_span.is_some());
        let Some(owner) = generic_owner else { return rhs_ty.fits(wanted) };
        let signature = self.generic_signature(owner);
        let mut inference = Inference::new(&signature);
        inference.unify(own, lhs_ty);
        inference.unify(wanted, rhs_ty);
        rhs_ty.fits(&inference.found().apply(wanted))
    }

    /// `E0180`: no `impl` of the operator's trait serves the operands. `missing` is true when the left type has
    /// no `impl` of the trait at all, false when it has some for other right operands.
    fn operator_refused(&mut self, file: FileId, span: Span, operator: OperatorTrait, lhs_ty: &Ty, rhs: Option<ExprId>, missing: bool) {
        let name = lhs_ty.source_name(self.design);
        let trait_name = operator.trait_name();
        let message = if missing {
            format!("mismatched types: `{name}` has no `{trait_name}`, so `{}` does not apply to it (Appendix H.10 rule 1)", operator.symbol())
        } else {
            let found = rhs.and_then(|rhs| self.infer(rhs)).map_or_else(|| "the right operand".to_string(), |ty| ty.source_name(self.design));
            format!("mismatched types: no `impl {trait_name}` of `{name}` takes `{found}` as its right operand (Appendix H.10 rule 1)")
        };
        self.coded(file, "E0180", span, message, "an operator no `impl` gives");
        if let Some(rhs) = rhs {
            self.infer(rhs);
        }
    }
}

impl Checker<'_> {
    /// The rules of `impl Tr for T` where `Tr` is a built-in trait the project may implement but `Repr` (H.8, H.10): the
    /// `impl` defines the trait's functions (`bitand`, `eq` and `ne`, `from`, `default`, ..) and no other item, each
    /// with the signature the trait gives under `Self = T` and the `Rhs` and `Out` the `impl` writes or their
    /// defaults. Each is `E0187` naming the trait and the type. The written arguments are typed here so every use sees them.
    pub(super) fn builtin_impl_rules(&mut self, decl: DeclId, self_ty: TypeId) {
        let table = std::rc::Rc::clone(&self.impls);
        let Some(entry) = table.impls().iter().find(|entry| entry.decl == decl) else { return };
        let Some(trait_name) = entry.builtin_trait else { return };
        let (file, span) = (self.design.decl_data(decl).file, self.design.decl_data(decl).span);
        let Some(own) = self.lower_type(self_ty) else { return };
        let written: Option<Vec<Ty>> = entry.trait_arguments.iter().map(|argument| self.lower_type(*argument)).collect();
        let Some(written) = written else { return };
        let head = format!("`impl {trait_name} for {}`", own.source_name(self.design));
        let argument = |index: usize, default: Ty| written.get(index).cloned().unwrap_or(default);
        // (function, parameters after `self`, result)
        let wanted: Vec<(&str, Vec<Ty>, Ty)> = match (OperatorTrait::named(trait_name), trait_name) {
            (Some(operator @ (OperatorTrait::BitAnd | OperatorTrait::BitOr | OperatorTrait::BitXor)), _) => vec![(operator.function(), vec![argument(0, own.clone())], argument(1, own.clone()))],
            (Some(operator @ (OperatorTrait::Shl | OperatorTrait::Shr)), _) => vec![(operator.function(), vec![argument(0, Ty::UInt(64))], argument(1, own.clone()))],
            (Some(operator @ OperatorTrait::Not), _) => vec![(operator.function(), vec![], argument(0, own.clone()))],
            (_, "PartialEq") => vec![("eq", vec![argument(0, own.clone())], argument(1, Ty::Bool)), ("ne", vec![argument(0, own.clone())], argument(1, Ty::Bool))],
            (_, "From") => vec![("from", vec![], own.clone())],
            (_, "Default") => vec![("default", vec![], own.clone())],
            _ => return,
        };
        let items: Vec<(String, DeclId)> = self.design.decl_data(decl).children.iter().filter(|child| matches!(self.design.decl_data(**child).kind, DeclKind::Fn(_) | DeclKind::Const(_))).filter_map(|child| Some((self.design.decl_data(*child).name.as_ref()?.text.clone(), *child))).collect();
        for (name, ..) in &wanted {
            if !items.iter().any(|(own_name, _)| own_name == name) {
                self.coded(file, "E0187", span, format!("{head} leaves out `{name}` (Appendix H)"), "a function of the trait left out");
            }
        }
        for (name, item) in &items {
            let (item_file, item_span) = (self.design.decl_data(*item).file, self.design.decl_data(*item).span);
            let Some((_, parameters, result)) = wanted.iter().find(|(wanted_name, ..)| wanted_name == name) else {
                self.coded(item_file, "E0187", item_span, format!("{head} defines `{name}`, which the trait does not have (Appendix H)"), "an item the trait lacks");
                continue;
            };
            let Some((params, ret)) = self.declared_signature(*item) else { continue };
            let takes_self = !matches!(trait_name, "From" | "Default");
            let given: Vec<Ty> = if takes_self { std::iter::once(own.clone()).chain(parameters.iter().cloned()).collect() } else if trait_name == "From" { written.clone() } else { Vec::new() };
            let found = Ty::Fn(params, Box::new(ret));
            let wanted_type = Ty::Fn(given, Box::new(result.clone()));
            if !found.fits(&wanted_type) {
                self.coded(item_file, "E0187", item_span, format!("{head}: `{name}` has the type `{}`, the trait gives it `{}` (Appendix H)", found.name(self.design), wanted_type.name(self.design)), "a signature the trait does not give");
            }
        }
    }
}
