//! A composite value as a host reads it (composite-values CV-21, CV-22; Q2 (c), D13): the bytes of
//! a carried tuple, array, struct, enum or `Option` decoded with the IR's type arena into a
//! `CompositeCell`, which renders as the language's literal syntax, as nested JSON and as dotted
//! leaves, and names an enum's variant (the variant names live in the arena, never in the net's
//! bytes). A pin of such a value is the bytes of its parts one after another and nothing else
//! (`piperine_plan::CompositeLayout`); `CellShape` is the type read once from the arena, so a
//! decoder needs no `Ir`.

use piperine_ir::{FieldsForm, Ir, TypeKind, ValueType};

use crate::failure::Failure;
use crate::table::Cell;

/// The type of a composite value as a decoder reads it: the arena's names and layout, copied.
#[derive(Debug, Clone, PartialEq)]
pub enum CellShape {
    /// `bool`, `f64`, `uN` or `iN`.
    Scalar(ValueType),
    /// A struct and its fields.
    Struct {
        /// The source name.
        name: String,
        /// How the fields are written.
        form: FieldsForm,
        /// The fields, in order.
        fields: Vec<(String, CellShape)>,
    },
    /// A tuple.
    Tuple(Vec<CellShape>),
    /// An array: the element and the length.
    Array(Box<CellShape>, usize),
    /// An enum: the width of its tag in bytes and its variants.
    Enum {
        /// The source name.
        name: String,
        /// The bytes of the tag, 0 for a one-variant enum (§2 rule 5).
        tag_width: usize,
        /// The variants.
        variants: Vec<VariantShape>,
    },
    /// An `Option`: a presence byte, then the payload.
    Option(Box<CellShape>),
}

/// One variant of an enum shape.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantShape {
    /// The variant's name.
    pub name: String,
    /// The tag value that names it.
    pub discriminant: i128,
    /// How its fields are written.
    pub form: FieldsForm,
    /// Its fields, in order.
    pub fields: Vec<(String, CellShape)>,
}

impl CellShape {
    /// The shape of a pin holding `ty` in `ir`; `None` for a type no pin holds.
    pub fn of(ir: &Ir, ty: &ValueType) -> Option<CellShape> {
        match ty {
            ValueType::Bool | ValueType::F64 | ValueType::UInt(_) | ValueType::Int(_) => Some(CellShape::Scalar(ty.clone())),
            ValueType::Tuple(parts) => parts.iter().map(|part| CellShape::of(ir, part)).collect::<Option<Vec<_>>>().map(CellShape::Tuple),
            ValueType::Array(element, length) => Some(CellShape::Array(Box::new(CellShape::of(ir, element)?), usize::try_from(*length).ok()?)),
            ValueType::Option(inner) => Some(CellShape::Option(Box::new(CellShape::of(ir, inner)?))),
            ValueType::Struct(id) => {
                let data = ir.type_data(*id);
                let TypeKind::Struct { form, fields } = &data.kind else { return None };
                Some(CellShape::Struct { name: data.name.clone(), form: *form, fields: CellShape::fields(ir, fields)? })
            }
            ValueType::Enum(id) => {
                let data = ir.type_data(*id);
                let TypeKind::Enum { tag_levels, variants } = &data.kind else { return None };
                let variants = variants
                    .iter()
                    .map(|v| Some(VariantShape { name: v.name.clone(), discriminant: v.discriminant, form: v.form, fields: CellShape::fields(ir, &v.fields)? }))
                    .collect::<Option<Vec<_>>>()?;
                Some(CellShape::Enum { name: data.name.clone(), tag_width: Tag::width(*tag_levels), variants })
            }
            ValueType::Unit | ValueType::Fn(..) => None,
        }
    }

    /// The named parts of a struct or a variant as shapes.
    fn fields(ir: &Ir, fields: &[(String, ValueType)]) -> Option<Vec<(String, CellShape)>> {
        fields.iter().map(|(name, ty)| Some((name.clone(), CellShape::of(ir, ty)?))).collect()
    }

    /// Whether this is an enum no variant of which has a field: its value is its tag.
    pub fn is_unit_enum(&self) -> bool {
        matches!(self, CellShape::Enum { variants, .. } if variants.iter().all(|v| v.fields.is_empty()))
    }

    /// Whether a value of this shape can have the leaf `path` (named as `CompositeCell::leaves` names it): the
    /// payload leaf of a variant is one of the shape although the value holds another variant, and so is the
    /// `value` of an `Option` that holds nothing.
    pub fn can_hold_leaf(&self, path: &str) -> bool {
        let mut paths = Vec::new();
        self.collect_leaf_paths("", &mut paths);
        paths.iter().any(|candidate| candidate == path)
    }

