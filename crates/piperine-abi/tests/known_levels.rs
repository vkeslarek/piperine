//! Per-level known flags of the device ABI (feature `unknown-semantics-per-level`, UL-05; abi.md
//! §3, P3, P4, C7, §9): the `KNOWN_LEVELS` field flag reads one mask per element, validation
//! refuses a malformed levels field by name, canonical form, layout equality and merge work level
//! by level, and a plain known byte (minor 1) keeps reading as one level.

use piperine_abi::known::FieldKnown;
use piperine_abi::layout::{Merge, is_canonical, layout_eq};
use piperine_abi::raw::{Field, Layout, List, Str, field_flag, leaf, role};
use piperine_abi::shape::{PinSpec, ShapeBuf};
use piperine_abi::validate::{Rule, validate};

/// The fields the tests read, built by hand: the contract is bytes, not the transpiler.
struct Fixture;

impl Fixture {
    /// A `U16` field of two elements with a mask per element: value bytes 0..4, masks 4..8.
    fn masks() -> Field<'static> {
        Field {
            name: Str::from_static("v"),
            leaf: leaf::U16,
            offset: 0,
            count: 2,
            flags: field_flag::HAS_KNOWN | field_flag::KNOWN_LEVELS,
            known: 4,
        }
    }

    /// A `U8` field with a plain known byte (minor 1): value byte 0, known byte 1.
    fn plain_byte() -> Field<'static> {
        Field { name: Str::from_static("v"), leaf: leaf::U8, offset: 0, count: 1, flags: field_flag::HAS_KNOWN, known: 1 }
    }
}

