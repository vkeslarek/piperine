//! One array net (composite-values CV-27, part II §5.1.8, abi.md §3.1 P4a): `count` elements of one layout whose
//! storage is proportional to the elements written, never to `count`.
//!
//! An element no commit has written reads the array's initial bytes and its `PREVIOUS` is the same bytes; it is
//! `CHANGED` only in the microstep of the initial commit (part IV §13 rule 6), which stamps the array once instead of
//! materializing every element. A written element keeps its value, the value before its last change, and the
//! microstep of that change.

use std::collections::BTreeMap;

/// What every element of an array holds before it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArrayInit {
    /// One element's bytes, repeated: `[x; K]`.
    Repeat(Vec<u8>),
    /// Every element's bytes, one after the other: `[a, b, c]`.
    Each(Vec<u8>),
}

impl ArrayInit {
    /// The init a net of `size`-byte elements carries in `bytes`: one element's bytes repeat, `count` elements' bytes
    /// are each given, and nothing is all zero (abi.md §3.1 P3).
    pub fn of(bytes: &[u8], size: u32) -> ArrayInit {
        match bytes.len() {
            0 => ArrayInit::Repeat(vec![0; size as usize]),
            n if n == size as usize => ArrayInit::Repeat(bytes.to_vec()),
            _ => ArrayInit::Each(bytes.to_vec()),
        }
    }
}

/// One element that has been written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementState {
    value: Vec<u8>,
    previous: Vec<u8>,
    /// The microstep of the last change.
    changed_at: u64,
}

/// What a write of one element replaced, kept to undo it (HYG-14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ElementUndo {
    /// The net of the array.
    pub(crate) net: u32,
    /// The element written.
    pub(crate) element: u64,
    /// The element before the write; `None` when it had never been written.
    before: Option<ElementState>,
}

/// One array net: its shape, its initial value and the elements written so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SparseArray {
    count: u64,
    size: u32,
    init: ArrayInit,
    written: BTreeMap<u64, ElementState>,
    /// The microstep the initial commit lands in: every element not written since reads as changed there.
    initial_changed_at: u64,
}

impl SparseArray {
    /// An array of `count` elements of `size` bytes, none written.
    pub fn new(count: u64, size: u32, init: ArrayInit) -> SparseArray {
        SparseArray { count, size, init, written: BTreeMap::new(), initial_changed_at: 0 }
    }

    /// The number of elements.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// The bytes of one element.
    pub fn size(&self) -> u32 {
        self.size
    }

    /// How many elements have been written: what the storage is proportional to.
    pub fn written_len(&self) -> usize {
        self.written.len()
    }

    /// The initial bytes of `element`.
    pub fn initial(&self, element: u64) -> &[u8] {
        match &self.init {
            ArrayInit::Repeat(bytes) => bytes,
            ArrayInit::Each(bytes) => {
                let at = element as usize * self.size as usize;
                &bytes[at..at + self.size as usize]
            }
        }
    }

    /// `VALUE` of `element`: what was written last, else the initial bytes.
    pub fn value(&self, element: u64) -> &[u8] {
        self.written.get(&element).map_or_else(|| self.initial(element), |state| &state.value)
    }

    /// `PREVIOUS` of `element`: the bytes before its last change; an element never written reads its initial bytes.
    pub fn previous(&self, element: u64) -> &[u8] {
        self.written.get(&element).map_or_else(|| self.initial(element), |state| &state.previous)
    }

    /// `CHANGED` of `element` in microstep `step`.
    pub fn changed(&self, element: u64, step: u64) -> bool {
        self.written.get(&element).map_or(self.initial_changed_at == step, |state| state.changed_at == step)
    }

    /// Records the microstep the initial commit lands in (part IV §13 rule 6).
    pub(crate) fn stamp_initial(&mut self, step: u64) {
        self.initial_changed_at = step;
    }

    /// Writes `bytes` at `offset` of `element` in microstep `step`; returns whether the element's bytes differ
    /// afterwards. A change keeps the old bytes as `PREVIOUS`. Panics when the range leaves the element or the element
    /// is not below `count`.
    pub(crate) fn write(&mut self, net: u32, element: u64, offset: u32, bytes: &[u8], step: u64) -> Option<ElementUndo> {
        assert!(element < self.count, "element {element} of an array of {}", self.count);
        let at = offset as usize;
        let current = self.value(element);
        if current[at..at + bytes.len()] == *bytes {
            return None;
        }
        let mut value = current.to_vec();
        value[at..at + bytes.len()].copy_from_slice(bytes);
        let previous = current.to_vec();
        let before = self.written.insert(element, ElementState { value, previous, changed_at: step });
        Some(ElementUndo { net, element, before })
    }

    /// Puts an element back as `undo` found it.
    pub(crate) fn undo(&mut self, undo: ElementUndo) {
        match undo.before {
            Some(state) => {
                self.written.insert(undo.element, state);
            }
            None => {
                self.written.remove(&undo.element);
            }
        }
    }
}