    /// Adds to `found` every leaf path of this shape, named under `prefix`.
    fn collect_leaf_paths(&self, prefix: &str, found: &mut Vec<String>) {
        let join = |part: &str| if prefix.is_empty() { part.to_string() } else if part.is_empty() { prefix.to_string() } else { format!("{prefix}.{part}") };
        match self {
            CellShape::Scalar(_) => found.push(prefix.to_string()),
            CellShape::Struct { fields, .. } => fields.iter().for_each(|(name, shape)| shape.collect_leaf_paths(&join(name), found)),
            CellShape::Tuple(parts) => parts.iter().enumerate().for_each(|(k, shape)| shape.collect_leaf_paths(&join(&k.to_string()), found)),
            CellShape::Array(element, length) => (0..*length).for_each(|k| element.collect_leaf_paths(&format!("{prefix}[{k}]"), found)),
            CellShape::Enum { variants, .. } => {
                found.push(prefix.to_string());
                for variant in variants {
                    variant.fields.iter().for_each(|(name, shape)| shape.collect_leaf_paths(&join(&format!("{}.{name}", variant.name)), found));
                }
            }
            CellShape::Option(inner) => {
                found.push(join("some"));
                inner.collect_leaf_paths(&join("value"), found);
            }
        }
    }

    /// The width of the value bytes of a value of this shape, which is the size of its pin.
    pub fn width(&self) -> usize {
        match self {
            CellShape::Scalar(ValueType::Bool) => 1,
            CellShape::Scalar(ValueType::F64) => 8,
            CellShape::Scalar(ValueType::UInt(n) | ValueType::Int(n)) => Tag::width(*n),
            CellShape::Scalar(_) => 0,
            CellShape::Struct { fields, .. } => fields.iter().map(|(_, shape)| shape.width()).sum(),
            CellShape::Tuple(parts) => parts.iter().map(CellShape::width).sum(),
            CellShape::Array(element, length) => element.width() * length,
            CellShape::Enum { tag_width, variants, .. } => tag_width + variants.iter().map(|v| v.fields.iter().map(|(_, shape)| shape.width()).sum::<usize>()).max().unwrap_or(0),
            CellShape::Option(inner) => 1 + inner.width(),
        }
    }

    /// The value of a pin's `bytes`: the variant's bare name for an enum with no payload, a
    /// `Cell::Composite` for any other (an `Option` is `None` or `Some` of its payload). A length
    /// shorter than the value, or a tag that names no variant, is a `Failure`, never a guess.
    pub fn decode(&self, bytes: &[u8]) -> Result<Cell, Failure> {
        let width = self.width();
        if bytes.len() < width {
            return Err(Failure::Backend(format!("cannot decode a composite value of {width} bytes from {} bytes", bytes.len())));
        }
        let mut at = 0;
        let Some(read) = self.read(bytes, &mut at) else {
            return Err(Failure::Backend(format!("cannot decode a composite value from {bytes:?}: an enum's tag names no variant")));
        };
        Ok(match (&read, self.is_unit_enum()) {
            (CompositeCell::Variant { variant, .. }, true) => Cell::Text(variant.clone()),
            (CompositeCell::Scalar(cell), _) => cell.clone(),
            _ => Cell::Composite(Box::new(read)),
        })
    }

    /// The value at `*at`, moving `*at` past it; `None` for a tag that names no variant.
    fn read(&self, bytes: &[u8], at: &mut usize) -> Option<CompositeCell> {
        match self {
            CellShape::Scalar(ty) => Some(CompositeCell::Scalar(Self::scalar(ty, bytes, at))),
            CellShape::Struct { name, form, fields } => Some(CompositeCell::Struct { name: name.clone(), form: *form, fields: Self::read_fields(fields, bytes, at)? }),
            CellShape::Tuple(parts) => parts.iter().map(|part| part.read(bytes, at)).collect::<Option<Vec<_>>>().map(CompositeCell::Tuple),
            CellShape::Array(element, length) => (0..*length).map(|_| element.read(bytes, at)).collect::<Option<Vec<_>>>().map(CompositeCell::Array),
            CellShape::Option(inner) => {
                let present = bytes[*at] != 0;
                let start = *at;
                *at += 1;
                let payload = inner.read(bytes, at);
                *at = start + 1 + inner.width();
                Some(CompositeCell::Option(if present { Some(Box::new(payload?)) } else { None }))
            }
            CellShape::Enum { name, tag_width, variants } => {
                let start = *at;
                let tag = Tag::read(&bytes[start..start + tag_width]);
                *at = start + tag_width;
                let variant = match variants.iter().find(|v| v.discriminant == tag) {
                    Some(variant) => variant,
                    // a one-variant enum has no tag: its only variant is the value
                    None if *tag_width == 0 && variants.len() == 1 => &variants[0],
                    None => return None,
                };
                let fields = Self::read_fields(&variant.fields, bytes, at)?;
                *at = start + self.width();
                Some(CompositeCell::Variant { enum_name: name.clone(), variant: variant.name.clone(), form: variant.form, fields })
            }
        }
    }

