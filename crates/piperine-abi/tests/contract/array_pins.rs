//! Array pins (composite-values CV-27, abi.md §3.1, arrays of carriers): `count` 0 keeps every existing shape as it
//! was, an array pin validates its count and its `init` length, and a read names its element or `WHOLE`.

use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{ArrayView, Cell, Statement, WHOLE, cell, part, pin_flag, role};
use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, ShapeRef};
use piperine_abi::validate::{Rule, validate};

/// Builds the shapes this target validates: one `Bit` array pin, one reaction that reads it.
struct ArrayShapes;

impl ArrayShapes {
    /// A `count`-element array pin of `Bit` carrying `init` as its `HAS_INIT` bytes, and a reaction that reads
    /// `elements` of it (one `VALUE` read per entry) and writes one statement on it.
    fn shape(count: u64, init: &'static [u8], elements: Vec<u64>) -> ShapeRef {
        let mut buffer = ShapeBuf::new();
        let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
        buffer.pin(PinSpec { origin: "ram.msl:1:5", flags, init, count, ..PinSpec::discrete("mem", role::PORT, &BIT) });
        buffer.pin(PinSpec { origin: "ram.msl:2:5", ..PinSpec::event("tick", role::PRIVATE) });
        buffer.reaction(ReactionSpec {
            name: "write",
            origin: "ram.msl:3:5",
            on: vec![1],
            reads: elements.iter().map(|_| Cell { kind: cell::PIN, index: 0, part: part::VALUE }).collect(),
            writes: vec![Statement { pin: 0, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            elements,
            ..ReactionSpec::default()
        });
        buffer.freeze()
    }
}

#[test]
fn array_pins_validate_their_count_and_init() {
    // `count` 0 is one carrier: the shapes before arrays of carriers stay valid.
    let one = validate(ArrayShapes::shape(0, &[], vec![]).shape());
    assert!(one.is_ok(), "count 0 is one carrier: {one:?}");
    // §3.1: an init of one element (`[x; K]`) or of `count` elements (`[a, b, c]`).
    static ONE: [u8; 1] = [1];
    static EACH: [u8; 3] = [0, 1, 1];
    assert!(validate(ArrayShapes::shape(3, &ONE, vec![]).shape()).is_ok(), "one element repeats");
    assert!(validate(ArrayShapes::shape(3, &EACH, vec![]).shape()).is_ok(), "every element is given");
    let wrong = validate(ArrayShapes::shape(3, &EACH[..2], vec![]).shape()).expect_err("two bytes for three elements");
    assert_eq!(wrong.rule, Rule::ArrayInitLength);
    // an array pin is DISCRETE: `count` on a continuous pin is refused by name.
    let mut buffer = ShapeBuf::new();
    buffer.pin(PinSpec { origin: "a.msl:1:1", count: 4, ..PinSpec::continuous("v", role::PORT) });
    let continuous = validate(buffer.freeze().shape()).expect_err("a continuous array pin");
    assert_eq!((continuous.rule, continuous.entry.as_str()), (Rule::ArrayNotDiscrete, "pin v"));
    // a read names an element below `count`, or WHOLE; one at `count` is refused.
    assert!(validate(ArrayShapes::shape(4, &[], vec![0, 3, WHOLE]).shape()).is_ok());
    let past = validate(ArrayShapes::shape(4, &[], vec![4]).shape()).expect_err("element 4 of 4");
    assert_eq!((past.rule, past.entry.as_str()), (Rule::ElementOutOfRange, "reaction write"));
    // WHOLE on a pin that is no array is refused too.
    let scalar = validate(ArrayShapes::shape(0, &[], vec![WHOLE]).shape()).expect_err("WHOLE of one carrier");
    assert_eq!(scalar.rule, Rule::ElementOutOfRange);
    // an `elements` list of another length than `reads` is refused.
    let mut buffer = ShapeBuf::new();
    buffer.pin(PinSpec { count: 4, ..PinSpec::discrete("mem", role::PORT, &F64) });
    buffer.pin(PinSpec::event("tick", role::PRIVATE));
    buffer.reaction(ReactionSpec { name: "r", on: vec![1], elements: vec![0], ..ReactionSpec::default() });
    let lengths = validate(buffer.freeze().shape()).expect_err("one element for no read");
    assert_eq!(lengths.rule, Rule::ElementOutOfRange);
    assert_eq!(HIGH.len(), LOW.len(), "a BIT is one byte");
}

#[test]
fn a_whole_read_cell_holds_an_array_view() {
    // The cell of a WHOLE read is the view's own bytes (abi.md §3.1): a lens reads an element through it.
    let rows = [[0u8], [1], [0]];
    let view = ArrayView { context: rows.as_ptr().cast(), count: 3, size: 1, read: ArrayViews::read };
    let read_back = ArrayView::from_bytes(view.as_bytes()).expect("a view's bytes hold a view");
    assert_eq!(read_back.element(1, part::VALUE), Some(&[1u8][..]));
    assert_eq!(read_back.element(3, part::VALUE), None, "an element at count is past the end");
    assert!(ArrayView::from_bytes(&[0u8; 3]).is_none(), "too few bytes hold no view");
}

/// The element reader of the array views these tests build.
struct ArrayViews;

impl ArrayViews {
    /// The cell of a WHOLE read is the view's own bytes (abi.md §3.1): a lens reads an element through it.
    unsafe extern "C" fn read(data: *const std::ffi::c_void, element: u64, _part: u32) -> *const u8 {
        // SAFETY: `data` is the `[[u8; 1]; 3]` the test passed as the view's data and `element` is below 3.
        unsafe { data.cast::<[u8; 1]>().add(element as usize).cast::<u8>() }
    }
}
