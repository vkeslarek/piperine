//! Value types, the type table, and the constants of a type.

use serde::{Deserialize, Serialize};

use crate::ids::{AggregateId, WideId, FunctionId, TypeId};
use crate::real::Real;
use crate::model::{GenericInstance, Origin};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    F64,
    Bool,
    Unit,
    Fn(Vec<ValueType>, Box<ValueType>),
    /// `UInt<N>` (`uN`): `N` bits, arithmetic modulo `2^N`.
    UInt(u32),
    /// `Int<N>` (`iN`): `N` bits, two's complement, arithmetic modulo `2^N`.
    Int(u32),
    /// A tuple `(A, B, ..)`: its fields, lowest first.
    Tuple(Vec<ValueType>),
    /// An array `[T; N]`: the element type and the length.
    Array(Box<ValueType>, u64),
    /// A source struct, named by its entry of the type arena.
    Struct(TypeId),
    /// A source enum, named by its entry of the type arena.
    Enum(TypeId),
    /// An `Option<T>` held as one value: a field, an element, an item, a carrier.
    Option(Box<ValueType>),
    /// `f32` (`Float<32>`): IEEE 754 binary32, computed as Rust's `f32`.
    F32,
    /// `f16` (`Float<16>`): IEEE 754 binary16, correctly rounded.
    F16,
    /// `f128` (`Float<128>`): IEEE 754 binary128, correctly rounded.
    F128,
}

/// How the fields of a struct or a variant are written in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldsForm {
    /// `{ x: f64, y: f64 }`: the fields have names.
    Named,
    /// `(f64, f64)`: the fields are numbered, `0`, `1`, ...
    Tuple,
    /// No fields.
    Unit,
}

/// The traits of Appendix H.8 a source type derives, one flag each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derives {
    pub clone: bool,
    pub copy: bool,
    pub partial_eq: bool,
    pub eq: bool,
    pub partial_ord: bool,
    pub ord: bool,
    pub default: bool,
    pub debug: bool,
    pub hash: bool,
    /// `Repr` (§2): the type crosses the device ABI.
    pub repr: bool,
}

/// One source struct or enum: its name, where it was written, and its fields or variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeData {
    /// The bare name, `Pair`.
    pub name: String,
    /// The path of the declaration in the source, `bus::Pair`; `std::logic::Logic` for one the standard library declares.
    pub path: String,
    pub origin: Origin,
    pub kind: TypeKind,
    pub derives: Derives,
    /// The generic item this type is one instantiation of (`Pair<f64>`); `None` for a type as
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<GenericInstance>,
    /// How the type crosses the device ABI: derived from its fields (§2) or written by the author.
    #[serde(default, skip_serializing_if = "ReprForm::is_derived")]
    pub repr: ReprForm,
}

/// Where the layout of a type that implements `Repr` comes from (§2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReprForm {
    /// `#[derive(Repr)]`: the layout follows the fields.
    #[default]
    Derived,
    /// `impl Repr for T`: `width` levels, written to levels by the function `encode` (the type to
    /// `[bool; width]`) and read back by `decode` (`[bool; width]` to `Option` of the type).
    HandWritten { width: u32, encode: FunctionId, decode: FunctionId },
}

impl ReprForm {
    /// Whether the layout is derived, which serde leaves unwritten.
    pub fn is_derived(&self) -> bool {
        matches!(self, ReprForm::Derived)
    }
}

/// The shape of a source type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeKind {
    /// A struct and its fields, named or numbered (the names of a tuple struct are `0`, `1`, ..).
    Struct { form: FieldsForm, fields: Vec<(String, ValueType)> },
    /// An enum: the width of its tag in levels (§2 rule 5) and its variants.
    Enum { tag_levels: u32, variants: Vec<VariantData> },
}

/// One variant of an enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantData {
    pub name: String,
    /// The tag value of the variant (§1.3.7).
    pub discriminant: i128,
    pub form: FieldsForm,
    pub fields: Vec<(String, ValueType)>,
}

impl ValueType {
    /// Whether this is a tuple, an array, a struct or an `Option`: a value of parts, which a
    /// param or a pin holds as bytes rather than as one scalar. An enum is not counted (its
    /// params belong to `params-and-io`).
    pub fn is_composite(&self) -> bool {
        matches!(self, ValueType::Tuple(_) | ValueType::Array(..) | ValueType::Struct(_) | ValueType::Option(_))
    }

    /// Whether this is `UInt<N>` or `Int<N>`.
    pub fn is_integer(&self) -> bool {
        matches!(self, ValueType::UInt(_) | ValueType::Int(_))
    }

    /// Whether this is a float of any width: `f64`, `f32`, `f16` or `f128`.
    pub fn is_float(&self) -> bool {
        matches!(self, ValueType::F64 | ValueType::F32 | ValueType::F16 | ValueType::F128)
    }

    /// Whether this is a float of a width other than `f64`.
    pub fn is_other_float(&self) -> bool {
        matches!(self, ValueType::F32 | ValueType::F16 | ValueType::F128)
    }

    /// The width of a float in bits (16, 32, 64, 128), `None` for any other type.
    pub fn float_width(&self) -> Option<u32> {
        match self {
            ValueType::F16 => Some(16),
            ValueType::F32 => Some(32),
            ValueType::F64 => Some(64),
            ValueType::F128 => Some(128),
            _ => None,
        }
    }

    /// Whether this is an integer type wider than 64 bits, the one form `Constant::Wide` holds.
    pub fn is_wide_integer(&self) -> bool {
        matches!(self, ValueType::UInt(n) | ValueType::Int(n) if *n > 64)
    }

    /// The range of an integer type, `None` for any other type.
    pub fn integer_range(&self) -> Option<(i128, i128)> {
        match *self {
            ValueType::UInt(n) if (1..=64).contains(&n) => Some((0, (1i128 << n) - 1)),
            ValueType::Int(n) if (1..=64).contains(&n) => Some((-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Constant {
    Real(Real),
    Bool(bool),
    /// An integer; its width and sign are the type of the param or carrier it initializes. For a type with a
    /// hand-written `Repr` it is the levels word: level `k` is bit `k`.
    Int(i128),
    /// A composite value (a tuple, an array, a struct, an enum variant, an `Option`): an entry of
    /// the arena of composite constants. The handle keeps `Constant` `Copy`.
    Aggregate(AggregateId),
    /// A float of a width other than `f64` (`f32`, `f16`, `f128`): its IEEE 754 bit pattern, read by the type
    /// of the param or carrier it initializes, as `Int` is.
    FloatBits(u128),
    /// An integer of an integer type wider than 64 bits: an entry of the arena of wide constants.
    Wide(WideId),
}

/// One constant of an integer type wider than 64 bits: its two's complement value at the type's
/// width, as `ceil(N/64)` little-endian `u64` limbs (low limb first), canonical when no bit past
/// the width is set (an `Int<N>` sign-extends to the limb boundary).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WideData {
    /// The limbs, low limb first.
    pub limbs: Vec<u64>,
}

/// One composite constant: its type and its parts in declaration order (§1.5.25). A part is
/// itself a `Constant`, so aggregates nest through `Constant::Aggregate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateData {
    /// The type of the value: a tuple, an array, a struct, an enum or an `Option`.
    pub ty: ValueType,
    /// The variant of an enum value; `None` for every other type.
    pub variant: Option<u32>,
    /// The fields, the elements, or the fields of the variant. An `Option` has none (`None`) or
    /// one (`Some`).
    pub parts: Vec<Constant>,
}
