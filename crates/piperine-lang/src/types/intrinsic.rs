//! The five functions of §22.1 that are generic over `Repr` (`merge`, `same`, `cast`, `logic`,
//! `matches_bits`), typed by their normative contract at the concrete type until `generics`
//! lands (spec Q1 (b)). A call whose callee is the stdlib's own item never reaches
//! `signature_parts`, which refuses a generic function and an `extern` one; a project's own
//! function of the same name is an ordinary function (§22 rule 3).

use crate::msl::Span;
use crate::pom::{Arg, DeclId, DeclKind, ExprId, ExprKind, FileId, GenericArgNode, GenericArgsNode, LitKind};

use super::check::Checker;
use super::{Callee, Ty};

/// A function of §22.1 the checker types by its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intrinsic {
    /// `merge<T: Repr>(a: T, b: T) -> T` (§22.1.2).
    Merge,
    /// `same<T: Repr>(a: T, b: T) -> bool` (§22.1.4).
    Same,
    /// `cast<A: Repr, B: Repr>(a: A) -> B`, `A::WIDTH == B::WIDTH` (§22.1.3).
    Cast,
    /// `logic<T: Repr>(pattern: &str) -> T` (§22.1.4).
    Logic,
    /// `matches_bits<T: Repr>(s: T, pattern: &str) -> bool` (§22.1.4).
    MatchesBits,
}

impl Intrinsic {
    /// The function of §22.1 called `name`, if it is one of the five.
    fn named(name: &str) -> Option<Intrinsic> {
        match name {
            "merge" => Some(Intrinsic::Merge),
            "same" => Some(Intrinsic::Same),
            "cast" => Some(Intrinsic::Cast),
            "logic" => Some(Intrinsic::Logic),
            "matches_bits" => Some(Intrinsic::MatchesBits),
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

    /// A call of one of the five: its type from the arguments, the turbofish or the type the
    /// context expects, the contract's errors reported (`E0701`, `E0702`), the callee recorded.
    pub(super) fn intrinsic_call(&mut self, expr: ExprId, callee: ExprId, intrinsic: Intrinsic, args: &[Arg]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let written = self.turbofish_types(callee);
        let expected = self.call_expected.take().filter(|(at, _)| *at == expr).map(|(_, ty)| ty);
        let result = match intrinsic {
            Intrinsic::Merge | Intrinsic::Same => {
                let value = self.operand_type(file, span, written.first().cloned(), args.first())?;
                self.apply(expr, &[value.clone(), value.clone()], args)?;
                if intrinsic == Intrinsic::Merge { value } else { Ty::Bool }
            }
            Intrinsic::Cast => {
                let from = self.operand_type(file, span, written.first().cloned(), args.first())?;
                let Some(to) = written.get(1).cloned().or(expected) else {
                    self.unsupported(file, span, "`cast` whose result type is not written (use `cast::<A, B>` or an expected type)");
                    return None;
                };
                let to = self.value_type(file, span, Some(to))?;
                self.apply(expr, &[from.clone()], args)?;
                self.same_width(file, span, &from, &to)?;
                to
            }
            Intrinsic::Logic => {
                let value = self.value_type(file, span, written.first().cloned().or(expected))?;
                self.apply(expr, &[Ty::Str], args)?;
                self.bit_pattern(file, &value, args.first()?)?;
                value
            }
            Intrinsic::MatchesBits => {
                let value = self.operand_type(file, span, written.first().cloned(), args.first())?;
                self.apply(expr, &[value.clone(), Ty::Str], args)?;
                self.bit_pattern(file, &value, args.get(1)?)?;
                Ty::Bool
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
        if !ty.has_repr() {
            let message = format!("`{}` has no `Repr`: this function of §22.1 works on a type that implements it (part I §2)", ty.name(self.design));
            self.coded(file, "E0112", span, message, "no `Repr`");
            return None;
        }
        Some(ty)
    }

    /// `E0701` unless `from` and `to` have the same number of levels (§22.1.3 rule 3).
    fn same_width(&mut self, file: FileId, span: Span, from: &Ty, to: &Ty) -> Option<()> {
        let (a, b) = (from.levels(self.design)?, to.levels(self.design)?);
        if a != b {
            let message = format!("`cast` between `{}` ({a} levels) and `{}` ({b} levels): the widths differ (§22.1.3)", from.name(self.design), to.name(self.design));
            self.coded(file, "E0701", span, message, "widths differ");
            return None;
        }
        Some(())
    }

    /// `E0702` unless `arg` is a string literal of `0`, `1` and `x`, one character per level of
    /// `value` (§22.1.4 rule 3).
    fn bit_pattern(&mut self, file: FileId, value: &Ty, arg: &Arg) -> Option<()> {
        let levels = value.levels(self.design)?;
        let ExprKind::Lit(lit) = &self.design.expr_node(arg.value).kind else {
            let message = "a pattern is a string literal of `0`, `1` and `x` (§22.1.4)".to_string();
            self.coded(file, "E0702", arg.span, message, "not a pattern literal");
            return None;
        };
        let LitKind::Str { text, .. } = &lit.kind else { return None };
        if text.chars().count() != levels as usize || !text.chars().all(|c| matches!(c, '0' | '1' | 'x')) {
            let message = format!("the pattern `{text}` is not {levels} characters of `0`, `1` and `x`, one per level of `{}` (§22.1.4)", value.name(self.design));
            self.coded(file, "E0702", arg.span, message, "bad pattern");
            return None;
        }
        Some(())
    }
}
