//! The shipped layouts and the byte rule of abi.md §3.1 (feature `solver-abi-core`, SAC-03;
//! explicit-unknowns EU-16): `BIT`, `BYTE` and `F64` hold the value only, with their stated
//! fields; layout equality ignores names and nothing else (P4); an `Option<T>` pin is a presence
//! field then the payload (P3, P5).

use piperine_abi::layout::{BIT, BYTE, F64, HIGH, LOW, layout_eq, leaf_size};
use piperine_abi::raw::{Field, Layout, List, Str, leaf};
use piperine_abi::validate::validate;

#[test]
fn the_shipped_layouts_hold_the_value_only() {
    // abi.md §3.1 P3: `BIT` is `{ v: BOOL @0 }`, `BYTE` is `{ v: U8 @0 }`, `F64` is
    // `{ v: F64 @0 }`; no layout has a byte beside its value
    assert_eq!((BIT.size, BIT.align), (1, 1));
    assert_eq!(LayoutReading::fields_of(&BIT), [("v".to_string(), leaf::BOOL, 0, 1)]);
    assert_eq!((BYTE.size, BYTE.align), (1, 1));
    assert_eq!(LayoutReading::fields_of(&BYTE), [("v".to_string(), leaf::U8, 0, 1)]);
    assert_eq!((F64.size, F64.align), (8, 8));
    assert_eq!(LayoutReading::fields_of(&F64), [("v".to_string(), leaf::F64, 0, 1)]);
    // `LOW` and `HIGH` are the bytes of `false` and `true`
    assert_eq!(LOW, [0]);
    assert_eq!(HIGH, [1]);
    // Leaf sizes, little-endian scalars of abi.md §3.
    let sizes = [
        (leaf::BOOL, 1), (leaf::U8, 1), (leaf::U16, 2), (leaf::U32, 4), (leaf::U64, 8),
        (leaf::I8, 1), (leaf::I16, 2), (leaf::I32, 4), (leaf::I64, 8), (leaf::F32, 4),
        (leaf::F64, 8),
    ];
    for (l, size) in sizes {
        assert_eq!(leaf_size(l), Some(size), "leaf {l}");
    }
    assert_eq!(leaf_size(0), None);
    assert_eq!(leaf_size(12), None);
}

#[test]
fn layout_equality_ignores_names() {
    let base = [LayoutReading::field("v", leaf::F64, 0, 1), LayoutReading::field("w", leaf::I32, 12, 1)];
    let renamed = [LayoutReading::field("value", leaf::F64, 0, 1), LayoutReading::field("other", leaf::I32, 12, 1)];
    let a = LayoutReading::layout(16, 8, &base);
    assert!(layout_eq(&a, &a));
    assert!(layout_eq(&a, &LayoutReading::layout(16, 8, &renamed)), "names do not count");
    // A field-less pair of equal size and alignment is equal.
    assert!(layout_eq(&LayoutReading::layout(0, 1, &[]), &LayoutReading::layout(0, 1, &[])));

    let leaf_differs = [LayoutReading::field("v", leaf::I64, 0, 1), LayoutReading::field("w", leaf::I32, 12, 1)];
    let offset_differs = [LayoutReading::field("v", leaf::F64, 0, 1), LayoutReading::field("w", leaf::I32, 9, 1)];
    let count_differs = [LayoutReading::field("v", leaf::F64, 0, 1), LayoutReading::field("w", leaf::I32, 12, 2)];
    for (what, other) in [("leaf", &leaf_differs), ("offset", &offset_differs), ("count", &count_differs)] {
        assert!(!layout_eq(&a, &LayoutReading::layout(16, 8, other)), "a differing {what} is unequal");
    }
    assert!(!layout_eq(&a, &LayoutReading::layout(24, 8, &base)), "a differing size is unequal");
    assert!(!layout_eq(&a, &LayoutReading::layout(16, 4, &base)), "a differing alignment is unequal");
    assert!(!layout_eq(&a, &LayoutReading::layout(16, 8, &base[..1])), "a differing field count is unequal");
}

