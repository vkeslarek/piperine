//! `ParamText`: a param value a host gives as text, read by the param's type (PI-01 to PI-03, design
//! D24). `--param Top.db.n=6` and `Session::set_text` carry the text; the type of the param decides
//! what it means: an integer literal of the type and range (decimal, `0x`, `0b`, `_` between
//! digits), the bare name of a variant of a unit enum, `None` or a value of `T` for an `Option<T>`.
//! Anything else is `E0802` naming the param and the value, and for an enum listing its variants.

use piperine_ir::{Ir, ValueType};

use crate::composite::CellShape;
use crate::failure::Failure;
use crate::session::ParamPath;

/// What a text reads as for the param it is given to.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamText {
    /// A param of one value: a number, an integer, a flag as 0 or 1, or an enum's tag.
    Number(f64),
    /// An `Option` param: the leaves of its pin, `some` then `value`, each with the number it takes.
    Option { some: bool, value: f64 },
}

/// The reading of one text, with the names the refusal uses.
struct Reading<'a> {
    ir: &'a Ir,
    label: String,
    text: &'a str,
}

impl ParamText {
    /// `text` as a value of the param `path` of `ir`: `E0801` when the design has no such param, `E0802`
    /// when the text is no value of its type.
    pub fn read(ir: &Ir, path: &ParamPath, text: &str) -> Result<ParamText, Failure> {
        let ty = ir
            .instance_at(&path.instance)
            .and_then(|instance| instance.body().params().iter().find(|param| param.name == path.param).map(|param| param.ty.clone()))
            .ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{}` names no param of the design", path.label()) })?;
        let reading = Reading { ir, label: path.label(), text: text.trim() };
        match &ty {
            ValueType::Option(payload) => reading.option(payload),
            other if ir.is_composite_value(other) => Err(reading.refused("a composite param, an enum with fields included, is set by its leaves, `<param>.<leaf>`".to_string())),
            other => reading.scalar(other).map(ParamText::Number),
        }
    }

    /// The leaves of an `Option` param's pin this text writes, in the order of its layout.
    pub fn option_leaves(&self) -> Option<[(&'static str, f64); 2]> {
        match self {
            ParamText::Option { some, value } => Some([("some", f64::from(u8::from(*some))), ("value", *value)]),
            ParamText::Number(_) => None,
        }
    }
}

impl Reading<'_> {
    /// The refusal of this text, `E0802` naming the param and the value.
    fn refused(&self, why: String) -> Failure {
        Failure::Host { code: "E0802", message: format!("`{}` refuses the value `{}`: {why}", self.label, self.text) }
    }

    /// The text as a value of the one-valued type `ty`.
    fn scalar(&self, ty: &ValueType) -> Result<f64, Failure> {
        match ty {
            ValueType::F64 => self.text.parse::<f64>().map_err(|_| self.refused("it is not a number".to_string())),
            ValueType::Bool => match self.text {
                "true" => Ok(1.0),
                "false" => Ok(0.0),
                _ => Err(self.refused("a `bool` is `true` or `false`".to_string())),
            },
            integer if integer.is_integer() => self.integer(integer),
            ValueType::Enum(_) => self.variant(ty),
            other => Err(self.refused(format!("a value of type {other} cannot be given as text"))),
        }
    }

    /// An integer literal of `ty`: decimal, `0x` or `0b`, `_` between digits, a `-` for a signed type.
    fn integer(&self, ty: &ValueType) -> Result<f64, Failure> {
        let (low, high) = ty.integer_range().ok_or_else(|| self.refused(format!("{ty} is no integer type")))?;
        let (negative, unsigned) = match self.text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, self.text),
        };
        let (radix, digits) = match (unsigned.strip_prefix("0x"), unsigned.strip_prefix("0b")) {
            (Some(rest), _) => (16, rest),
            (_, Some(rest)) => (2, rest),
            _ => (10, unsigned),
        };
        let malformed = || self.refused(format!("it is not an integer literal of {ty}, from {low} to {high}"));
        if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') {
            return Err(malformed());
        }
        let magnitude = i128::from_str_radix(&digits.replace('_', ""), radix).map_err(|_| malformed())?;
        let value = if negative { -magnitude } else { magnitude };
        if !(low..=high).contains(&value) {
            return Err(self.refused(format!("it is outside the range of {ty}, from {low} to {high}")));
        }
        Ok(value as f64)
    }

    /// The tag of the variant of the unit enum `ty` the text names, bare or as `Enum::Variant`.
    fn variant(&self, ty: &ValueType) -> Result<f64, Failure> {
        let Some(CellShape::Enum { name, variants, .. }) = CellShape::of(self.ir, ty) else {
            return Err(self.refused("it is no enum".to_string()));
        };
        let wanted = self.text.strip_prefix(&format!("{name}::")).unwrap_or(self.text);
        match variants.iter().find(|variant| variant.name == wanted) {
            Some(variant) if variant.fields.is_empty() => Ok(variant.discriminant as f64),
            _ => {
                let names: Vec<String> = variants.iter().filter(|variant| variant.fields.is_empty()).map(|variant| format!("`{}`", variant.name)).collect();
                Err(self.refused(format!("`{name}` has no such variant: the variants are {}", names.join(", "))))
            }
        }
    }

    /// `None`, or a value of the payload type, bare or as `Some(value)`.
    fn option(&self, payload: &ValueType) -> Result<ParamText, Failure> {
        if self.text == "None" {
            return Ok(ParamText::Option { some: false, value: 0.0 });
        }
        let inner = self.text.strip_prefix("Some(").and_then(|rest| rest.strip_suffix(')')).unwrap_or(self.text);
        let reading = Reading { ir: self.ir, label: self.label.clone(), text: inner.trim() };
        if self.ir.is_composite_value(payload) {
            return Err(self.refused("the payload is a composite: set `some` and the leaves of `value`".to_string()));
        }
        reading.scalar(payload).map(|value| ParamText::Option { some: true, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_ir::{BodySpec, Constant, InstanceSpec, IrBuilder, Origin, Param, Real};

    /// A design of `Top` with one param of each type under test.
    fn design(ty: ValueType) -> Ir {
        let mut builder = IrBuilder::new();
        let origin = Origin { file: "t.msl".into(), line: 1, col: 1 };
        let param = Param { name: "p".into(), ty, default: Some(Constant::Real(Real::new(0.0))), check: None, structural: false, origin: origin.clone() };
        let body = builder.body(BodySpec { name: "Top".into(), origin: origin.clone(), params: vec![param], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = builder.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![Constant::Int(0)], origin });
        builder.finish(top)
    }

    fn read(ty: ValueType, text: &str) -> Result<ParamText, Failure> {
        ParamText::read(&design(ty), &ParamPath { instance: "Top".into(), param: "p".into() }, text)
    }

    #[test]
    fn an_integer_literal_is_read_in_its_radix_and_range() {
        // D24: decimal, `0x`, `0b` and `_` read; the range of the type decides.
        assert!(read(ValueType::UInt(8), "0x_ff").is_err(), "a `_` first among the digits is no literal");
        for (text, wanted) in [("6", 6.0), ("0x1F", 31.0), ("0b101", 5.0), ("1_000", 1000.0)] {
            assert_eq!(read(ValueType::UInt(16), text).expect(text), ParamText::Number(wanted), "{text}");
        }
        assert_eq!(read(ValueType::Int(8), "-128").expect("the lowest i8"), ParamText::Number(-128.0));
        assert!(matches!(read(ValueType::UInt(8), "256"), Err(Failure::Host { code: "E0802", .. })));
        assert!(matches!(read(ValueType::UInt(8), "1.5"), Err(Failure::Host { code: "E0802", .. })));
    }
}
