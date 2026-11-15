//! The shapes of the digital and mixed bodies (feature `ir-rust`, RS-12): carriers as discrete
//! pins, drives and events as discrete functions, reactions and their statements, `cross`
//! watches and analog reads of carriers. Expected entries are written from spec RS-12 and
//! abi.md (§3.1 P3, §4, §5, §6, §11.4, §11.5), cited at each assertion; the bodies are the
//! hand-built buck slice of `piperine-ir/tests/support/digital.rs`.

use piperine_abi::layout::{BIT, BYTE, REAL, is_canonical};
use piperine_abi::raw::{Layout, Pin, field_flag, kind, leaf, pin_flag, role};
use piperine_ir::{BodyId, BodySpec, CarrierRole, Constant, InstanceSpec, Ir, IrBuilder, Origin, Real, ValueType};
use piperine_plan::BodyPlan;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use digital::{Buck, Ty};

/// One body alone in a design, under an empty top body, as the IR's own tests place it.
struct Alone;

impl Alone {
    fn origin(file: &str, line: u32, col: u32) -> Origin {
        Origin { file: file.to_string(), line, col }
    }

    /// The design holding the body `make` builds, which must pass `check()`.
    fn design(make: fn(&mut IrBuilder) -> BodyId) -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: Alone::origin("top.msl", 1, 1),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.instance(InstanceSpec {
            path: "Top".into(),
            name: "Top".into(),
            body: top_body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: Alone::origin("top.msl", 1, 1),
        });
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        (ir, body)
    }
}

/// A pin as the assertions read it.
#[derive(Debug, PartialEq)]
struct PinView {
    name: String,
    kind: u32,
    role: u32,
    flags: u32,
    init: Vec<u8>,
    origin: String,
}

impl PinView {
    fn of(p: &Pin<'_>) -> PinView {
        PinView {
            name: p.name.as_str().to_string(),
            kind: p.kind,
            role: p.role,
            flags: p.flags,
            init: p.init.as_slice().to_vec(),
            origin: p.origin.as_str().to_string(),
        }
    }
}

/// A layout as the assertions read it: size, align and, per field, name, leaf, offset, count,
/// flags and known offset.
fn layout(l: &Layout<'_>) -> (u32, u32, Vec<(String, u32, u32, u32, u32, u32)>) {
    (l.size, l.align, l.fields.iter().map(|f| (f.name.as_str().to_string(), f.leaf, f.offset, f.count, f.flags, f.known)).collect())
}

/// The layout RS-12 AC 1 gives an integer: one field `v` of `leaf` at 0, its known byte right
/// after the value, the size rounded to the value's alignment (as `BIT`, `BYTE`, `REAL`).
fn integer(leaf: u32, width: u32) -> (u32, u32, Vec<(String, u32, u32, u32, u32, u32)>) {
    (2 * width, width, vec![("v".to_string(), leaf, 0, 1, field_flag::HAS_KNOWN, width)])
}