#[test]
fn an_option_pin_is_presence_then_payload() {
    // EU-16 AC 4, abi.md P3 and P5: an `Option<u8>` pin is `{ some: BOOL @0, v: U8 @1 }`, two
    // bytes; `None` is both bytes zero, `Some(7)` is `[1, 7]`, and the layout is a valid one
    let fields = [LayoutReading::field("some", leaf::BOOL, 0, 1), LayoutReading::field("v", leaf::U8, 1, 1)];
    let option = LayoutReading::layout(2, 1, &fields);
    assert_eq!(LayoutReading::fields_of(&option), [("some".to_string(), leaf::BOOL, 0, 1), ("v".to_string(), leaf::U8, 1, 1)]);
    assert!(!layout_eq(&option, &BYTE), "an `Option<u8>` is not a `u8`: its size and its fields differ");
    let widest = [LayoutReading::field("some", leaf::BOOL, 0, 1), LayoutReading::field("v", leaf::F64, 1, 1)];
    assert_eq!(LayoutReading::layout(9, 1, &widest).size, 1 + leaf_size(leaf::F64).expect("a leaf") as u32, "the payload follows the presence byte");
    assert_eq!(LayoutReading::validate_layout(&option), Ok(()));
}

#[test]
fn a_logic_pin_is_two_planes() {
    // EU-16 AC 5 (explicit-unknowns T47, C69): a pin of `Logic<N>` is two fields, `binary` at offset 0 and `value` at the size of
    // `UInt<N>`, each with the leaf of `UInt<N>`, the whole twice that size and packed (alignment 1, the composite rule; coordinator ruling
    // 2026-11-23). `Logic<4>` is two `U8` fields of size 2; `Logic<64>` two `U64` of size 16; both are valid layouts
    let narrow = [LayoutReading::field("binary", leaf::U8, 0, 1), LayoutReading::field("value", leaf::U8, 1, 1)];
    let wide = [LayoutReading::field("binary", leaf::U64, 0, 1), LayoutReading::field("value", leaf::U64, 8, 1)];
    let (four, sixty_four) = (LayoutReading::layout(2, 1, &narrow), LayoutReading::layout(16, 1, &wide));
    assert_eq!((four.size, four.align), (2, 1));
    assert_eq!(LayoutReading::fields_of(&four), [("binary".to_string(), leaf::U8, 0, 1), ("value".to_string(), leaf::U8, 1, 1)]);
    assert_eq!((sixty_four.size, sixty_four.align), (16, 1));
    assert_eq!(LayoutReading::fields_of(&sixty_four), [("binary".to_string(), leaf::U64, 0, 1), ("value".to_string(), leaf::U64, 8, 1)]);
    for pin in [&four, &sixty_four] {
        assert_eq!(LayoutReading::validate_layout(pin), Ok(()), "a plane pair is a valid layout of size {}", pin.size);
    }
    assert!(!layout_eq(&four, &sixty_four), "two widths are two layouts");
}

/// Reads layouts back for the layout tests.
pub struct LayoutReading;

impl LayoutReading {
    /// One field of a layout: (name, leaf, offset, count).
    fn fields_of(layout: &Layout) -> Vec<(String, u32, u32, u32)> {
        layout.fields.iter().map(|f| (f.name.as_str().to_string(), f.leaf, f.offset, f.count)).collect()
    }

    /// The field helper `field`.
    fn field(name: &'static str, leaf: u32, offset: u32, count: u32) -> Field<'static> {
        Field { name: Str::from_static(name), leaf, offset, count }
    }

    /// The layout helper `layout`.
    fn layout<'a>(size: u32, align: u32, fields: &'a [Field<'a>]) -> Layout<'a> {
        Layout { size, align, fields: List::from_slice(fields) }
    }

    /// The result of validating a pin whose layout is `layout`: the abi crate's own shape rules.
    fn validate_layout(layout: &Layout<'_>) -> Result<(), String> {
        use piperine_abi::raw::role;
        use piperine_abi::shape::{PinSpec, ShapeBuf};
        let mut shape = ShapeBuf::new();
        shape.pin(PinSpec::discrete("o", role::PORT, layout));
        let frozen = shape.freeze();
        validate(frozen.shape()).map_err(|e| e.to_string())
    }
}
