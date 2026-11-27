//! Carriers: their layouts and their pins, the resolved and element-drive pins.

use piperine_abi::layout::{BIT, BYTE, F64, leaf_size};
use piperine_abi::raw::{Field, Layout, List, Str, leaf, pin_flag, role};
use piperine_abi::shape::{PinSpec, ShapeBuf};
use piperine_ir::{Body, CarrierRole, Constant, Ir, Origin, ValueType};

use crate::array::ArrayPin;
use crate::error::PlanError;
use crate::pin::PinLayout;
use crate::widths::NumberLayout;
use crate::digital::Digital;
use crate::plan::ResolvedPins;
use crate::resolved::DriveLayout;

/// The layout of a discrete value of a given IR type, and its bytes.
pub struct CarrierLayout;

static U16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U16)];
static U32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U32)];
static U64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::U64)];
static I8_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I8)];
static I16_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I16)];
static I32_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I32)];
static I64_FIELDS: [Field<'static>; 1] = [CarrierLayout::field(leaf::I64)];

/// `{ v: U16 @0 }`, size 2, align 2.
static U16: Layout<'static> = Layout { size: 2, align: 2, fields: List::from_slice(&U16_FIELDS) };
/// `{ v: U32 @0 }`, size 4, align 4.
static U32: Layout<'static> = Layout { size: 4, align: 4, fields: List::from_slice(&U32_FIELDS) };
/// `{ v: U64 @0 }`, size 8, align 8.
static U64: Layout<'static> = Layout { size: 8, align: 8, fields: List::from_slice(&U64_FIELDS) };
/// `{ v: I8 @0 }`, size 1, align 1.
static I8: Layout<'static> = Layout { size: 1, align: 1, fields: List::from_slice(&I8_FIELDS) };
/// `{ v: I16 @0 }`, size 2, align 2.
static I16: Layout<'static> = Layout { size: 2, align: 2, fields: List::from_slice(&I16_FIELDS) };
/// `{ v: I32 @0 }`, size 4, align 4.
static I32: Layout<'static> = Layout { size: 4, align: 4, fields: List::from_slice(&I32_FIELDS) };
/// `{ v: I64 @0 }`, size 8, align 8.
static I64: Layout<'static> = Layout { size: 8, align: 8, fields: List::from_slice(&I64_FIELDS) };

impl CarrierLayout {
    /// The one field `v` of a layout: `leaf` at offset 0.
    const fn field(leaf: u32) -> Field<'static> {
        Field { name: Str::from_static("v"), leaf, offset: 0, count: 1 }
    }

    /// The width in bytes of the value of an `N`-bit integer: the narrowest of 1, 2, 4 and 8.
    fn width(n: u32) -> u32 {
        match n {
            0..=8 => 1,
            9..=16 => 2,
            17..=32 => 4,
            _ => 8,
        }
    }

    /// The layout of a carrier (or a discrete param) holding `ty`; `None` for a type no carrier
    /// holds (`check()` refuses it on a carrier).
    pub fn of(ty: &ValueType) -> Option<&'static Layout<'static>> {
        match *ty {
            ValueType::Bool => Some(&BIT),
            ValueType::F64 => Some(&F64),
            ValueType::F16 => Some(&U16),
            ValueType::F32 | ValueType::F128 => NumberLayout::static_layout(ty),
            // an integer wider than 64 bits is a `U64` array: its layout depends on its width
            ValueType::UInt(n) | ValueType::Int(n) if n > 64 => None,
            ValueType::UInt(n) => Some(match Self::width(n) {
                1 => &BYTE,
                2 => &U16,
                4 => &U32,
                _ => &U64,
            }),
            ValueType::Int(n) => Some(match Self::width(n) {
                1 => &I8,
                2 => &I16,
                4 => &I32,
                _ => &I64,
            }),
            _ => None,
        }
    }

    /// The layout of a carrier holding `ty` in the design `ir`: [`of`](Self::of) the scalar the pin holds
    /// for it (`Ir::carried`: an enum without fields is its tag).
    pub fn in_ir(ir: &Ir, ty: &ValueType) -> Option<&'static Layout<'static>> {
        Self::of(&ir.carried(ty))
    }

    /// [`bytes`](Self::bytes) of a value of `ty` in the design `ir`: an enum constant is its
    /// discriminant held as the tag.
    pub fn bytes_in(ir: &Ir, ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        match ir.discriminant(ty, value) {
            Some(tag) => Self::bytes(&ir.carried(ty), Constant::Int(tag)),
            None => Self::bytes(&ir.carried(ty), value),
        }
    }

    /// The bytes of `value` held as `ty` (abi.md P3): the value little-endian at offset 0 and
    /// nothing else. `None` when `value` is not a value of `ty`.
    pub fn bytes(ty: &ValueType, value: Constant) -> Option<Vec<u8>> {
        let layout = Self::of(ty)?;
        let field = layout.fields.get(0)?;
        let value: Vec<u8> = match (ty, value) {
            (ValueType::Bool, Constant::Bool(b)) => vec![u8::from(b)],
            (ValueType::F64, Constant::Real(r)) => r.get().to_le_bytes().to_vec(),
            (ValueType::F32 | ValueType::F16 | ValueType::F128, Constant::FloatBits(bits)) => NumberLayout::float_bytes(ty, bits)?,
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                let (lo, hi) = ty.integer_range()?;
                if v < lo || v > hi {
                    return None;
                }
                v.to_le_bytes()[..leaf_size(field.leaf)?].to_vec()
            }
            _ => return None,
        };
        let mut bytes = vec![0u8; layout.size as usize];
        bytes[..value.len()].copy_from_slice(&value);
        Some(bytes)
    }
}

