//! The number widths in the checker (composite-values phase D, CV-29 to CV-33): the float type a
//! literal's text names, a float literal typed by its position (§1.4.2), and the range of a float
//! literal in its own format (A.2 rule 8 read per format, `E0909`).

use piperine_numbers::decimal::DecimalText;
use piperine_numbers::format::{BinaryFormat, FloatClass};

use crate::msl::lexer;
use crate::msl::{Diagnostic, Span};
use crate::pom::{ExprId, ExprKind, FileId, LitKind};

use super::Ty;
use super::check::Checker;

/// The text of a literal that is spelled as an integer and carries a float suffix (`2f32`, `2f64`).
pub struct FloatText;

impl FloatText {
    /// The float type the suffix of `text` names, `None` for a text with no float suffix.
    pub fn suffix(text: &str) -> Option<Ty> {
        if text.ends_with("f64") {
            Some(Ty::F64)
        } else if text.ends_with("f32") {
            Some(Ty::F32)
        } else {
            None
        }
    }

    /// Whether `text` is an integer-spelled literal with a float suffix.
    pub fn is(text: &str) -> bool {
        Self::suffix(text).is_some()
    }

    /// The decimal text of a float literal without its type suffix, `_` separators kept for the reader.
    pub fn digits(text: &str) -> &str {
        let trimmed = text.trim_end_matches("f64").trim_end_matches("f32");
        trimmed.strip_suffix('_').unwrap_or(trimmed)
    }

    /// The format a float type is read in; `None` for a type that is no float.
    pub fn format(ty: &Ty) -> Option<BinaryFormat> {
        match ty {
            Ty::F16 => Some(BinaryFormat::HALF),
            Ty::F32 => Some(BinaryFormat::SINGLE),
            Ty::F64 => Some(BinaryFormat::DOUBLE),
            Ty::F128 => Some(BinaryFormat::QUAD),
            _ => None,
        }
    }
}

impl Checker<'_> {
    /// Whether `expr` is a float literal with no type suffix (`1.5`, `1e3`): it takes the float type
    /// its position requires (§1.4.2), `f64` when none.
    pub(super) fn bare_float_literal(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.bare_float_literal(*inner),
            ExprKind::Lit(lit) => matches!(&lit.kind, LitKind::Float { raw, .. } if lexer::float_suffix(raw).is_empty()),
            _ => false,
        }
    }

    /// A float literal typed `ty`: `E0909` when its value read in `ty`'s own format is infinite (A.2 rule 8
    /// per format, QD5). `raw` is the literal's text as written, suffix included.
    pub(super) fn check_float_literal(&mut self, file: FileId, span: Span, raw: &str, ty: &Ty) {
        let Some(format) = FloatText::format(ty) else { return };
        let digits = FloatText::digits(raw);
        let Some(pattern) = DecimalText::nearest(digits, format) else { return };
        if format.classify(pattern) == FloatClass::Infinite {
            let name = ty.name(self.design);
            let message = format!("`{raw}` is out of range: a float literal beyond the range of `{name}`");
            self.report(file, Diagnostic::new("E0909", message, span, "literal out of range").with_help("write a value the type holds, or a wider type"));
        }
    }
}
