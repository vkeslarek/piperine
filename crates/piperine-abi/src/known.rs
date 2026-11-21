//! The reader of a field's known flags (abi.md §3, P3, C7, §9 D1): one owner for the two forms
//! the flags take, the plain known byte (`HAS_KNOWN`, one byte per element, minor 1) and the
//! per-level mask (`HAS_KNOWN | KNOWN_LEVELS`, `leaf_size` bytes per element, bit `k` set when
//! level `k` is known, minor 2). Merge, canonical form, relaxation and host writes all read a
//! field through a [`FieldKnown`], so no caller does byte arithmetic on a flag.

use crate::layout::leaf_size;
use crate::raw::{Field, field_flag};

/// How the known flags of one element are stored (abi.md §3 `Field.known`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KnownForm {
    /// One byte: 0 is no value, anything else is the whole value known (minor 1).
    Byte,
    /// A mask of the leaf's size, bit `k` set when level `k` is known (`KNOWN_LEVELS`).
    Mask,
}

/// The known flags of one field of a layout, read per element (abi.md §3, P3).
///
/// Every method takes `bytes`, the bytes of a whole value of the field's layout: the caller has
/// checked they cover `Layout.size`, as `validate` guarantees of a bound shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldKnown {
    /// The offset of the first value element.
    value: usize,
    /// The size of one value element, which is also the size of one mask.
    width: usize,
    /// The offset of the first known flag.
    known: usize,
    /// The array length.
    count: usize,
    /// How the flags are stored.
    form: KnownForm,
}

impl FieldKnown {
    /// The reader of `field`'s known flags; `None` for a field without `HAS_KNOWN` or with a
    /// leaf this ABI does not define.
    pub fn of(field: &Field) -> Option<FieldKnown> {
        if field.flags & field_flag::HAS_KNOWN == 0 {
            return None;
        }
        let form = if field.flags & field_flag::KNOWN_LEVELS != 0 { KnownForm::Mask } else { KnownForm::Byte };
        Some(FieldKnown {
            value: field.offset as usize,
            width: leaf_size(field.leaf)?,
            known: field.known as usize,
            count: field.count as usize,
            form,
        })
    }

    /// What P4 compares of a field's known flags: the offset and whether it is a mask; `None`
    /// for a field without `HAS_KNOWN`.
    pub fn form_of(field: &Field) -> Option<(u32, bool)> {
        (field.flags & field_flag::HAS_KNOWN != 0).then_some((field.known, field.flags & field_flag::KNOWN_LEVELS != 0))
    }

    /// The array length of the field.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Whether the flags are one mask per element.
    pub fn is_mask(&self) -> bool {
        self.form == KnownForm::Mask
    }

    /// Whether the value elements and the known flags all lie within `size` bytes.
    pub fn fits(&self, size: usize) -> bool {
        self.value + self.count * self.width <= size && self.known + self.count * self.known_width() <= size
    }

    /// Whether every level of `element` is known: its byte is non-zero, or every bit of its
    /// mask is set. A type narrower than its leaf (a `u9` in a `U16`) has fewer levels than the
    /// leaf has bits, which the ABI cannot know: ask [`FieldKnown::level`] for those.
    pub fn is_whole(&self, bytes: &[u8], element: usize) -> bool {
        match self.form {
            KnownForm::Byte => self.known_word(bytes, element) != 0,
            KnownForm::Mask => self.known_word(bytes, element) == self.full_mask(),
        }
    }

    /// Whether no level of `element` is known (P3: such an element holds no value).
    pub fn is_none(&self, bytes: &[u8], element: usize) -> bool {
        self.known_word(bytes, element) == 0
    }

    /// Whether level `k` of `element` is known. A plain known byte has the one level 0.
    pub fn level(&self, bytes: &[u8], element: usize, k: u32) -> bool {
        let known = self.known_word(bytes, element);
        match self.form {
            KnownForm::Byte => k == 0 && known != 0,
            KnownForm::Mask => k < 8 * self.width as u32 && known >> k & 1 == 1,
        }
    }

    /// Marks every level of `element` known: the write of a host value (abi.md §3 P3).
    pub fn fill(&self, bytes: &mut [u8], element: usize) {
        let all = match self.form {
            KnownForm::Byte => 1,
            KnownForm::Mask => self.full_mask(),
        };
        self.set_known_word(bytes, element, all);
    }