impl Fixture {
    /// The layout of [`Fixture::masks`]: two `U16` elements and their masks, 8 bytes.
    fn mask_layout<'a>(fields: &'a [Field<'a>]) -> Layout<'a> {
        Layout { size: 8, align: 2, fields: List::from_slice(fields) }
    }

    /// The layout of [`Fixture::single`]: one `U16` and its flag, 4 bytes.
    fn single_layout<'a>(fields: &'a [Field<'a>]) -> Layout<'a> {
        Layout { size: 4, align: 2, fields: List::from_slice(fields) }
    }

    /// One `U16` element with a known flag at byte 2, a mask when `levels`, a plain byte otherwise.
    fn single(levels: bool) -> Field<'static> {
        let flags = if levels { field_flag::HAS_KNOWN | field_flag::KNOWN_LEVELS } else { field_flag::HAS_KNOWN };
        Field { name: Str::from_static("v"), leaf: leaf::U16, offset: 0, count: 1, flags, known: 2 }
    }
}

/// Shapes with one discrete pin `lv` of a hand-built layout, validated.
struct Shapes;

impl Shapes {
    /// The result of validating a shape whose only pin is `lv` with `layout`.
    fn validate_pin(layout: &Layout<'_>) -> Result<(), piperine_abi::validate::ShapeError> {
        let mut buf = ShapeBuf::new();
        buf.pin(PinSpec { origin: "lv.msl:1:1", ..PinSpec::discrete("lv", role::PRIVATE, layout) });
        let shape = buf.freeze();
        validate(shape.shape())
    }
}

/// abi.md §3 `KNOWN_LEVELS`: the mask is one per element, the size of the leaf, bit `k` = level
/// `k` known; a plain known byte reads as one level.
#[test]
fn a_levels_field_reads_one_mask_per_element() {
    let field = Fixture::masks();
    let known = FieldKnown::of(&field).expect("a field with HAS_KNOWN");
    // value elements 0x0005 and 0x01FF, masks 0x0005 and 0x01FF, little-endian
    let bytes = [0x05, 0x00, 0xFF, 0x01, 0x05, 0x00, 0xFF, 0x01];
    assert!(known.is_mask());
    assert_eq!(known.count(), 2);
    let first: Vec<bool> = (0..16).map(|k| known.level(&bytes, 0, k)).collect();
    let expected_first: Vec<bool> = (0..16).map(|k| k == 0 || k == 2).collect();
    assert_eq!(first, expected_first, "mask 0x0005 knows levels 0 and 2 of element 0");
    let second: Vec<bool> = (0..16).map(|k| known.level(&bytes, 1, k)).collect();
    let expected_second: Vec<bool> = (0..16).map(|k| k < 9).collect();
    assert_eq!(second, expected_second, "mask 0x01FF knows levels 0 to 8 of element 1");
    assert!(!known.is_whole(&bytes, 0) && !known.is_none(&bytes, 0), "0x0005 is partly known");
    assert!(!known.is_whole(&bytes, 1), "0x01FF leaves the leaf's upper levels unknown");
    assert!(known.is_none(&[0; 8], 1), "a zero mask is every level unknown");

    // fill: a host value makes every level of the element known
    let mut written = [0x34, 0x12, 0, 0, 0, 0, 0, 0];
    known.fill(&mut written, 0);
    assert_eq!(written, [0x34, 0x12, 0, 0, 0xFF, 0xFF, 0, 0], "the full mask of a U16 is 0xFFFF");
    assert!(known.is_whole(&written, 0) && known.is_none(&written, 1));

    // a plain known byte is one level
    let plain = FieldKnown::of(&Fixture::plain_byte()).expect("a field with HAS_KNOWN");
    assert!(!plain.is_mask());
    assert!(plain.level(&[7, 1], 0, 0), "a known byte knows level 0");
    assert!(!plain.level(&[7, 1], 0, 1), "and no other");
    assert!(!plain.level(&[0, 0], 0, 0), "a zero byte knows nothing");
    assert!(plain.is_whole(&[7, 1], 0) && plain.is_none(&[0, 0], 0));
}

/// abi.md §3, UL-05 AC 3: `KNOWN_LEVELS` without `HAS_KNOWN` is refused, naming the rule and the
/// field.
#[test]
fn levels_without_has_known_is_refused_by_name() {
    let fields = [Field {
        name: Str::from_static("levels_field"),
        leaf: leaf::U16,
        offset: 0,
        count: 1,
        flags: field_flag::KNOWN_LEVELS,
        known: 2,
    }];
    let layout = Layout { size: 4, align: 2, fields: List::from_slice(&fields) };
    let error = Shapes::validate_pin(&layout).expect_err("a mask with no known flags");
    assert_eq!(error.rule, Rule::LevelsWithoutKnown);
    assert!(error.to_string().contains("levels_field"), "the message names the field: {error}");
}

/// abi.md §3, UL-05 AC 3: a mask that does not fit the layout is refused, naming the rule and the
/// field; the same field inside the layout validates.
#[test]
fn a_mask_outside_the_layout_is_refused_by_name() {
    let flags = field_flag::HAS_KNOWN | field_flag::KNOWN_LEVELS;
    let outside = [Field { name: Str::from_static("wide"), leaf: leaf::U16, offset: 0, count: 2, flags, known: 4 }];
    let layout = Layout { size: 6, align: 2, fields: List::from_slice(&outside) };
    let error = Shapes::validate_pin(&layout).expect_err("masks of 2 x 2 bytes at 4 end at 8, past 6");
    assert_eq!(error.rule, Rule::MaskOutsideLayout);
    assert!(error.to_string().contains("wide"), "the message names the field: {error}");

    let inside = Layout { size: 8, align: 2, fields: List::from_slice(&outside) };
    Shapes::validate_pin(&inside).expect("the same field in an 8-byte layout validates");
}

/// abi.md §3.1 P3 per level: all-zero bytes hold no value; the value bit of an unknown level is
/// zero; a plain known byte of 0 holds no value.
#[test]
fn canonical_form_holds_per_level() {
    let fields = [Fixture::masks()];
    let layout = Fixture::mask_layout(&fields);
    // element 0: value 0b0101 with mask 0b0111 (level 1 known false); element 1 all zero
    assert!(is_canonical(&layout, &[0x05, 0, 0, 0, 0x07, 0, 0, 0]), "value bits only on known levels");
    assert!(is_canonical(&layout, &[0; 8]), "all-zero bytes are the no-value form");
    // level 3 of element 0 has its value bit set while its flag is clear
    assert!(!is_canonical(&layout, &[0x08, 0, 0, 0, 0x07, 0, 0, 0]), "a value bit on an unknown level");
    // a zero mask with a non-zero value is the old P3 violation
    assert!(!is_canonical(&layout, &[0, 0, 0x01, 0, 0, 0, 0, 0]), "element 1 has a value and no flag");
    assert!(!is_canonical(&layout, &[0; 7]), "bytes shorter than the layout are not canonical");

    let plain = [Fixture::plain_byte()];
    let byte = Layout { size: 2, align: 1, fields: List::from_slice(&plain) };
    assert!(is_canonical(&byte, &[0, 0]) && is_canonical(&byte, &[7, 1]));
    assert!(!is_canonical(&byte, &[7, 0]), "a plain known byte of 0 holds no value");
}

/// abi.md C7 per level: a merge keeps the levels both values hold known and equal and clears the
/// rest, value bits included; an element equal on both sides is unchanged.
#[test]
fn a_merge_commit_keeps_the_levels_both_hold() {
    let fields = [Fixture::masks()];
    let layout = Fixture::mask_layout(&fields);
    // element 0 settled: levels 1, 2, 3 known, values 1, 0, 1 -> value 0b1010, mask 0b1110.
    // element 0 committed: levels 0, 1, 2 known, values 1, 1, 1 -> value 0b0111, mask 0b0111.
    // both known: levels 1 and 2; equal: level 1 (1 = 1) only -> mask 0b0010, value 0b0010.
    // element 1 is the same on both sides (value 0x00AB, mask 0x01FF) and stays.
    let mut settled = [0x0A, 0, 0xAB, 0x00, 0x0E, 0, 0xFF, 0x01];
    let committed = [0x07, 0, 0xAB, 0x00, 0x07, 0, 0xFF, 0x01];
    Merge::of(&layout).merge_into(&mut settled, &committed).expect("a mergeable layout");
    assert_eq!(settled, [0x02, 0, 0xAB, 0x00, 0x02, 0, 0xFF, 0x01]);
    assert!(is_canonical(&layout, &settled), "the result is canonical (P3)");

    // a level known on one side only is cleared, and nothing is invented
    let mut one_sided = [0x01, 0, 0, 0, 0x01, 0, 0, 0];
    Merge::of(&layout).merge_into(&mut one_sided, &[0; 8]).expect("a mergeable layout");
    assert_eq!(one_sided, [0; 8], "an unknown commit clears every level");
}

/// abi.md P4 with `KNOWN_LEVELS`: the flag counts, so the same field with a plain known byte and
/// with a mask are unequal layouts (D4); names still do not count.
#[test]
fn pins_with_and_without_levels_are_unequal() {
    let (with, without) = ([Fixture::single(true)], [Fixture::single(false)]);
    assert!(!layout_eq(&Fixture::single_layout(&with), &Fixture::single_layout(&without)), "a mask and a plain byte are not one shape");
    assert!(layout_eq(&Fixture::single_layout(&with), &Fixture::single_layout(&with)));
    let renamed = [Field { name: Str::from_static("w"), ..Fixture::single(true) }];
    assert!(layout_eq(&Fixture::single_layout(&with), &Fixture::single_layout(&renamed)), "names do not count");
}
