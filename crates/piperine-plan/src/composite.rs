//! The ABI layout of an IR aggregate type (composite-values CV-17, CV-20; spec §2).
//!
//! A pin holding a composite is one blob: the value bytes of its parts one after another, the
//! first lowest and unpadded (what `rt::flag::Carried` writes), then one known byte for the whole
//! value (one flag per value, G6/S6). The layout lists every scalar leaf of the value as a field
//! at its byte offset, named by its dotted path from the value (`p.x`, `0`, `Data.n`, `tag`),
//! and every field's known byte is that one byte, so a value reads known only when its flag is
//! set (P3: an unknown value is all zero). An array has one field per element (`num[2]`) because
//! one known byte cannot stand for the `count` known bytes of a counted field. A struct, tuple
//! or array of nothing has no field and a size of 1 (the known byte). An enum lays its tag first
//! (`tag`, none for a one-variant enum: §2 rule 5), then the widest payload, each variant's
//! fields overlapping at the payload's start as `Variant.field`; an `Option<T>` is a presence
//! byte `some` then `T`'s leaves as `value`. The layout is a function of the IR type alone, so
//! two pins of one net get equal layouts (abi.md P4).

use piperine_abi::raw::{Field, Str, field_flag, leaf};
use piperine_ir::{Constant, Ir, TypeKind, ValueType};

/// One scalar leaf of a composite value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeLeaf {
    /// The dotted path from the value to the leaf: `p.x`, `0`, `Data.n`, `num[2]`; empty when
    /// the value is itself the leaf.
    pub name: String,
    /// The `leaf::*` constant of the scalar.
    pub leaf: u32,
    /// The byte offset of the leaf's value bytes.
    pub offset: u32,
}

/// The layout of a composite pin: its leaves and the width of the value bytes before the known byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositeLayout {
    leaves: Vec<CompositeLeaf>,
    width: u32,
}

impl CompositeLayout {
    /// The layout of a pin holding `ty` in the design `ir`; `None` for a type no pin holds
    /// (a function type, a unit).
    pub fn of(ir: &Ir, ty: &ValueType) -> Option<CompositeLayout> {
        let mut walk = LeafWalk { ir, leaves: Vec::new(), at: 0 };
        walk.place(ty, "")?;
        Some(CompositeLayout { width: walk.at, leaves: walk.leaves })
    }

    /// The width in bytes of the value, before the known byte (`T::WIDTH` of the runtime's `Carried`).
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The size of the pin: the value bytes, then the known byte.
    pub fn size(&self) -> u32 {
        self.width + 1
    }

    /// The leaves, in the order of the value's bytes (an enum's payloads overlap).
    pub fn leaves(&self) -> &[CompositeLeaf] {
        &self.leaves
    }

    /// The offset of the known byte.
    pub fn known_offset(&self) -> u32 {
        self.width
    }

    /// The ABI fields of the layout, one per leaf, each holding its value bytes at `offset` and
    /// the value's known byte.
    pub fn abi_fields(&self) -> Vec<Field<'_>> {
        self.leaves
            .iter()
            .map(|l| Field { name: Str::from_str(&l.name), leaf: l.leaf, offset: l.offset, count: 1, flags: field_flag::HAS_KNOWN, known: self.width })
            .collect()
    }

    /// The canonical bytes of `value` held as `ty` (abi.md P3): the value bytes as the runtime's
    /// `Carried::encode` writes them, the known byte 1. `None` when `value` is not a value of `ty`.
    pub fn bytes(ir: &Ir, ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        let layout = Self::of(ir, ty)?;
        let mut bytes = vec![0u8; layout.size() as usize];
        let mut at = 0usize;
        ConstantBytes { ir, out: &mut bytes }.write(ty, value, &mut at)?;
        debug_assert_eq!(at as u32, layout.width, "the bytes of a constant fill the layout's value width");
        bytes[layout.width as usize] = 1;
        Some(bytes)
    }
}

/// The byte widths of tags and integers: 0, 1, 2, 4 or 8 (the runtime's `Tag` and `Word`).
struct TagWidth;