    /// P3 for `element`: all-zero flags hold no value, and no value bit of an unknown level is
    /// set.
    pub fn is_canonical(&self, bytes: &[u8], element: usize) -> bool {
        let value = self.value_word(bytes, element);
        match self.form {
            KnownForm::Byte => self.known_word(bytes, element) != 0 || value == 0,
            KnownForm::Mask => value & !self.known_word(bytes, element) == 0,
        }
    }

    /// C7 for `element`: `settled` keeps each level that both it and `committed` hold known and
    /// equal and loses the rest, value bits included (P3). A plain known byte keeps the whole
    /// element or loses it.
    pub fn merge(&self, settled: &mut [u8], committed: &[u8], element: usize) {
        let (settled_value, committed_value) = (self.value_word(settled, element), self.value_word(committed, element));
        let (settled_known, committed_known) = (self.known_word(settled, element), self.known_word(committed, element));
        match self.form {
            KnownForm::Byte => {
                if settled_known == 0 || committed_known == 0 || settled_value != committed_value {
                    self.set_known_word(settled, element, 0);
                    self.set_value_word(settled, element, 0);
                }
            }
            KnownForm::Mask => {
                let kept = settled_known & committed_known & !(settled_value ^ committed_value);
                self.set_known_word(settled, element, kept);
                self.set_value_word(settled, element, settled_value & kept);
            }
        }
    }

    /// D1 for `element`: clears in `value` the levels whose value bit or flag differs between
    /// `before` and `after` (`value` starts as a copy of `after`), flag and value bit both. A
    /// plain known byte clears the whole element. True when something moved.
    pub fn clear_moving(&self, before: &[u8], after: &[u8], value: &mut [u8], element: usize) -> bool {
        let moved_value = self.value_word(before, element) ^ self.value_word(after, element);
        let moved_known = self.known_word(before, element) ^ self.known_word(after, element);
        match self.form {
            KnownForm::Byte => {
                let moved = moved_value != 0 || moved_known != 0;
                if moved {
                    self.set_known_word(value, element, 0);
                    self.set_value_word(value, element, 0);
                }
                moved
            }
            KnownForm::Mask => {
                let moved = moved_value | moved_known;
                self.set_known_word(value, element, self.known_word(value, element) & !moved);
                self.set_value_word(value, element, self.value_word(value, element) & !moved);
                moved != 0
            }
        }
    }

    /// The mask with every bit of the leaf set.
    fn full_mask(&self) -> u64 {
        if self.width >= 8 { u64::MAX } else { (1u64 << (8 * self.width)) - 1 }
    }

    /// The size of one known flag in bytes.
    fn known_width(&self) -> usize {
        match self.form {
            KnownForm::Byte => 1,
            KnownForm::Mask => self.width,
        }
    }

    /// The little-endian word of `width` bytes at `at`.
    fn word(bytes: &[u8], at: usize, width: usize) -> u64 {
        bytes[at..at + width].iter().rev().fold(0u64, |word, byte| word << 8 | u64::from(*byte))
    }

    /// Writes the low `width` bytes of `word` little-endian at `at`.
    fn store(bytes: &mut [u8], at: usize, width: usize, word: u64) {
        bytes[at..at + width].copy_from_slice(&word.to_le_bytes()[..width]);
    }

    /// The value element as a word (the leaf's bits, level `k` at bit `k`).
    fn value_word(&self, bytes: &[u8], element: usize) -> u64 {
        Self::word(bytes, self.value + element * self.width, self.width)
    }

    /// Writes the value element.
    fn set_value_word(&self, bytes: &mut [u8], element: usize, word: u64) {
        Self::store(bytes, self.value + element * self.width, self.width, word);
    }

    /// The known flag of `element` as a word.
    fn known_word(&self, bytes: &[u8], element: usize) -> u64 {
        Self::word(bytes, self.known + element * self.known_width(), self.known_width())
    }

    /// Writes the known flag of `element`.
    fn set_known_word(&self, bytes: &mut [u8], element: usize, word: u64) {
        Self::store(bytes, self.known + element * self.known_width(), self.known_width(), word);
    }
}