impl Digital {
    /// Adds one pin per carrier of `body`, in declaration order; returns the index of the first
    /// (the index the next pin would get when the body has none). The body passes `check()`, so
    /// every carrier holds `bool`, `f64` or an integer type.
    pub(crate) fn carrier_pins(s: &mut ShapeBuf, body: Body<'_>) -> Result<u32, PlanError> {
        let first = s.shape().pins.len() as u32;
        let drives = body.resolve_drive_carriers();
        for (i, c) in body.carriers().iter().enumerate() {
            let at = format!("{}:{}:{}", c.origin.file, c.origin.line, c.origin.col);
            // a resolved carrier's initial value is its first drive's: it starts the drive pin (`resolved_pins`)
            let init = match c.init.filter(|_| !matches!(c.role, CarrierRole::Resolved { .. })) {
                Some(value) => ArrayPin::init(body.ir(), c, value).or_else(|| PinLayout::bytes_in(body.ir(), &c.ty, value)).ok_or_else(|| {
                    PlanError::new(body.name(), format!("carrier {} at {at}: initial value {value:?} is not a value of {}", c.name, c.ty))
                })?,
                None => Vec::new(),
            };
            let pin_role = if Self::is_port(body, i as u32) { role::PORT } else { role::PRIVATE };
            let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            // a drive of a `resolve` is read through the drive layout of its net: the driven type, then `driving`
            let pin = if drives.contains(&(i as u32)) {
                let Some(layout) = DriveLayout::of(body.ir(), &c.ty) else {
                    return Err(PlanError::new(body.name(), format!("carrier {} at {at} holds {}, which no drive net holds", c.name, c.ty)));
                };
                let fields = layout.fields();
                let abi = layout.layout(&fields);
                s.pin(PinSpec { origin: &at, flags, init: &init, ..PinSpec::discrete(&c.name, pin_role, &abi) })
            } else {
                let Some(layout) = PinLayout::in_ir(body.ir(), &c.ty) else {
                    return Err(PlanError::new(body.name(), format!("carrier {} at {at} holds {}, which no pin holds", c.name, c.ty)));
                };
                let fields = layout.fields();
                let abi = layout.layout(&fields);
                s.pin(PinSpec { origin: &at, flags, init: &init, count: c.count.unwrap_or(0), ..PinSpec::discrete(&c.name, pin_role, &abi) })
            };
            debug_assert_eq!(pin, first + i as u32);
        }
        Ok(first)
    }