impl TagWidth {
    /// The width of the tag of an enum of `levels` tag levels.
    fn of(levels: u32) -> u32 {
        match levels {
            0 => 0,
            1..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The width in bytes of an `N`-bit integer: the narrowest of 1, 2, 4 and 8.
    fn bytes_of_bits(bits: u32) -> u32 {
        match bits {
            0..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The leaf of an unsigned integer `width` bytes wide.
    fn leaf(width: u32) -> u32 {
        match width {
            1 => leaf::U8,
            2 => leaf::U16,
            4 => leaf::U32,
            _ => leaf::U64,
        }
    }
}

/// The dotted name of a part of a value: the prefix, a dot, the part, with an empty side dropped.
struct LeafName;

impl LeafName {
    /// `prefix.part`, or whichever of the two is not empty.
    fn join(prefix: &str, part: &str) -> String {
        match (prefix.is_empty(), part.is_empty()) {
            (true, _) => part.to_string(),
            (_, true) => prefix.to_string(),
            _ => format!("{prefix}.{part}"),
        }
    }

    /// `prefix[index]`.
    fn element(prefix: &str, index: u64) -> String {
        format!("{prefix}[{index}]")
    }
}

/// The walk that places each leaf of a type at the next byte.
struct LeafWalk<'ir> {
    ir: &'ir Ir,
    leaves: Vec<CompositeLeaf>,
    /// The offset the next leaf lands on.
    at: u32,
}

impl LeafWalk<'_> {
    /// Places the leaves of `ty` named under `name`; `None` for a type no pin holds.
    fn place(&mut self, ty: &ValueType, name: &str) -> Option<()> {
        match ty {
            ValueType::Bool => self.scalar(leaf::BOOL, 1, name),
            ValueType::F64 => self.scalar(leaf::F64, 8, name),
            ValueType::UInt(n) => self.integer(*n, false, name),
            ValueType::Int(n) => self.integer(*n, true, name),
            ValueType::Tuple(items) => {
                for (k, item) in items.iter().enumerate() {
                    self.place(item, &LeafName::join(name, &k.to_string()))?;
                }
            }
            ValueType::Array(element, count) => {
                for k in 0..*count {
                    self.place(element, &LeafName::element(name, k))?;
                }
            }
            ValueType::Struct(id) => {
                let TypeKind::Struct { fields, .. } = &self.ir.type_data(*id).kind else { return None };
                for (field, field_ty) in fields {
                    self.place(field_ty, &LeafName::join(name, field))?;
                }
            }
            ValueType::Enum(id) => self.enumeration(*id, name)?,
            ValueType::Option(inner) => {
                self.scalar(leaf::U8, 1, &LeafName::join(name, "some"));
                self.place(inner, &LeafName::join(name, "value"))?;
            }
            ValueType::Unit | ValueType::Fn(..) => return None,
        }
        Some(())
    }

    /// One leaf of `size` bytes at the next offset.
    fn scalar(&mut self, leaf: u32, size: u32, name: &str) {
        self.leaves.push(CompositeLeaf { name: name.to_string(), leaf, offset: self.at });
        self.at += size;
    }

    /// An `N`-bit integer: the narrowest of 1, 2, 4 and 8 bytes, signed or not.
    fn integer(&mut self, bits: u32, signed: bool, name: &str) {
        let width = TagWidth::bytes_of_bits(bits);
        let leaf = match (signed, width) {
            (false, w) => TagWidth::leaf(w),
            (true, 1) => leaf::I8,
            (true, 2) => leaf::I16,
            (true, 4) => leaf::I32,
            (true, _) => leaf::I64,
        };
        self.scalar(leaf, width, name);
    }

    /// An enum: the tag, then the widest payload, the variants' fields overlapping.
    fn enumeration(&mut self, id: piperine_ir::TypeId, name: &str) -> Option<()> {
        let TypeKind::Enum { tag_levels, variants } = &self.ir.type_data(id).kind else { return None };
        let tag_width = TagWidth::of(*tag_levels);
        let start = self.at;
        if tag_width > 0 {
            self.scalar(TagWidth::leaf(tag_width), tag_width, &LeafName::join(name, "tag"));
        }
        let payload = self.at;
        let mut end = payload;
        let unit_only = variants.iter().all(|v| v.fields.is_empty());
        for variant in variants {
            self.at = payload;
            if unit_only {
                continue;
            }
            let variant_name = LeafName::join(name, &variant.name);
            for (field, field_ty) in &variant.fields {
                self.place(field_ty, &LeafName::join(&variant_name, field))?;
            }
            end = end.max(self.at);
        }
        debug_assert!(end >= start);
        self.at = end;
        Some(())
    }
}

/// The writer of a constant's bytes, mirroring the runtime's `Carried::encode`.
struct ConstantBytes<'a> {
    ir: &'a Ir,
    out: &'a mut [u8],
}

impl ConstantBytes<'_> {
    /// Writes `value` held as `ty` at `*at` and moves `*at` past its bytes; `None` when `value`
    /// is not a value of `ty`.
    fn write(&mut self, ty: &ValueType, value: Constant, at: &mut usize) -> Option<()> {
        match (ty, value) {
            (ValueType::Bool, Constant::Bool(b)) => self.put(&[u8::from(b)], at),
            (ValueType::F64, Constant::Real(r)) => self.put(&r.get().to_le_bytes(), at),
            (ValueType::UInt(n) | ValueType::Int(n), Constant::Int(v)) => {
                let (low, high) = ty.integer_range()?;
                if v < low || v > high {
                    return None;
                }
                self.put(&v.to_le_bytes()[..TagWidth::bytes_of_bits(*n) as usize], at)
            }
            (ValueType::Tuple(items), Constant::Aggregate(id)) => self.parts(items.iter(), id, at),
            (ValueType::Array(element, count), Constant::Aggregate(id)) => self.parts(std::iter::repeat_n(&**element, *count as usize), id, at),
            (ValueType::Struct(type_id), Constant::Aggregate(id)) => {
                let TypeKind::Struct { fields, .. } = &self.ir.type_data(*type_id).kind else { return None };
                self.parts(fields.iter().map(|(_, field_ty)| field_ty), id, at)
            }
            (ValueType::Enum(type_id), Constant::Int(tag)) => self.tag(*type_id, tag, at),
            (ValueType::Enum(type_id), Constant::Aggregate(id)) => self.variant(*type_id, id, at),
            (ValueType::Option(inner), Constant::Aggregate(id)) => {
                let parts = &self.ir.aggregate(id).parts;
                let start = *at;
                *at += 1 + CompositeLayout::of(self.ir, inner)?.width() as usize;
                let payload = parts.first().copied();
                if let Some(part) = payload {
                    self.out[start] = 1;
                    let mut inner_at = start + 1;
                    self.write(inner, part, &mut inner_at)?;
                }
                Some(())
            }
            _ => None,
        }
    }

