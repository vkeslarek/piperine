//! D1 on one net (abi.md §9): what a relaxed net holds when its cycle of functions did not settle.
//! A field with known flags that still moves loses what moves, a field without them is a fault.

use piperine_abi::known::FieldKnown;
use piperine_abi::layout::leaf_size;
use piperine_abi::raw::Layout;

/// A relaxed net whose last pass still moved its bytes, read through its layout.
pub struct Relaxation<'a> {
    /// The net's layout.
    layout: &'a Layout<'a>,
    /// The bytes at the start of the last pass.
    before: &'a [u8],
    /// The bytes at the end of the last pass.
    after: &'a [u8],
}

impl<'a> Relaxation<'a> {
    /// The relaxation of a net whose last pass took `before` to `after`.
    pub fn of(layout: &'a Layout<'a>, before: &'a [u8], after: &'a [u8]) -> Relaxation<'a> {
        Relaxation { layout, before, after }
    }

    /// The bytes the net settles to: `after` with the moving part of every field cleared (P3: no
    /// value). A plain known byte clears its whole element; a mask (`KNOWN_LEVELS`) clears exactly
    /// the levels whose value bit or flag moved and keeps the rest. `Err` names the first moving
    /// field that has no known flags, which has nothing to fall back to.
    pub fn settled(&self) -> Result<Vec<u8>, String> {
        let mut value = self.after.to_vec();
        for field in self.layout.fields.iter() {
            match FieldKnown::of(field) {
                Some(known) => {
                    for element in 0..known.count() {
                        known.clear_moving(self.before, self.after, &mut value, element);
                    }
                }
                None => {
                    let width = leaf_size(field.leaf).ok_or_else(|| field.name.as_str().to_string())?;
                    let at = field.offset as usize..field.offset as usize + width * field.count as usize;
                    if self.before[at.clone()] != self.after[at] {
                        return Err(field.name.as_str().to_string());
                    }
                }
            }
        }
        Ok(value)
    }
}
