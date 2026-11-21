//! Per-level known flags of the device ABI (feature `unknown-semantics-per-level`, UL-05; abi.md
//! §3, P3, P4, C7, §9): the `KNOWN_LEVELS` field flag reads one mask per element, validation
//! refuses a malformed levels field by name, canonical form, layout equality and merge work level
//! by level, and a plain known byte (minor 1) keeps reading as one level.

use piperine_abi::known::FieldKnown;
use piperine_abi::raw::{Field, Str, field_flag, leaf};

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
