//! The text of a `--param` of a structural param (PI-01 to PI-03, D24): the elaborator reads it by the param's type, because the
//! value selects the body's variant before any host can write it. An integer literal (decimal, `0x`, `0b`, `_`), a variant name of a
//! unit enum, `None`, `Some(value)` or a plain value of `T` for an `Option<T>`, a number, a flag.

use piperine_ir::{AggregateData, Constant, Real};

use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind};
use crate::types::Ty;

/// The integer literals of the language a text may be (Appendix A): the one reading both the elaborator and the host share.
pub struct LiteralText;

impl LiteralText {
    /// The integer a literal is, exactly: decimal, `0x` or `0b`, `_` between digits, a `-` in front; `None` for any other text
    /// (a point, an exponent, a letter).
    pub fn integer(text: &str) -> Option<i128> {
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (radix, digits) = match (unsigned.strip_prefix("0x"), unsigned.strip_prefix("0b")) {
            (Some(rest), _) => (16, rest),
            (_, Some(rest)) => (2, rest),
            _ => (10, unsigned),
        };
        if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') {
            return None;
        }
        let magnitude = i128::from_str_radix(&digits.replace('_', ""), radix).ok()?;
        Some(if negative { -magnitude } else { magnitude })
    }
}

impl Elaboration<'_, '_> {
    /// The constant the text `text` is for the param `param`, read by the param's type; the reason it is none otherwise.
    pub(super) fn constant_of_text(&mut self, param: DeclId, text: &str) -> Result<Constant, String> {
        let DeclKind::Param(declared) = &self.design.decl_data(param).kind else { return Err("it is no param".to_string()) };
        let ty = self.typed_written(declared.ty).ok_or_else(|| "its type has no text".to_string())?;
        self.constant_as(&ty, text.trim())
    }

    /// The constant `text` is as a value of `ty`.
    fn constant_as(&mut self, ty: &Ty, text: &str) -> Result<Constant, String> {
        match ty {
            Ty::F64 => text.parse::<f64>().map(|value| Constant::Real(Real::new(value))).map_err(|_| "it is not a number".to_string()),
            Ty::Bool => match text {
                "true" => Ok(Constant::Bool(true)),
                "false" => Ok(Constant::Bool(false)),
                _ => Err("a `bool` is `true` or `false`".to_string()),
            },
            Ty::UInt(_) | Ty::Int(_) => {
                let (low, high) = Self::integer_range(ty);
                let value = LiteralText::integer(text).ok_or_else(|| format!("it is not an integer literal of {}, from {low} to {high}", ty.name(self.design)))?;
                if (low..=high).contains(&value) { Ok(Constant::Int(value)) } else { Err(format!("it is outside the range of {}, from {low} to {high}", ty.name(self.design))) }
            }
            Ty::Enum(nominal) => self.variant_constant(ty, nominal.decl, text),
            Ty::Option(inner) => {
                let stored = self.stored_type(ty).ok_or_else(|| "its type has no value in the IR".to_string())?;
                let parts = match (text, text.strip_prefix("Some(").and_then(|rest| rest.strip_suffix(')'))) {
                    ("None", _) => Vec::new(),
                    (_, Some(payload)) => vec![self.constant_as(inner, payload.trim())?],
                    _ => vec![self.constant_as(inner, text)?],
                };
                if matches!(inner.as_ref(), Ty::Tuple(_) | Ty::Array(..) | Ty::Struct(_) | Ty::Option(_)) {
                    return Err("the payload is a composite: set its leaves".to_string());
                }
                Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty: stored, variant: None, parts })))
            }
            _ => Err("a composite param is set by its leaves".to_string()),
        }
    }

    /// The range of the integer type `ty`.
    fn integer_range(ty: &Ty) -> (i128, i128) {
        match ty {
            Ty::UInt(bits) => (0, (1i128 << bits) - 1),
            Ty::Int(bits) => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
            _ => (0, 0),
        }
    }

    /// The unit variant of the enum `decl` the text names, bare or as `Enum::Variant`; the variants listed when it names none.
    fn variant_constant(&mut self, ty: &Ty, decl: DeclId, text: &str) -> Result<Constant, String> {
        let variants: Vec<(String, bool)> = self
            .design
            .decl_data(decl)
            .children
            .iter()
            .filter_map(|child| match &self.design.decl_data(*child).kind {
                DeclKind::Variant(variant) => Some((self.design.decl_data(*child).name.as_ref().map_or_else(String::new, |name| name.text.clone()), variant.form == crate::pom::FieldsForm::Unit)),
                _ => None,
            })
            .collect();
        let enum_name = self.design.decl_data(decl).name.as_ref().map_or("", |name| name.text.as_str()).to_string();
        let wanted = text.strip_prefix(&format!("{enum_name}::")).unwrap_or(text);
        match variants.iter().position(|(name, unit)| name == wanted && *unit) {
            Some(position) => {
                let stored = self.stored_type(ty).ok_or_else(|| "its type has no value in the IR".to_string())?;
                Ok(Constant::Aggregate(self.ir.aggregate(AggregateData { ty: stored, variant: Some(position as u32), parts: Vec::new() })))
            }
            None => {
                let names: Vec<String> = variants.iter().filter(|(_, unit)| *unit).map(|(name, _)| format!("`{name}`")).collect();
                Err(format!("`{enum_name}` has no such variant: the variants are {}", names.join(", ")))
            }
        }
    }
}