    /// The fields of a struct or a variant read in order.
    fn read_fields(fields: &[(String, CellShape)], bytes: &[u8], at: &mut usize) -> Option<Vec<(String, CompositeCell)>> {
        fields.iter().map(|(name, shape)| Some((name.clone(), shape.read(bytes, at)?))).collect()
    }

    /// The scalar of type `ty` at `*at`, moving `*at` past it.
    fn scalar(ty: &ValueType, bytes: &[u8], at: &mut usize) -> Cell {
        let width = match ty {
            ValueType::Bool => 1,
            ValueType::F64 => 8,
            ValueType::UInt(n) | ValueType::Int(n) => Tag::width(*n),
            _ => 0,
        };
        let mut word = [0u8; 8];
        word[..width].copy_from_slice(&bytes[*at..*at + width]);
        *at += width;
        let raw = u64::from_le_bytes(word);
        match ty {
            ValueType::Bool => Cell::Bool(raw != 0),
            ValueType::F64 => Cell::Real(f64::from_bits(raw)),
            ValueType::Int(_) => {
                let shift = 64 - 8 * width as u32;
                Cell::Int(i128::from(((raw << shift) as i64) >> shift))
            }
            _ => Cell::Int(i128::from(raw)),
        }
    }
}

/// The widths of a tag or an integer and a tag's value.
struct Tag;

impl Tag {
    /// The bytes of a tag or an integer of `levels` levels: 0, 1, 2, 4 or 8.
    fn width(levels: u32) -> usize {
        match levels {
            0 => 0,
            1..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The tag in `bytes`, little-endian.
    fn read(bytes: &[u8]) -> i128 {
        let mut word = [0u8; 8];
        word[..bytes.len()].copy_from_slice(bytes);
        i128::from(u64::from_le_bytes(word))
    }
}

/// A composite value read from a net, a record or a param: the tree of its parts, with the names of
/// its types and of an enum's variant.
#[derive(Debug, Clone, PartialEq)]
pub enum CompositeCell {
    /// A scalar part: `Cell::Real`, `Cell::Bool` or `Cell::Int`.
    Scalar(Cell),
    /// A struct and its fields.
    Struct {
        /// The source name.
        name: String,
        /// How the fields are written.
        form: FieldsForm,
        /// The fields, in order.
        fields: Vec<(String, CompositeCell)>,
    },
    /// A tuple.
    Tuple(Vec<CompositeCell>),
    /// An array.
    Array(Vec<CompositeCell>),
    /// The variant an enum value is, and its fields.
    Variant {
        /// The enum's name.
        enum_name: String,
        /// The variant's name.
        variant: String,
        /// How the fields are written.
        form: FieldsForm,
        /// The fields, in order.
        fields: Vec<(String, CompositeCell)>,
    },
    /// An `Option`: `None`, or `Some` of a value.
    Option(Option<Box<CompositeCell>>),
}

impl CompositeCell {
    /// The value in the language's literal syntax (`Pair { a: 7, b: true }`, `Wrap(1, 2)`,
    /// `[1, 2, 3]`, `(9, true)`, `Tx::Data { n: 5 }`, `Some(4)`), each scalar written by `scalar`.
    pub fn literal(&self, scalar: &dyn Fn(&Cell) -> String) -> String {
        match self {
            CompositeCell::Scalar(cell) => scalar(cell),
            CompositeCell::Struct { name, form, fields } => format!("{name}{}", Self::literal_fields(*form, fields, scalar)),
            CompositeCell::Tuple(parts) => {
                let parts: Vec<String> = parts.iter().map(|part| part.literal(scalar)).collect();
                if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) }
            }
            CompositeCell::Array(parts) => format!("[{}]", parts.iter().map(|part| part.literal(scalar)).collect::<Vec<_>>().join(", ")),
            CompositeCell::Variant { enum_name, variant, form, fields } => format!("{enum_name}::{variant}{}", Self::literal_fields(*form, fields, scalar)),
            CompositeCell::Option(Some(inner)) => format!("Some({})", inner.literal(scalar)),
            CompositeCell::Option(None) => "None".to_string(),
        }
    }

