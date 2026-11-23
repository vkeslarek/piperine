//! The two functions of §22.1 that are generic over `Repr` and have no `Logic` in them (`same`, `cast`), typed by their normative contract at the concrete type until `generics`
//! lands (spec Q1 (b)). A call whose callee is the stdlib's own item never reaches
//! `signature_parts`, which refuses a generic function and an `extern` one; a project's own
//! function of the same name is an ordinary function (§22 rule 3).

use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode};

use super::check::Checker;
use super::{Callee, Nominal, Ty};

/// A function of §22.1 the checker types by its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    /// `same<T: Repr>(a: T, b: T) -> bool` (§22.1.4).
    Same,
    /// `cast<A: Repr, B: Repr>(a: A) -> B`, `A::WIDTH == B::WIDTH` (§22.1.3).
    Cast,
}

impl Intrinsic {
    /// The function of §22.1 called `name`, if it is one of the two.
    fn named(name: &str) -> Option<Intrinsic> {
        match name {
            "same" => Some(Intrinsic::Same),
            "cast" => Some(Intrinsic::Cast),
            _ => None,
        }
    }
}

impl Checker<'_> {
    /// Which §22.1 function `function` is, when it is the stdlib's own (`std/values.msl`).
    pub(super) fn intrinsic_of(&self, function: DeclId) -> Option<Intrinsic> {
        let data = self.design.decl_data(function);
        if !matches!(data.kind, DeclKind::Fn(_)) || !self.is_std(function) {
            return None;
        }
        let parent = data.parent.map(|p| self.design.decl_data(p))?;
        let in_values = matches!(parent.kind, DeclKind::Namespace(_)) && parent.name.as_ref().is_some_and(|n| n.text == "values");
        in_values.then(|| Intrinsic::named(&data.name.as_ref()?.text)).flatten()
    }

    /// A call of one of the two: its type from the arguments, the turbofish or the type the
    /// context expects, the contract's errors reported (`E0701`, `E0702`), the callee recorded.
    pub(super) fn intrinsic_call(&mut self, expr: ExprId, callee: ExprId, intrinsic: Intrinsic, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let written = self.turbofish_types(callee);
        let expected = self.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty);
        let result = match intrinsic {
            Intrinsic::Same => {
                let value = self.operand_type(file, span, written.first().cloned(), args.first())?;
                self.apply(expr, &[value.clone(), value.clone()], args)?;
                Ty::Bool
            }
            Intrinsic::Cast => {
                let from = self.operand_type(file, span, written.first().cloned(), args.first())?;
                // the result is `Option<B>` (§22.1.3): the expected type names `B` through its `Option`
                let expected = expected.map(|ty| match ty {
                    Ty::Option(inner) => *inner,
                    other => other,
                });
                let Some(to) = written.get(1).cloned().or(expected) else {
                    self.unsupported(file, span, "`cast` whose result type is not written (use `cast::<A, B>` or an expected type)");
                    return None;
                };
                let to = self.value_type(file, span, Some(to))?;
                self.apply(expr, &[from.clone()], args)?;
                self.same_width(file, span, &from, &to)?;
                self.refuse_enum_in_a_sequence(file, span, &to)?;
                Ty::Option(Box::new(to))
            }
        };
        self.callees.insert(expr, Callee::Intrinsic(intrinsic));
        Some(result)
    }

    /// The types written in the turbofish of the callee's last segment (`cast::<u8, Mode>`).
    fn turbofish_types(&mut self, callee: ExprId) -> Vec<Ty> {
        let ExprKind::Path(path) = &self.design.expr_node(callee).kind else { return Vec::new() };
        let Some(GenericArgsNode::Angle { args, .. }) = self.design.path_node(*path).segments.last().and_then(|s| s.args.clone()) else { return Vec::new() };
        args.iter()
            .filter_map(|arg| match arg {
                GenericArgNode::Type(ty) => self.lower_type(*ty),
                GenericArgNode::Const(_) => None,
            })
            .collect()
    }

    /// The `Repr` type a call works at: the turbofish's, else the type of its first argument.
    fn operand_type(&mut self, file: FileId, span: Span, written: Option<Ty>, first: Option<&Arg>) -> Option<Ty> {
        let ty = match written {
            Some(ty) => Some(ty),
            None => self.infer(first?.value),
        };
        self.value_type(file, span, ty)
    }

    /// `ty` as the `Repr` type a call works at: `None` after a diagnostic when it has none.
    fn value_type(&mut self, file: FileId, span: Span, ty: Option<Ty>) -> Option<Ty> {
        let ty = ty?;
        if !self.repr_ok(&ty) {
            let message = format!("`{}` has no `Repr`: this function of §22.1 works on a type that implements it (part I §2)", ty.name(self.design));
            self.coded(file, "E0112", span, message, "no `Repr`");
            return None;
        }
        Some(ty)
    }

    /// `P0001` (`GAPS.md` L58) for a `cast` target that is a tuple or an array holding an enum, directly or
    /// through a struct or a payload: the generated decoders judge a tag that names no variant for an enum and a
    /// struct, and the runtime's tuples and arrays do not yet, so `None` would not be given where the rule needs it.
    fn refuse_enum_in_a_sequence(&mut self, file: FileId, span: Span, to: &Ty) -> Option<()> {
        if self.sequence_holds_enum(to, false) {
            self.unsupported(file, span, &format!("`cast` to `{}`, a tuple or an array holding an enum: a tag that names no variant is not judged inside one yet", to.name(self.design)));
            return None;
        }
        Some(())
    }

    /// Whether `ty` holds an enum below a tuple or an array (`in_sequence`: already below one).
    fn sequence_holds_enum(&self, ty: &Ty, in_sequence: bool) -> bool {
        match ty {
            Ty::Enum(Nominal { decl, .. }) => {
                in_sequence
                    || self.design.decl_data(*decl).children.iter().any(|variant| {
                        self.types_of_fields(*variant).iter().any(|field| self.sequence_holds_enum(field, false))
                    })
            }
            Ty::Tuple(items) => items.iter().any(|item| self.sequence_holds_enum(item, true)),
            Ty::Array(element, _) => self.sequence_holds_enum(element, true),
            Ty::Struct(Nominal { decl, .. }) => self.types_of_fields(*decl).iter().any(|field| self.sequence_holds_enum(field, in_sequence)),
            _ => false,
        }
    }

    /// The types of the fields of the struct or variant `owner`.
    fn types_of_fields(&self, owner: DeclId) -> Vec<Ty> {
        self.layouts.get(&owner).map(|fields| fields.iter().map(|f| f.ty.clone()).collect()).unwrap_or_default()
    }

    /// `E0701` unless `from` and `to` have the same number of levels (§22.1.3 rule 3).
    fn same_width(&mut self, file: FileId, span: Span, from: &Ty, to: &Ty) -> Option<()> {
        let (a, b) = (self.repr_levels(from)?, self.repr_levels(to)?);
        if a != b {
            let message = format!("`cast` between `{}` ({a} levels) and `{}` ({b} levels): the widths differ (§22.1.3)", from.name(self.design), to.name(self.design));
            self.coded(file, "E0701", span, message, "widths differ");
            return None;
        }
        Some(())
    }
}