    /// Adds, after the `SETTLE` pin and the element drive pins, the drive pin `<carrier>.drive` of each resolved carrier
    /// (`PORT`, `RELAX`, the drive layout of its driven type) and, when the body calls `others()` of it, the others
    /// pin `<carrier>.others` (`PORT`, the carrier's own layout); `None` for every other carrier. The netlist binds
    /// the drive pin to the carrier's drive net and the others pin to its others net.
    pub(crate) fn resolved_pins(s: &mut ShapeBuf, body: Body<'_>) -> Result<Vec<Option<ResolvedPins>>, PlanError> {
        body.carriers()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let CarrierRole::Resolved { driven, .. } = &c.role else { return Ok(None) };
                let at = Self::at(&c.origin);
                let Some(layout) = DriveLayout::of(body.ir(), driven) else {
                    return Err(PlanError::new(body.name(), format!("carrier {} at {at} is driven with {driven}, which no drive net holds", c.name)));
                };
                let fields = layout.fields();
                let abi = layout.layout(&fields);
                // an initial value is the first drive at `(0, 0)`: the value then `driving` 1
                let init = c.init.and_then(|value| layout.init_bytes(body.ir(), driven, value)).unwrap_or_default();
                let flags = if init.is_empty() { pin_flag::RELAX } else { pin_flag::RELAX | pin_flag::HAS_INIT };
                let drive = s.pin(PinSpec { origin: &at, flags, init: &init, ..PinSpec::discrete(&format!("{}.drive", c.name), role::PORT, &abi) });
                let others = body.reads_others(i as u32).then(|| -> Result<u32, PlanError> {
                    let Some(layout) = PinLayout::in_ir(body.ir(), &c.ty) else {
                        return Err(PlanError::new(body.name(), format!("carrier {} at {at} holds {}, which no pin holds", c.name, c.ty)));
                    };
                    let fields = layout.fields();
                    let abi = layout.layout(&fields);
                    Ok(s.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("{}.others", c.name), role::PORT, &abi) }))
                });
                let others = others.transpose()?;
                Ok(Some(ResolvedPins { drive, others }))
            })
            .collect()
    }

    /// The size in bytes of the drive net of resolved carrier `c` of `body`; 0 for a carrier that is not resolved.
    pub(super) fn drive_size(body: Body<'_>, c: u32) -> u32 {
        match body.carriers().get(c as usize).map(|x| &x.role) {
            Some(CarrierRole::Resolved { driven, .. }) => DriveLayout::of(body.ir(), driven).map_or(0, |layout| layout.size()),
            _ => 0,
        }
    }

    /// Whether carrier `c` of `body` is a carrier of a resolved nature: a definition or a commit of it writes its drive pin.
    pub(crate) fn is_resolved(body: Body<'_>, c: u32) -> bool {
        body.carriers().get(c as usize).is_some_and(|x| matches!(x.role, CarrierRole::Resolved { .. }))
    }

    /// Whether carrier `c` of `body` is a `PORT` pin: an `in` or `out` port, a `signal` that a
    /// child's carrier port is bound to, or an array `signal` a definition of one element drives.
    pub(crate) fn is_port(body: Body<'_>, c: u32) -> bool {
        match body.carriers().get(c as usize).map(|x| &x.role) {
            // A reached carrier is a read-only view of a child's: a `PORT` pin, as an `in` port.
            Some(CarrierRole::In | CarrierRole::Out | CarrierRole::Reached | CarrierRole::Resolved { .. }) => true,
            // A `signal` is met on a net when a child's carrier port is bound to it or a parent reads it (`inst.name`), and
            // when a definition of one of its elements writes it: that drive's pin is bound to the element of the array
            // net (`Pins::element_drives`), so the carrier's own pin, which its readers read, must be that net too
            Some(CarrierRole::Signal | CarrierRole::Internal) => {
                (0..body.children().len()).any(|k| body.child_carriers(k).contains(&Some(c))) || body.is_reached(c) || Self::has_element_drive(body, c)
            }
            None => false,
        }
    }

    /// Whether an undelayed definition of one element of the array carrier `c` writes it through an element drive pin.
    pub(super) fn has_element_drive(body: Body<'_>, c: u32) -> bool {
        body.drives().iter().any(|drive| drive.carrier == c && drive.element.is_some() && drive.after.is_none())
    }

    /// `origin` as a shape writes it.
    pub(super) fn at(o: &Origin) -> String {
        format!("{}:{}:{}", o.file, o.line, o.col)
    }

    /// The name of carrier `c` of `body`.
    pub(super) fn carrier_name(body: Body<'_>, c: u32) -> String {
        body.carriers().get(c as usize).map_or_else(|| format!("carrier{c}"), |x| x.name.clone())
    }

    /// Adds the `PORT` pin of each drive of one element of an array carrier, `drive:<carrier>[<element>]`, in the element's
    /// layout; `None` for every other drive. The netlist binds it to that element of the array net (`Port::Window`).
    pub(crate) fn element_drive_pins(s: &mut ShapeBuf, body: Body<'_>) -> Vec<Option<u32>> {
        body.drives()
            .iter()
            .map(|d| {
                let element = d.element.filter(|_| d.after.is_none())?;
                let carrier = body.carriers().get(d.carrier as usize)?;
                let layout = PinLayout::in_ir(body.ir(), &carrier.ty)?;
                let fields = layout.fields();
                let abi = layout.layout(&fields);
                let name = format!("drive:{}[{element}]", carrier.name);
                Some(s.pin(PinSpec { origin: &Self::at(&d.origin), ..PinSpec::discrete(&name, role::PORT, &abi) }))
            })
            .collect()
    }
}