    /// The fields of a struct or variant as the literal writes them after its name.
    fn literal_fields(form: FieldsForm, fields: &[(String, CompositeCell)], scalar: &dyn Fn(&Cell) -> String) -> String {
        match form {
            FieldsForm::Unit => String::new(),
            FieldsForm::Tuple => format!("({})", fields.iter().map(|(_, part)| part.literal(scalar)).collect::<Vec<_>>().join(", ")),
            FieldsForm::Named => format!(" {{ {} }}", fields.iter().map(|(name, part)| format!("{name}: {}", part.literal(scalar))).collect::<Vec<_>>().join(", ")),
        }
    }

    /// The value as nested JSON (D13): a named struct an object, a tuple, an array and a tuple struct
    /// an array, a payload variant `{"Variant": ..}`, a unit variant its name as a string, `None`
    /// `null`, each scalar written by `scalar` and each name by `string`.
    pub fn json(&self, scalar: &dyn Fn(&Cell) -> String, string: &dyn Fn(&str) -> String) -> String {
        match self {
            CompositeCell::Scalar(cell) => scalar(cell),
            CompositeCell::Struct { form, fields, .. } => Self::json_fields(*form, fields, scalar, string),
            CompositeCell::Tuple(parts) | CompositeCell::Array(parts) => format!("[{}]", parts.iter().map(|part| part.json(scalar, string)).collect::<Vec<_>>().join(",")),
            CompositeCell::Variant { variant, form: FieldsForm::Unit, .. } => string(variant),
            CompositeCell::Variant { variant, form, fields, .. } => format!("{{{}:{}}}", string(variant), Self::json_fields(*form, fields, scalar, string)),
            CompositeCell::Option(Some(inner)) => inner.json(scalar, string),
            CompositeCell::Option(None) => "null".to_string(),
        }
    }

    /// The fields of a struct or variant as an object (named) or an array (positional).
    fn json_fields(form: FieldsForm, fields: &[(String, CompositeCell)], scalar: &dyn Fn(&Cell) -> String, string: &dyn Fn(&str) -> String) -> String {
        match form {
            FieldsForm::Unit => "{}".to_string(),
            FieldsForm::Tuple => format!("[{}]", fields.iter().map(|(_, part)| part.json(scalar, string)).collect::<Vec<_>>().join(",")),
            FieldsForm::Named => format!("{{{}}}", fields.iter().map(|(name, part)| format!("{}:{}", string(name), part.json(scalar, string))).collect::<Vec<_>>().join(",")),
        }
    }

    /// Every scalar leaf of the value with its dotted path from the value (`p.x`, `0`, `[2]`,
    /// `num[2]`); an enum is a leaf of its own, the variant's name under the empty path, and its
    /// payload leaves are `Variant.field`, an `Option` is `some` and `value`.
    pub fn leaves(&self) -> Vec<(String, Cell)> {
        let mut found = Vec::new();
        self.collect("", &mut found);
        found
    }

    /// Adds to `found` the leaves of this value, named under `prefix`.
    fn collect(&self, prefix: &str, found: &mut Vec<(String, Cell)>) {
        let join = |part: &str| if prefix.is_empty() { part.to_string() } else if part.is_empty() { prefix.to_string() } else { format!("{prefix}.{part}") };
        match self {
            CompositeCell::Scalar(cell) => found.push((prefix.to_string(), cell.clone())),
            CompositeCell::Struct { fields, .. } => fields.iter().for_each(|(name, part)| part.collect(&join(name), found)),
            CompositeCell::Tuple(parts) => parts.iter().enumerate().for_each(|(k, part)| part.collect(&join(&k.to_string()), found)),
            CompositeCell::Array(parts) => parts.iter().enumerate().for_each(|(k, part)| part.collect(&format!("{prefix}[{k}]"), found)),
            CompositeCell::Variant { variant, fields, .. } => {
                found.push((prefix.to_string(), Cell::Text(variant.clone())));
                fields.iter().for_each(|(name, part)| part.collect(&join(&format!("{variant}.{name}")), found));
            }
            CompositeCell::Option(inner) => {
                found.push((join("some"), Cell::Bool(inner.is_some())));
                if let Some(inner) = inner {
                    inner.collect(&join("value"), found);
                }
            }
        }
    }

