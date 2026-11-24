//! `ParamText`: a param value a host gives as text, read by the param's type (PI-01 to PI-03, design
//! D24). `--param Top.db.n=6` and `Session::set_text` carry the text; the type of the param decides
//! what it means: an integer literal of the type and range (decimal, `0x`, `0b`, `_` between
//! digits), the bare name of a variant of a unit enum, `None` or a value of `T` for an `Option<T>`, the
//! four-state pattern of §22.1.4 (`10xz`) for a `Logic<N>`.
//! Anything else is `E0802` naming the param and the value, and for an enum listing its variants.

use piperine_ir::{Ir, ValueType};
use piperine_lang::elab::LiteralText;

use crate::composite::CellShape;
use crate::failure::Failure;
use crate::leaf_input::LeafInput;
use crate::logic_pattern::LogicPattern;
use crate::number_cell::{FloatCell, FloatFormat, NumberCell, WideCell};
use crate::session::ParamPath;

/// One value a text gives: a real, or an integer held exactly (never through an `f64`, PI-01 AC 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scalar {
    /// A real, a flag as 0 or 1.
    Real(f64),
    /// An integer of the param's type, or the tag of a unit enum's variant.
    Integer(i128),
}

impl Scalar {
    /// The value as an `f64` when it is exact in one: a real, or an integer of at most 2^53.
    pub fn exact_real(&self) -> Option<f64> {
        match *self {
            Scalar::Real(value) => Some(value),
            Scalar::Integer(value) if value.unsigned_abs() <= 1 << 53 => Some(value as f64),
            Scalar::Integer(_) => None,
        }
    }
}

/// What a text reads as for the param it is given to.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamText {
    /// A param of one value: a number, an integer, a flag as 0 or 1, or an enum's tag.
    Whole(Scalar),
    /// A param of a number width (`f32`, `f16`, `f128`, an integer wider than 64 bits): the exact bytes of its pin.
    Exact {
        /// The pin's bytes, the value read from the text in its own format.
        bytes: Vec<u8>,
    },
    /// An `Option` param: `None` (`some` false), or `Some` of the payload.
    Option {
        /// Whether the param holds a value.
        some: bool,
        /// The payload, zero for `None`.
        value: Scalar,
    },
    /// A `Logic<N>` param: the exact bytes of its pin, the planes `binary` and `value` its pattern lays out (`abi.md` P3), at any
    /// `N`.
    Pattern {
        /// The pin's bytes: the plane `binary` (1 where a level is 0 or 1), then the plane `value` (the level where binary, else
        /// X as 0 and Z as 1).
        bytes: Vec<u8>,
    },
}

/// The reading of one text, with the names the refusal uses.
struct Reading<'a> {
    /// The design, whose arena names the variants of an enum.
    ir: &'a Ir,
    /// The param as the refusal names it, `<instance>.<param>`.
    label: String,
    /// The text, trimmed.
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
            number if NumberCell::is_number_width(number) => reading.exact(number).map(|bytes| ParamText::Exact { bytes }),
            ValueType::Option(payload) => reading.option(payload),
            other if CellShape::of(ir, other).and_then(|shape| LogicPattern::planes_of(&shape)).is_some() => reading.pattern(other),
            other if ir.is_composite_value(other) => Err(reading.refused("a composite param, an enum with fields included, is set by its leaves, `<param>.<leaf>`".to_string())),
            other => reading.scalar(other).map(ParamText::Whole),
        }
    }

    /// The leaves of an `Option` param's pin this text writes, `some` then `value`, each exactly (an integer payload as its
    /// digits, never through an `f64`); `None` for a text that is no `Option`.
    pub fn leaves(&self) -> Option<[(&'static str, LeafInput); 2]> {
        let ParamText::Option { some, value } = self else { return None };
        let payload = match value {
            Scalar::Real(real) => LeafInput::Number(*real),
            Scalar::Integer(integer) => LeafInput::Text(integer.to_string()),
        };
        Some([("some", LeafInput::Number(f64::from(u8::from(*some)))), ("value", payload)])
    }
}

