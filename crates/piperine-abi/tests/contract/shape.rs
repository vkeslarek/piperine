//! The owned shape builder (feature `solver-abi-core`, SAC-07): a shape built
//! with `ShapeBuf` reads back through its raw pointers, its lists stay valid
//! until the last `ShapeRef` is dropped, and a `ShapeRef` may cross threads.

use std::sync::Arc;

use piperine_abi::layout;
use piperine_abi::raw::{Cell, Partial, Shape, cell, kind, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};

#[test]
fn resistor_shape_reads_back_through_raw_pointers() {
    let frozen = ShapeReading::resistor();
    let shape = ShapeReading::through_raw(&frozen);
    assert_eq!(shape.size as usize, size_of::<Shape>());

    let pins: Vec<(&str, &str, u32, u32)> =
        shape.pins.iter().map(|p| (p.name.as_str(), p.origin.as_str(), p.kind, p.role)).collect();
    assert_eq!(pins, [
        ("r", "resistor.msl:2:5", kind::DISCRETE, role::PARAM),
        ("p", "resistor.msl:1:17", kind::CONTINUOUS, role::PORT),
        ("n", "resistor.msl:1:20", kind::CONTINUOUS, role::PORT),
    ]);
    let r = shape.pins.get(0).unwrap();
    assert_eq!(r.init.as_slice(), 1e3f64.to_le_bytes());
    assert_eq!(r.flags, pin_flag::HAS_INIT);
    assert_eq!(r.unit.as_str(), "Ohm");
    assert!(layout::layout_eq(&r.layout, &layout::F64));
    assert_eq!(r.layout.fields.get(0).unwrap().name.as_str(), "v", "the layout's names are copied");
    let p = shape.pins.get(1).unwrap();
    assert!(layout::layout_eq(&p.layout, &layout::F64), "a continuous pin is one F64");

    assert_eq!(shape.terms.len(), 1);
    let t0 = shape.terms.get(0).unwrap();
    assert_eq!((t0.name.as_str(), t0.origin.as_str(), t0.op, t0.plus, t0.minus), ("i", "resistor.msl:3:5", op::FLOW, 1, 2));

    assert_eq!(shape.functions.len(), 1);
    let f = shape.functions.get(0).unwrap();
    assert_eq!((f.name.as_str(), f.origin.as_str(), f.flags), ("i", "resistor.msl:3:5", 0));
    let reads: Vec<Cell> = f.reads.iter().copied().collect();
    assert_eq!(reads, [ShapeReading::pin_value(1), ShapeReading::pin_value(2), ShapeReading::pin_value(0)]);
    let writes: Vec<Cell> = f.writes.iter().copied().collect();
    assert_eq!(writes, [Cell { kind: cell::TERM, index: 0, part: part::VALUE }]);
    let partials: Vec<Partial> = f.partials.iter().copied().collect();
    assert_eq!(partials, [
        Partial { write: 0, to: 0, read: 0, at: 0 },
        Partial { write: 0, to: 0, read: 1, at: 0 },
        Partial { write: 0, to: 0, read: 2, at: 0 },
    ]);

    assert!(shape.inputs.is_empty() && shape.watches.is_empty() && shape.reactions.is_empty());
    assert!(!shape.inputs.as_ptr().is_null() && !shape.reactions.as_ptr().is_null(), "empty lists are non-null");
}

#[test]
fn shape_outlives_its_builder_scope() {
    let (kept, raw) = {
        let first = ShapeReading::resistor();
        let clones: Vec<ShapeRef> = (0..4).map(|_| Arc::clone(&first)).collect();
        let raw = first.as_raw();
        let kept = Arc::clone(&clones[2]);
        drop(clones);
        drop(first);
        (kept, raw)
    };
    assert_eq!(Arc::strong_count(&kept), 1, "every other ShapeRef is dropped");
    assert_eq!(kept.as_raw(), raw, "the shape did not move");
    // SAFETY: SAC-07 AC 7: one `ShapeRef` (`kept`) still lives, so the pointer
    // taken from a dropped clone still points to the same live shape.
    let shape = unsafe { &*raw };
    let names: Vec<&str> = shape.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["r", "p", "n"]);
    let f = shape.functions.get(0).unwrap();
    assert_eq!(f.reads.len(), 3);
    assert_eq!(f.partials.get(2).unwrap().read, 2);
}

#[test]
fn shape_ref_is_send_and_sync() {
    ShapeReading::assert_send_sync::<ShapeRef>();
    let shape = ShapeReading::resistor();
    let names = std::thread::spawn(move || shape.shape().pins.iter().map(|p| p.name.as_str().to_string()).collect::<Vec<_>>())
        .join()
        .unwrap();
    assert_eq!(names, ["r", "p", "n"]);
}

/// The shapes the shape tests read back.
pub struct ShapeReading;

impl ShapeReading {
    /// The pin value helper `pin_value`.
    fn pin_value(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// abi.md §11.1, with the `PARAM` pin first (P5):
    /// pins `r` (PARAM F64, default 1e3), `p`, `n` (PORT continuous);
    /// term T0 FLOW {p, n} "i"; fn 0 "i": reads [p, n, r] writes [T0]
    /// partials (0←0) (0←1) (0←2).
    fn resistor() -> ShapeRef {
        let default = 1e3f64.to_le_bytes();
        let mut s = ShapeBuf::new();
        let r = s.pin(PinSpec {
            origin: "resistor.msl:2:5",
            flags: pin_flag::HAS_INIT,
            init: &default,
            unit: "Ohm",
            ..PinSpec::discrete("r", role::PARAM, &layout::F64)
        });
        let p = s.pin(PinSpec { origin: "resistor.msl:1:17", ..PinSpec::continuous("p", role::PORT) });
        let n = s.pin(PinSpec { origin: "resistor.msl:1:20", ..PinSpec::continuous("n", role::PORT) });
        let t0 = s.term(TermSpec {
            name: "i",
            origin: "resistor.msl:3:5",
            op: op::FLOW,
            plus: p,
            minus: n,
            abstol: piperine_abi::raw::DEFAULT,
            max: piperine_abi::raw::DEFAULT,
        });
        s.function(FunctionSpec {
            name: "i",
            origin: "resistor.msl:3:5",
            reads: vec![ShapeReading::pin_value(p), ShapeReading::pin_value(n), ShapeReading::pin_value(r)],
            writes: vec![Cell { kind: cell::TERM, index: t0, part: part::VALUE }],
            partials: (0..3).map(|read| Partial { write: 0, to: 0, read, at: 0 }).collect(),
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// Reads a shape through the raw pointer `ShapeBuf::as_raw` returns.
    fn through_raw(shape: &ShapeRef) -> &Shape<'_> {
        // SAFETY: SAC-07 AC 7, the property under test: the pointer stays valid
        // while `shape` (a `ShapeRef`) lives, which outlives the returned borrow.
        unsafe { &*shape.as_raw() }
    }

    /// The assert send sync helper `assert_send_sync`.
    fn assert_send_sync<T: Send + Sync>() {}
}