    /// The leaf named `path`, `None` when the value has none.
    pub fn leaf(&self, path: &str) -> Option<Cell> {
        self.leaves().into_iter().find(|(name, _)| name == path).map(|(_, cell)| cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shape holding the bytes of `Pair { a: u8, b: bool }` and an enum `Tx { Idle, Data { n: u3 } }`.
    struct Shapes;

    impl Shapes {
        fn pair() -> CellShape {
            CellShape::Struct {
                name: "Pair".into(),
                form: FieldsForm::Named,
                fields: vec![("a".into(), CellShape::Scalar(ValueType::UInt(8))), ("b".into(), CellShape::Scalar(ValueType::Bool))],
            }
        }

        fn tx() -> CellShape {
            CellShape::Enum {
                name: "Tx".into(),
                tag_width: 1,
                variants: vec![
                    VariantShape { name: "Idle".into(), discriminant: 0, form: FieldsForm::Unit, fields: vec![] },
                    VariantShape { name: "Data".into(), discriminant: 1, form: FieldsForm::Named, fields: vec![("n".into(), CellShape::Scalar(ValueType::UInt(3)))] },
                ],
            }
        }

        fn text(cell: &Cell) -> String {
            match cell {
                Cell::Real(v) => v.to_string(),
                Cell::Bool(v) => v.to_string(),
                Cell::Int(v) => v.to_string(),
                Cell::Text(v) => v.clone(),
                Cell::Composite(_) => "composite".into(),
            }
        }
    }

    #[test]
    fn a_struct_reads_its_fields_and_prints_as_a_literal() {
        let Cell::Composite(value) = Shapes::pair().decode(&[7, 1]).expect("decodes") else { panic!("a composite") };
        assert_eq!(value.literal(&Shapes::text), "Pair { a: 7, b: true }");
        assert_eq!(value.leaves().iter().map(|(n, c)| format!("{n}={}", Shapes::text(c))).collect::<Vec<_>>(), ["a=7", "b=true"]);
        assert_eq!(value.leaf("b"), Some(Cell::Bool(true)));
        assert_eq!(value.leaf("zz"), None);
        let Cell::Composite(zero) = Shapes::pair().decode(&[0, 0]).expect("decodes") else { panic!("a composite") };
        assert_eq!(zero.literal(&Shapes::text), "Pair { a: 0, b: false }", "all-zero bytes are a value, not an unknown");
    }

    #[test]
    fn an_enum_reads_its_variant_name_and_a_tag_no_variant_names_is_a_failure() {
        let Cell::Composite(value) = Shapes::tx().decode(&[1, 5]).expect("decodes") else { panic!("a composite") };
        assert_eq!(value.literal(&Shapes::text), "Tx::Data { n: 5 }");
        assert_eq!(value.leaves().iter().map(|(n, c)| format!("{n}={}", Shapes::text(c))).collect::<Vec<_>>(), ["=Data", "Data.n=5"]);
        assert_eq!(value.json(&Shapes::text, &|s| format!("\"{s}\"")), "{\"Data\":{\"n\":5}}");
        assert!(Shapes::tx().decode(&[2, 0]).is_err(), "tag 2 names no variant: a failure, never a guess");
        let units = CellShape::Enum {
            name: "Mode".into(),
            tag_width: 1,
            variants: vec![VariantShape { name: "Off".into(), discriminant: 0, form: FieldsForm::Unit, fields: vec![] }, VariantShape { name: "Run".into(), discriminant: 2, form: FieldsForm::Unit, fields: vec![] }],
        };
        assert_eq!(units.decode(&[2]).expect("decodes"), Cell::Text("Run".into()), "an enum with no payload reads as its variant's name");
    }

    #[test]
    fn a_shape_holds_the_payload_leaf_of_a_variant_the_value_does_not_hold() {
        let Cell::Composite(idle) = Shapes::tx().decode(&[0, 0]).expect("decodes") else { panic!("a composite") };
        assert_eq!(idle.leaf("Data.n"), None, "the value holds Idle");
        assert!(Shapes::tx().can_hold_leaf("Data.n"), "the shape holds the leaf of Data");
        assert!(!Shapes::tx().can_hold_leaf("Data.m"), "no variant has an `m`");
        assert!(Shapes::pair().can_hold_leaf("b") && !Shapes::pair().can_hold_leaf("c"));
    }
}