impl Reading<'_> {
    /// The text as the four-state pattern of a `Logic<N>` param `ty` (§22.1.4): `E0702` for a character outside `0 1 x z` or a
    /// length other than `N`.
    fn pattern(&self, ty: &ValueType) -> Result<ParamText, Failure> {
        let shape = CellShape::of(self.ir, ty).and_then(|shape| LogicPattern::planes_of(&shape));
        let Some((width, plane)) = shape else { return Err(self.refused(format!("{ty} is no `Logic<N>`"))) };
        Ok(ParamText::Pattern { bytes: LogicPattern::parse(self.text, width, &format!("`{}`", self.label))?.bytes(plane) })
    }

    /// The pin bytes of the text as a value of the number-width type `ty`, read in its own format (A.8 rule 8; an integer by
    /// its digits, `0x`, `0o` or `0b`), never through an `f64`.
    fn exact(&self, ty: &ValueType) -> Result<Vec<u8>, Failure> {
        if let Some(format) = FloatFormat::of(ty) {
            let cell = FloatCell::parse(format, self.text).ok_or_else(|| self.refused(format!("it is not a number of type {ty}")))?;
            let infinite = format.binary().classify(cell.bits) == piperine_numbers::format::FloatClass::Infinite;
            let spelled_infinite = matches!(self.text.trim_start_matches(['+', '-']).to_ascii_lowercase().as_str(), "inf" | "infinity");
            if infinite && !spelled_infinite {
                return Err(self.refused(format!("it is outside the range of {ty}")));
            }
            return Ok(cell.to_bytes());
        }
        let (signed, width) = match ty {
            ValueType::UInt(width) => (false, *width),
            ValueType::Int(width) => (true, *width),
            other => return Err(self.refused(format!("a value of type {other} cannot be given as text"))),
        };
        WideCell::parse(signed, width, self.text).map(|cell| cell.to_bytes()).ok_or_else(|| self.refused(format!("it is not an integer literal of {ty} in its range")))
    }

    /// The refusal of this text, `E0802` naming the param and the value.
    fn refused(&self, why: String) -> Failure {
        Failure::Host { code: "E0802", message: format!("`{}` refuses the value `{}`: {why}", self.label, self.text) }
    }

    /// The text as a value of the one-valued type `ty`.
    fn scalar(&self, ty: &ValueType) -> Result<Scalar, Failure> {
        match ty {
            ValueType::F64 => self.text.parse::<f64>().map(Scalar::Real).map_err(|_| self.refused("it is not a number".to_string())),
            ValueType::Bool => match self.text {
                "true" => Ok(Scalar::Real(1.0)),
                "false" => Ok(Scalar::Real(0.0)),
                _ => Err(self.refused("a `bool` is `true` or `false`".to_string())),
            },
            integer if integer.is_integer() => self.integer(integer).map(Scalar::Integer),
            ValueType::Enum(_) => self.variant(ty).map(Scalar::Integer),
            other => Err(self.refused(format!("a value of type {other} cannot be given as text"))),
        }
    }

    /// An integer literal of `ty`: decimal, `0x` or `0b`, `_` between digits, a `-` for a signed type.
    fn integer(&self, ty: &ValueType) -> Result<i128, Failure> {
        let (low, high) = ty.integer_range().ok_or_else(|| self.refused(format!("{ty} is no integer type")))?;
        let value = LiteralText::integer(self.text).ok_or_else(|| self.refused(format!("it is not an integer literal of {ty}, from {low} to {high}")))?;
        if !(low..=high).contains(&value) {
            return Err(self.refused(format!("it is outside the range of {ty}, from {low} to {high}")));
        }
        Ok(value)
    }

    /// The tag of the variant of the unit enum `ty` the text names, bare or as `Enum::Variant`.
    fn variant(&self, ty: &ValueType) -> Result<i128, Failure> {
        let Some(CellShape::Enum { name, variants, .. }) = CellShape::of(self.ir, ty) else {
            return Err(self.refused("it is no enum".to_string()));
        };
        let wanted = self.text.strip_prefix(&format!("{name}::")).unwrap_or(self.text);
        match variants.iter().find(|variant| variant.name == wanted) {
            Some(variant) if variant.fields.is_empty() => Ok(variant.discriminant),
            _ => {
                let names: Vec<String> = variants.iter().filter(|variant| variant.fields.is_empty()).map(|variant| format!("`{}`", variant.name)).collect();
                Err(self.refused(format!("`{name}` has no such variant: the variants are {}", names.join(", "))))
            }
        }
    }

    /// `None`, or a value of the payload type, bare or as `Some(value)`.
    fn option(&self, payload: &ValueType) -> Result<ParamText, Failure> {
        if self.text == "None" {
            return Ok(ParamText::Option { some: false, value: Scalar::Real(0.0) });
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

    /// The readings of a text for a one-param design: the owner of the helpers the tests share.
    struct Texts;

    impl Texts {
        /// A design of `Top` with one param `p` of type `ty`.
        fn design(ty: ValueType) -> Ir {
            let mut builder = IrBuilder::new();
            let origin = Origin { file: "t.msl".into(), line: 1, col: 1 };
            let param = Param { name: "p".into(), ty, default: Some(Constant::Real(Real::new(0.0))), check: None, structural: false, origin: origin.clone() };
            let body = builder.body(BodySpec { name: "Top".into(), origin: origin.clone(), params: vec![param], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
            let top = builder.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![Constant::Int(0)], origin });
            builder.finish(top)
        }

        /// `text` read as a value of the param `p` of type `ty`.
        fn read(ty: ValueType, text: &str) -> Result<ParamText, Failure> {
            ParamText::read(&Texts::design(ty), &ParamPath { instance: "Top".into(), param: "p".into() }, text)
        }
    }

    #[test]
    fn an_integer_literal_is_read_in_its_radix_and_range() {
        // D24: decimal, `0x`, `0b` and `_` read; the range of the type decides.
        assert!(Texts::read(ValueType::UInt(8), "0x_ff").is_err(), "a `_` first among the digits is no literal");
        for (text, wanted) in [("6", 6), ("0x1F", 31), ("0b101", 5), ("1_000", 1000)] {
            assert_eq!(Texts::read(ValueType::UInt(16), text).expect(text), ParamText::Whole(Scalar::Integer(wanted)), "{text}");
        }
        assert_eq!(Texts::read(ValueType::Int(8), "-128").expect("the lowest i8"), ParamText::Whole(Scalar::Integer(-128)));
        for refused in ["256", "1.5", "1e1", "-1", "six"] {
            assert!(matches!(Texts::read(ValueType::UInt(8), refused), Err(Failure::Host { code: "E0802", .. })), "{refused}");
        }
    }

    #[test]
    fn an_integer_above_2_pow_53_is_read_exactly() {
        // PI-01 AC 4, D24: the literal is the value; no `f64` round trip rounds it.
        let wanted = 9_007_199_254_740_993_i128;
        assert_eq!(Texts::read(ValueType::UInt(64), "9007199254740993").expect("in range"), ParamText::Whole(Scalar::Integer(wanted)));
        assert_eq!(Scalar::Integer(wanted).exact_real(), None, "an `f64` cannot hold it");
        assert_eq!(Texts::read(ValueType::UInt(64), "18446744073709551615").expect("the highest u64"), ParamText::Whole(Scalar::Integer(i128::from(u64::MAX))));
    }
}