#[test]
fn carriers_are_discrete_pins_with_a_known_byte() {
    // A body with one carrier of each width class, built for this test: names are the type.
    fn widths(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Widths", "widths.msl", 1));
        let carriers: [(&str, ValueType, CarrierRole, Option<Constant>); 16] = [
            ("b", ValueType::Bool, CarrierRole::In, None),
            ("r", ValueType::F64, CarrierRole::Out, Some(Constant::Real(Real::new(1.5)))),
            ("u1", Ty::u(1), CarrierRole::Out, Some(Constant::Int(1))),
            ("u8", Ty::u(8), CarrierRole::Signal, Some(Constant::Int(255))),
            ("u9", Ty::u(9), CarrierRole::Signal, Some(Constant::Int(300))),
            ("u16", Ty::u(16), CarrierRole::Signal, None),
            ("u17", Ty::u(17), CarrierRole::Signal, None),
            ("u32", Ty::u(32), CarrierRole::Signal, None),
            ("u33", Ty::u(33), CarrierRole::Signal, None),
            ("u64", Ty::u(64), CarrierRole::Signal, Some(Constant::Int(u64::MAX as i128))),
            ("i1", Ty::i(1), CarrierRole::Signal, Some(Constant::Int(-1))),
            ("i8", Ty::i(8), CarrierRole::Signal, Some(Constant::Int(-128))),
            ("i9", Ty::i(9), CarrierRole::Signal, Some(Constant::Int(-2))),
            ("i32", Ty::i(32), CarrierRole::Signal, None),
            ("i33", Ty::i(33), CarrierRole::Signal, None),
            ("i64", Ty::i(64), CarrierRole::Signal, Some(Constant::Int(i64::MIN as i128))),
        ];
        for (k, (name, ty, role, init)) in carriers.into_iter().enumerate() {
            b.carrier(body, Buck::carrier(name, ty, role, init, "widths.msl", 2 + k as u32, 5));
        }
        body
    }
    let (ir, body) = Alone::design(widths);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let pins: Vec<&Pin<'_>> = shape.pins.iter().collect();
    assert_eq!(pins.len(), 16, "one pin per carrier, nothing else");

    // RS-12 AC 1: the layouts. `BIT`, `REAL` and `BYTE` are abi.md P3's shipped layouts; the
    // rest is the narrowest leaf that holds N bits, with its known byte.
    let expected = [
        layout(&BIT),
        layout(&REAL),
        layout(&BYTE),
        layout(&BYTE),
        integer(leaf::U16, 2),
        integer(leaf::U16, 2),
        integer(leaf::U32, 4),
        integer(leaf::U32, 4),
        integer(leaf::U64, 8),
        integer(leaf::U64, 8),
        integer(leaf::I8, 1),
        integer(leaf::I8, 1),
        integer(leaf::I16, 2),
        integer(leaf::I32, 4),
        integer(leaf::I64, 8),
        integer(leaf::I64, 8),
    ];
    for (p, want) in pins.iter().zip(&expected) {
        assert_eq!(&layout(&p.layout), want, "pin {}: layout", p.name.as_str());
        // a `DISCRETE` pin (RS-12 AC 1)
        assert_eq!(p.kind, kind::DISCRETE, "pin {}: kind", p.name.as_str());
    }

    // RS-12 AC 1: `PORT` for `in` and `out`, `PRIVATE` for a signal; declaration order.
    let names: Vec<&str> = pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["b", "r", "u1", "u8", "u9", "u16", "u17", "u32", "u33", "u64", "i1", "i8", "i9", "i32", "i33", "i64"]);
    assert_eq!(pins[0].role, role::PORT);
    assert_eq!(pins[1].role, role::PORT);
    assert_eq!(pins[2].role, role::PORT);
    assert!(pins[3..].iter().all(|p| p.role == role::PRIVATE));

    // RS-12 AC 1: an initial value gives `HAS_INIT` with its bytes; abi.md P3: the value
    // little-endian, the known byte 1, every other byte 0. No initial value: no flag, no bytes.
    let init = |i: usize| pins[i].init.as_slice().to_vec();
    let mut real = 1.5f64.to_le_bytes().to_vec();
    real.extend([1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(init(1), real, "r = 1.5");
    assert_eq!(init(2), [1, 1], "u1 = 1");
    assert_eq!(init(3), [255, 1], "u8 = 255");
    assert_eq!(init(4), [44, 1, 1, 0], "u9 = 300 = 0x012C");
    assert_eq!(init(9), [255, 255, 255, 255, 255, 255, 255, 255, 1, 0, 0, 0, 0, 0, 0, 0], "u64 = 2^64 - 1");
    assert_eq!(init(10), [255, 1], "i1 = -1 in two's complement, one byte");
    assert_eq!(init(11), [128, 1], "i8 = -128");
    assert_eq!(init(12), [254, 255, 1, 0], "i9 = -2 in an I16");
    assert_eq!(init(15), [0, 0, 0, 0, 0, 0, 0, 128, 1, 0, 0, 0, 0, 0, 0, 0], "i64 = -2^63");
    for (i, p) in pins.iter().enumerate() {
        let given = !p.init.is_empty();
        assert_eq!(p.flags & pin_flag::HAS_INIT != 0, given, "pin {}: HAS_INIT exactly when it has an initial value", p.name.as_str());
        assert_eq!(p.flags & !pin_flag::HAS_INIT, 0, "pin {}: no other flag", p.name.as_str());
        if given {
            assert_eq!(p.init.len(), p.layout.size as usize, "pin {}: init fills the layout", p.name.as_str());
            assert!(is_canonical(&p.layout, p.init.as_slice()), "pin {}: canonical bytes (P3)", p.name.as_str());
        }
        assert_eq!(p.origin.as_str(), format!("widths.msl:{}:5", 2 + i), "pin {}: origin of the carrier", p.name.as_str());
    }
    assert!([0, 5, 6, 7, 8, 13, 14].iter().all(|i| pins[*i].init.is_empty()));

    // Carrier pins come after the M1 pins, and every M1 pin keeps its index: in the switch
    // (params `ron`, `roff` with a `where` each, ports `p`, `n`, carrier `g`) the M1 order of
    // `plan.rs` is params, nodes, check pins; `g` follows them.
    let (ir, body) = Alone::design(Buck::switch);
    let plan = BodyPlan::new(ir.body(body));
    let pins: Vec<PinView> = plan.shape().shape().pins.iter().map(PinView::of).collect();
    let names: Vec<&str> = pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["ron", "roff", "p", "n", "check:ron", "check:roff", "g"]);
    assert_eq!(pins[6], PinView { name: "g".into(), kind: kind::DISCRETE, role: role::PORT, flags: 0, init: vec![], origin: "switch.msl:1:52".into() });
    assert_eq!(layout(&plan.shape().shape().pins.get(6).expect("pin g").layout), layout(&BIT));

    // The Dpwm: ports first, then signals, each with its `= 0` or `= false`.
    let (ir, body) = Alone::design(Buck::dpwm);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let carrier_pins: Vec<PinView> = shape.pins.iter().filter(|p| p.kind == kind::DISCRETE && p.role != role::PARAM).map(PinView::of).collect();
    let summary: Vec<(&str, u32, Vec<u8>)> = carrier_pins.iter().map(|p| (p.name.as_str(), p.role, p.init.clone())).collect();
    let (port, private) = (role::PORT, role::PRIVATE);
    assert_eq!(
        summary,
        [
            ("clk", port, vec![]),
            ("run", port, vec![]),
            ("duty", port, vec![]),
            ("oc", port, vec![]),
            ("sink_oc", port, vec![]),
            ("hs", port, vec![0, 1]),
            ("ls", port, vec![0, 1]),
            ("sample", port, vec![]),
            ("cut", port, vec![0, 1]),
            ("count", private, vec![0, 0, 1, 0]),
            ("width", private, vec![0, 0, 1, 0]),
            ("frame", private, vec![0, 1]),
            ("hs_off", private, vec![0, 0, 1, 0]),
            ("hs_cut", private, vec![0, 1]),
            ("ls_cut", private, vec![0, 1]),
        ]
    );
}
