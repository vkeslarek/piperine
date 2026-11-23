//! `CarrierArray`: an array of carriers as a reaction reads it (composite-values CV-24, CV-25).
//!
//! A `WHOLE` read cell of an array pin holds an `ArrayView` over the microstep's snapshot (abi.md §3.1), so an array of
//! `1 << 32` elements is never copied into a frame: `get`, `previous` and `changed` read one element through the
//! view, and `whole` builds the array value on the heap. An index outside the array is a panic of the model's own
//! values (§1 rule 11) naming the carrier and the index; the owner adds the instance and the instant.

use std::marker::PhantomData;

use piperine_abi::raw::{ArrayView, part};

use super::control::Panic;
use super::pin::Carried;

/// An element index checked against the number of elements.
pub struct ElementIndex;

impl ElementIndex {
    /// The element `index` names in `count` elements of `carrier`; outside them the panic naming both.
    pub fn of(index: i128, count: u64, carrier: &str) -> u64 {
        if index < 0 || index as u128 >= count as u128 {
            Panic::raise(format!("{carrier}: the element {index} is out of range for an array of {count} carriers"));
        }
        index as u64
    }
}

/// The elements of one array pin a reaction reads, of carried type `T`.
pub struct CarrierArray<T: Carried> {
    view: ArrayView,
    /// The carrier's MSL path, named by a panic (R3).
    carrier: &'static str,
    element: PhantomData<T>,
}

impl<T: Carried> CarrierArray<T> {
    /// The array behind `view`, the pin of `carrier`.
    pub fn new(view: ArrayView, carrier: &'static str) -> CarrierArray<T> {
        CarrierArray { view, carrier, element: PhantomData }
    }

    /// The number of elements.
    pub fn count(&self) -> u64 {
        self.view.count
    }

    /// The bytes of `part` of element `index`, after the bound check.
    fn bytes(&self, index: i128, part: u32) -> &[u8] {
        let element = ElementIndex::of(index, self.view.count, self.carrier);
        match self.view.element(element, part) {
            Some(bytes) => bytes,
            None => Panic::raise(format!("{}: the element {element} could not be read", self.carrier)),
        }
    }

    /// `x[index]`: the value of one element.
    pub fn get(&self, index: i128) -> T {
        T::from_pin(self.bytes(index, part::VALUE))
    }

    /// `x[index].prev()`: the value an element had before its last change.
    pub fn previous(&self, index: i128) -> T {
        T::from_pin(self.bytes(index, part::PREVIOUS))
    }

    /// `x[index].changed()`: whether the element changed in this microstep.
    pub fn changed(&self, index: i128) -> bool {
        self.bytes(index, part::CHANGED).first().is_some_and(|flag| *flag != 0)
    }

    /// `x` read whole: every element, as an array value of `K` elements, built on the heap. A pin of another
    /// element count is a panic.
    pub fn whole<const K: usize>(&self) -> Box<[T; K]> {
        if self.view.count != K as u64 {
            Panic::raise(format!("{}: an array of {} carriers read as an array of {K}", self.carrier, self.view.count));
        }
        let values: Vec<T> = (0..K as i128).map(|index| self.get(index)).collect();
        match values.into_boxed_slice().try_into() {
            Ok(array) => array,
            Err(_) => Panic::raise(format!("{}: the elements do not make an array of {K}", self.carrier)),
        }
    }
}

#[cfg(test)]
mod tests {
    use piperine_abi::caller::{ElementSource, ViewHandle};
    use piperine_abi::raw::part;

    use super::{CarrierArray, ElementIndex};
    use crate::rt::int::UInt;
    use crate::rt::commits::Commits;
    use crate::rt::commits::tests::ElementsMade;

    /// Four one-byte elements `[1, 0, 1, 1]` that were `[0, 0, 0, 1]` before the last microstep.
    struct Rows;

    impl ElementSource for Rows {
        fn count(&self) -> u64 {
            4
        }
        fn size(&self) -> u32 {
            1
        }
        fn element(&self, element: u64, part: u32) -> &[u8] {
            const NOW: [[u8; 1]; 4] = [[1], [0], [1], [1]];
            const BEFORE: [[u8; 1]; 4] = [[0], [0], [0], [1]];
            const CHANGED: [[u8; 1]; 4] = [[1], [0], [1], [0]];
            let rows = match part {
                part::VALUE => &NOW,
                part::PREVIOUS => &BEFORE,
                _ => &CHANGED,
            };
            &rows[element as usize]
        }
    }

    #[test]
    fn carrier_array_index_past_the_end_panics_naming_it() {
        // §5.1.7 / §1 rule 11: an element index outside the array names the carrier and the index.
        let handle = ViewHandle::new(&Rows);
        let array = CarrierArray::<bool>::new(handle.view(), "Top.mem");
        assert_eq!([array.get(0), array.get(1), array.get(3)], [true, false, true], "each element is read");
        assert_eq!([array.previous(0), array.previous(2)], [false, false], "prev() is the value before the change");
        assert_eq!([array.changed(0), array.changed(1)], [true, false], "changed() is per element");
        assert_eq!(*array.whole::<4>(), [true, false, true, true], "read whole, the array of its elements' values");
        for (index, message) in [(4, "the element 4 is out of range for an array of 4"), (-1, "the element -1")] {
            let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| array.get(index))).expect_err("past the end");
            let text = payload.downcast_ref::<String>().expect("a String payload");
            assert!(text.contains("Top.mem") && text.contains(message), "{text}");
        }
        assert_eq!(ElementIndex::of(3, 4, "Top.mem"), 3, "the last element is in range");
    }

    #[test]
    fn element_commits_reach_the_host_with_their_element() {
        // CV-25: `mem[i] <- v` is a commit of element `i`; the same bound check applies before the host sees it.
        let made = ElementsMade::flushed(|commits: &mut Commits| {
            commits.commit_element(1, 2, 4, "Top.mem", true, 0.0);
            commits.commit_element(1, 0, 4, "Top.mem", false, 1e-9);
            commits.commit_range(0, 3, UInt::<8>::new(7), 0.0);
        });
        assert_eq!(made, [(1, Some(2), 0, vec![1], 0.0), (1, Some(0), 0, vec![0], 1e-9), (0, None, 3, vec![7], 0.0)]);
        let past = std::panic::catch_unwind(|| {
            let mut commits = Commits::new();
            commits.commit_element(1, 4, 4, "Top.mem", true, 0.0);
        });
        assert!(past.is_err(), "element 4 of 4 is refused before the host sees it");
    }
}