    /// Copies `bytes` to `*at` and moves past them.
    fn put(&mut self, bytes: &[u8], at: &mut usize) -> Option<()> {
        self.out.get_mut(*at..*at + bytes.len())?.copy_from_slice(bytes);
        *at += bytes.len();
        Some(())
    }

    /// The parts of the aggregate `id`, each held as the next of `types`.
    fn parts<'t>(&mut self, types: impl Iterator<Item = &'t ValueType>, id: piperine_ir::AggregateId, at: &mut usize) -> Option<()> {
        let parts = self.ir.aggregate(id).parts.clone();
        let types: Vec<&ValueType> = types.collect();
        if parts.len() != types.len() {
            return None;
        }
        for (part_ty, part) in types.into_iter().zip(parts) {
            self.write(part_ty, part, at)?;
        }
        Some(())
    }

    /// An enum value that is its tag alone (no variant has a field).
    fn tag(&mut self, id: piperine_ir::TypeId, tag: i128, at: &mut usize) -> Option<()> {
        let TypeKind::Enum { tag_levels, .. } = &self.ir.type_data(id).kind else { return None };
        let width = TagWidth::of(*tag_levels) as usize;
        let start = *at;
        *at += width;
        self.out.get_mut(start..start + width)?.copy_from_slice(&tag.to_le_bytes()[..width]);
        Some(())
    }

    /// An enum value of the variant of aggregate `id`: its discriminant, then its fields.
    fn variant(&mut self, id: piperine_ir::TypeId, aggregate: piperine_ir::AggregateId, at: &mut usize) -> Option<()> {
        let data = self.ir.aggregate(aggregate).clone();
        let TypeKind::Enum { tag_levels, variants } = &self.ir.type_data(id).kind else { return None };
        let variant = variants.get(data.variant? as usize)?;
        let start = *at;
        let mut field_at = start;
        self.tag(id, variant.discriminant, &mut field_at)?;
        if data.parts.len() != variant.fields.len() {
            return None;
        }
        for ((_, field_ty), part) in variant.fields.iter().zip(data.parts) {
            self.write(field_ty, part, &mut field_at)?;
        }
        // the payload is as wide as the widest variant's: the rest stays zero
        let widest = variants.iter().map(|v| v.fields.iter().filter_map(|(_, t)| CompositeLayout::of(self.ir, t).map(|l| l.width())).sum::<u32>()).max().unwrap_or(0);
        *at = start + (TagWidth::of(*tag_levels) + widest) as usize;
        Some(())
    }
}
