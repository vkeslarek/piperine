//! The shipped layouts and the two byte rules of abi.md §3.1 (feature
//! `solver-abi-core`, SAC-03): `BIT`, `REAL` and `BYTE` with their stated
//! fields, layout equality that ignores names and nothing else (P4), and the
//! canonical form of a field with a known byte (P3), and the merge of two values of a layout
//! (abi.md C7, part I §3.3.3).

use piperine_abi::layout::{BIT, BYTE, HIGH, LOW, REAL, is_canonical, layout_eq, leaf_size};
use piperine_abi::raw::{Field, Layout, List, Str, field_flag, leaf};

/// One field of a layout: (name, leaf, offset, count, flags, known).
fn fields_of(layout: &Layout) -> Vec<(String, u32, u32, u32, u32, u32)> {
    layout
        .fields
        .iter()
        .map(|f| (f.name.as_str().to_string(), f.leaf, f.offset, f.count, f.flags, f.known))
        .collect()
}

#[test]
fn shipped_layouts_have_their_stated_fields() {
    // abi.md §11: `BIT` is `{ v: BOOL @0, known @1 }`.
    assert_eq!((BIT.size, BIT.align), (2, 1));
    assert_eq!(fields_of(&BIT), [("v".to_string(), leaf::BOOL, 0, 1, field_flag::HAS_KNOWN, 1)]);
    // abi.md §3.1 P3 (written by this task): `REAL` is `{ v: F64 @0, known @8 }`.
    assert_eq!((REAL.size, REAL.align), (16, 8));
    assert_eq!(fields_of(&REAL), [("v".to_string(), leaf::F64, 0, 1, field_flag::HAS_KNOWN, 8)]);
    // `BYTE` is `{ v: U8 @0, known @1 }`.
    assert_eq!((BYTE.size, BYTE.align), (2, 1));
    assert_eq!(fields_of(&BYTE), [("v".to_string(), leaf::U8, 0, 1, field_flag::HAS_KNOWN, 1)]);
    // abi.md §11: `LOW = [0,1]`, `HIGH = [1,1]`.
    assert_eq!(LOW, [0, 1]);
    assert_eq!(HIGH, [1, 1]);
    // Both are canonical values of `BIT`.
    assert!(is_canonical(&BIT, &LOW));
    assert!(is_canonical(&BIT, &HIGH));
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

fn field(name: &'static str, leaf: u32, offset: u32, count: u32, known: Option<u32>) -> Field<'static> {
    Field {
        name: Str::from_static(name),
        leaf,
        offset,
        count,
        flags: if known.is_some() { field_flag::HAS_KNOWN } else { 0 },
        known: known.unwrap_or(0),
    }
}

fn layout<'a>(size: u32, align: u32, fields: &'a [Field<'a>]) -> Layout<'a> {
    Layout { size, align, fields: List::from_slice(fields) }
}

#[test]
fn layout_equality_ignores_names() {
    let base = [field("v", leaf::F64, 0, 1, Some(8)), field("w", leaf::I32, 12, 1, None)];
    let renamed = [field("value", leaf::F64, 0, 1, Some(8)), field("other", leaf::I32, 12, 1, None)];
    let a = layout(16, 8, &base);
    assert!(layout_eq(&a, &a));
    assert!(layout_eq(&a, &layout(16, 8, &renamed)), "names do not count");
    // A field-less pair of equal size and alignment is equal.
    assert!(layout_eq(&layout(0, 1, &[]), &layout(0, 1, &[])));

    let leaf_differs = [field("v", leaf::I64, 0, 1, Some(8)), field("w", leaf::I32, 12, 1, None)];
    let offset_differs = [field("v", leaf::F64, 0, 1, Some(8)), field("w", leaf::I32, 9, 1, None)];
    let count_differs = [field("v", leaf::F64, 0, 1, Some(8)), field("w", leaf::I32, 12, 2, None)];
    let known_differs = [field("v", leaf::F64, 0, 1, Some(9)), field("w", leaf::I32, 12, 1, None)];
    let known_absent = [field("v", leaf::F64, 0, 1, None), field("w", leaf::I32, 12, 1, None)];
    for (what, other) in [
        ("leaf", &leaf_differs),
        ("offset", &offset_differs),
        ("count", &count_differs),
        ("known offset", &known_differs),
        ("known byte present", &known_absent),
    ] {
        assert!(!layout_eq(&a, &layout(16, 8, other)), "a differing {what} is unequal");
    }
    assert!(!layout_eq(&a, &layout(24, 8, &base)), "a differing size is unequal");
    assert!(!layout_eq(&a, &layout(16, 4, &base)), "a differing alignment is unequal");
    assert!(!layout_eq(&a, &layout(16, 8, &base[..1])), "a differing field count is unequal");
}

#[test]
fn a_valueless_field_with_value_bytes_is_not_canonical() {
    // P3: all-zero bytes are canonical (nothing known, nothing held).
    assert!(is_canonical(&BIT, &[0, 0]));
    assert!(is_canonical(&REAL, &[0; 16]));
    assert!(is_canonical(&BYTE, &[0, 0]));
    // Known byte 0 with a non-zero value byte: not canonical.
    assert!(!is_canonical(&BIT, &[1, 0]));
    assert!(!is_canonical(&BYTE, &[7, 0]));
    let mut real = [0u8; 16];
    real[..8].copy_from_slice(&(-0.0f64).to_le_bytes());
    assert!(!is_canonical(&REAL, &real), "-0.0 has a non-zero byte");
    real[8] = 1;
    assert!(is_canonical(&REAL, &real), "a known -0.0 is canonical");

    // An array field: one known byte per element.
    let arr = [field("a", leaf::U16, 0, 3, Some(6))];
    let arr = layout(10, 2, &arr);
    assert!(is_canonical(&arr, &[0; 10]));
    // element 1 known, holds 0x0102; elements 0 and 2 unknown and zero
    assert!(is_canonical(&arr, &[0, 0, 2, 1, 0, 0, 0, 1, 0, 0]));
    // element 2 unknown but its value bytes are not zero
    assert!(!is_canonical(&arr, &[0, 0, 2, 1, 0, 9, 0, 1, 0, 0]));

    // A field without a known byte constrains nothing.
    let plain = [field("x", leaf::U8, 0, 1, None)];
    assert!(is_canonical(&layout(1, 1, &plain), &[0xFF]));

    // Bytes shorter than the layout are not a value of it.
    assert!(!is_canonical(&BIT, &[0]));
}
