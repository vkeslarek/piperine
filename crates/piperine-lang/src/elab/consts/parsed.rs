//! `parse_table` and `parse_hex` in a const context (§22.6.4, PI-09): the text of an `include_str!`, or of a
//! `const` that holds one, is read as the array the `const`'s own type asks for (rule 2) and becomes a
//! composite of the evaluator, exactly as an array written out would.

use std::sync::Arc;

use piperine_ir::{Constant, Real};

use super::composite::CompositeKind;
use super::{Bindings, ConstEval, ConstFault, Width};
use crate::refusal::forms::ConstantForm;
use crate::elab::tables::{HexText, TableFault, TableText};
use crate::pom::{ConstData, DeclKind, ExprId, ExprKind, Resolution};
use crate::types::{Intrinsic, Length, Ty};

impl ConstEval<'_, '_> {
    /// `parse_table(text)` or `parse_hex(text)` at `expr`, the array its type says: `E0614` when the text does not
    /// fit that type, `E0706` when it is not in the format (`ConstFault::Table`).
    pub(super) fn parse_array(&mut self, expr: ExprId, intrinsic: Intrinsic, args: &[ExprId], bindings: &Bindings) -> Result<Constant, ConstFault> {
        let [text_expr] = args else {
            return Err(ConstFault::refused(expr, ConstantForm::ParserCalledWithOtherThanOneText));
        };
        let Some(Ty::Array(element, length)) = self.typed(expr) else {
            return Err(ConstFault::refused(expr, ConstantForm::ParserWhoseArrayTypeCheckerDidNot));
        };
        let (source, text) = self.text_of(*text_expr, bindings)?;
        let count = match length {
            Length::Fixed(length) => usize::try_from(length).map_err(|_| ConstFault::refused(expr, ConstantForm::ArrayTooLongToParse))?,
            Length::Generic(_) => return Err(ConstFault::refused(expr, ConstantForm::ParsedArrayWhoseLengthIsGenericArgument)),
        };
        let table = |fault: TableFault| ConstFault::Table { expr, code: fault.code, message: fault.message };
        let elements: Vec<Constant> = match (intrinsic, &*element) {
            (Intrinsic::ParseTable, Ty::F64) => TableText::new(&source, &text).rows(1, count).map_err(table)?.into_iter().map(|row| Constant::Real(Real::new(row[0]))).collect(),
            (Intrinsic::ParseTable, Ty::Tuple(parts)) => {
                let rows = TableText::new(&source, &text).rows(parts.len(), count).map_err(table)?;
                rows.into_iter().map(|row| self.make(CompositeKind::Tuple, row.into_iter().map(|value| Constant::Real(Real::new(value))).collect())).collect()
            }
            (Intrinsic::ParseHex, ty) => {
                let Some(width) = Width::of(ty) else {
                    return Err(ConstFault::refused(expr, ConstantForm::HexImageOfOtherThanIntegers));
                };
                HexText::new(&source, &text).words(count, width.bits, width.signed).map_err(table)?.into_iter().map(Constant::Int).collect()
            }
            _ => return Err(ConstFault::refused(expr, ConstantForm::ParserOverThisElementType)),
        };
        Ok(self.make(CompositeKind::Array, elements))
    }

    /// The text `expr` stands for, for a caller outside the evaluator (the control string of a `table_model`).
    pub(in crate::elab) fn text_of_expression(&mut self, expr: ExprId, bindings: &Bindings) -> Result<(String, Arc<str>), ConstFault> {
        self.text_of(expr, bindings)
    }

    /// The text `expr` stands for and how a diagnostic names it: an `include_str!` (the resolved file), a `const`
    /// that holds one, or a string literal.
    fn text_of(&mut self, expr: ExprId, bindings: &Bindings) -> Result<(String, Arc<str>), ConstFault> {
        match self.design.expr_node(expr).kind.clone() {
            ExprKind::Paren(inner) => self.text_of(inner, bindings),
            ExprKind::Lit(lit) => match lit.str_value() {
                Some(text) => Ok(("a string literal".to_string(), Arc::from(text))),
                None => Err(ConstFault::refused(expr, ConstantForm::StringWithUndefinedEscape)),
            },
            ExprKind::IncludeStr(literal) => {
                let Some(path) = literal.str_value() else {
                    return Err(ConstFault::refused(expr, ConstantForm::IncludeStrPathWithUndefinedEscape));
                };
                let owner = self.design.decl_data(self.design.expr_node(expr).owner).file;
                let source = self.design.file_data(owner).path.clone();
                let text = self.files.text(source.as_deref(), &path).map_err(|fault| ConstFault::File { expr, fault })?;
                Ok((format!("`{path}`"), text))
            }
            ExprKind::Path(_) => match self.path_target(expr) {
                Some(decl) if matches!(&self.design.decl_data(decl).kind, DeclKind::Const(ConstData { value: Some(_), .. })) => {
                    let DeclKind::Const(ConstData { value: Some(value), .. }) = &self.design.decl_data(decl).kind else {
                        return Err(ConstFault::NoValue { decl });
                    };
                    let value = *value;
                    self.text_of(value, bindings)
                }
                _ => Err(ConstFault::refused(expr, ConstantForm::TextThatIsNoIncludeStrOr)),
            },
            _ => Err(ConstFault::refused(expr, ConstantForm::TextThatIsNoIncludeStrOr)),
        }
    }

    /// The declaration a path expression names, if it names one.
    fn path_target(&self, expr: ExprId) -> Option<crate::pom::DeclId> {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) => Some(decl),
            _ => None,
        }
    }
}
