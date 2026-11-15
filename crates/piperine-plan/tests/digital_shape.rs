//! The shapes of the digital and mixed bodies (feature `ir-rust`, RS-12): carriers as discrete
//! pins, drives and events as discrete functions, reactions and their statements, `cross`
//! watches and analog reads of carriers. Expected entries are written from spec RS-12 and
//! abi.md (§3.1 P3, §4, §5, §6, §11.4, §11.5), cited at each assertion; the bodies are the
//! hand-built buck slice of `piperine-ir/tests/support/digital.rs`.

use piperine_abi::layout::{BIT, BYTE, LOW, NONE, REAL, is_canonical};
use piperine_abi::raw::{Cell, Layout, Pin, Shape, cell, field_flag, kind, leaf, part, pin_flag, role};
use piperine_ir::{
    BodyId, BodySpec, CarrierRole, Constant, Event, EventSource, InstanceSpec, Ir, IrBuilder, Origin, Real, ValueType,
};
use piperine_plan::{BodyPlan, Mode, Source, Target};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use digital::{Buck, Dx, Ty};

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

/// Entries of a shape, found by name.
struct Find;

impl Find {
    fn pin(shape: &Shape<'_>, name: &str) -> u32 {
        shape.pins.iter().position(|p| p.name.as_str() == name).unwrap_or_else(|| panic!("no pin `{name}`")) as u32
    }

    fn function_names(shape: &Shape<'_>) -> Vec<String> {
        shape.functions.iter().map(|f| f.name.as_str().to_string()).collect()
    }

    /// Function `name`: its reads, its writes and its partials as `(write, read)`.
    fn function(shape: &Shape<'_>, name: &str) -> (Vec<Cell>, Vec<Cell>, Vec<(u32, u32)>) {
        let f = shape.functions.iter().find(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        (f.reads.iter().copied().collect(), f.writes.iter().copied().collect(), f.partials.iter().map(|p| (p.write, p.read)).collect())
    }

    /// Reaction `name`: `on`, its reads and its statements as `(pin, offset, len, flags,
    /// min_after)`.
    fn reaction(shape: &Shape<'_>, name: &str) -> (Vec<u32>, Vec<Cell>, Vec<(u32, u32, u32, u32, f64)>) {
        let r = shape.reactions.iter().find(|r| r.name.as_str() == name).unwrap_or_else(|| panic!("no reaction `{name}`"));
        (
            r.on.iter().copied().collect(),
            r.reads.iter().copied().collect(),
            r.writes.iter().map(|w| (w.pin, w.offset, w.len, w.flags, w.min_after)).collect(),
        )
    }
}

/// A cell of a pin.
fn pin(index: u32, part: u32) -> Cell {
    Cell { kind: cell::PIN, index, part }
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

#[test]
fn a_drive_is_a_discrete_function() {
    // RS-12 AC 2: `at_target = reference == target;` (control.msl:24) is one discrete function
    // reading the pins of its dependency set (`reference`, `target`) and writing the carrier's.
    let (ir, body) = Alone::design(Buck::ref_ramp);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (target, reference, at_target) = (Find::pin(shape, "target"), Find::pin(shape, "reference"), Find::pin(shape, "at_target"));
    let (reads, writes, partials) = Find::function(shape, "drive:at_target");
    assert_eq!(reads, [pin(target, part::VALUE), pin(reference, part::VALUE)], "reads the VALUE of each carrier it reaches");
    assert_eq!(writes, [pin(at_target, part::VALUE)], "writes the carrier's pin");
    assert_eq!(partials, [], "no real field: no partial");
    let f = shape.functions.iter().find(|f| f.name.as_str() == "drive:at_target").expect("drive:at_target");
    assert_eq!(f.origin.as_str(), "control.msl:24:5", "the drive's origin");
    let k = Find::function_names(shape).iter().position(|n| n == "drive:at_target").expect("its index");
    assert_eq!(plan.functions()[k].mode, Mode::Drive(0));
    assert_eq!(plan.functions()[k].writes, [Target::Carrier(4)]);
    // A discrete function writes a discrete pin (F2): no term, input or watch.
    assert!(shape.pins.get(at_target as usize).is_some_and(|p| p.kind == kind::DISCRETE));

    // The Dpwm's `sample = count == SAMPLE_COUNT;` (dpwm.msl:51) reads `count` alone.
    let (ir, body) = Alone::design(Buck::dpwm);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (reads, writes, _) = Find::function(shape, "drive:sample");
    assert_eq!(reads, [pin(Find::pin(shape, "count"), part::VALUE)]);
    assert_eq!(writes, [pin(Find::pin(shape, "sample"), part::VALUE)]);
}

#[test]
fn an_event_is_an_event_pin_and_its_function() {
    // abi.md §11.4, the D flip-flop:
    //   pins: clk, d (PORT BIT);  q (PORT BIT, init LOW);  rise (PRIVATE event)
    //   fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
    // The IR's `rise()` is `changed` guarded by `high()` (RS-12 AC 3: the function reads `VALUE`
    // and `CHANGED` of its carrier and the guard's pins); `PREVIOUS` is msl-digital's A7.
    let (ir, body) = Alone::design(Buck::dff);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let pins: Vec<(u32, u32, u32, Vec<u8>)> =
        shape.pins.iter().map(|p| (p.kind, p.role, p.flags, p.init.as_slice().to_vec())).collect();
    let (discrete, event, port, private) = (kind::DISCRETE, kind::EVENT, role::PORT, role::PRIVATE);
    assert_eq!(
        pins,
        [(discrete, port, 0, vec![]), (discrete, port, 0, vec![]), (discrete, port, pin_flag::HAS_INIT, LOW.to_vec()), (event, private, 0, vec![])],
        "clk, d, q (init LOW), rise"
    );
    for p in shape.pins.iter().take(3) {
        assert_eq!(layout(&p.layout), layout(&BIT), "{}: BIT", p.name.as_str());
    }
    let rise = shape.pins.get(3).expect("the event pin");
    assert_eq!(layout(&rise.layout), layout(&NONE), "an event pin holds nothing");
    assert_eq!(rise.name.as_str(), "event:0");
    assert_eq!(Find::function_names(shape), ["event:0"], "one function: the event's");
    let (reads, writes, partials) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(0, part::VALUE), pin(0, part::CHANGED)]);
    assert_eq!(writes, [pin(3, part::VALUE)], "writes the event pin");
    assert_eq!(partials, []);
    assert_eq!(plan.functions()[0].mode, Mode::Event(0));

    // The oscillator's bare `clk.changed()` (no guard) reads the same two cells of `clk`.
    let (ir, body) = Alone::design(Buck::osc);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let clk = Find::pin(shape, "clk");
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(clk, part::VALUE), pin(clk, part::CHANGED)]);
    assert_eq!(writes, [pin(Find::pin(shape, "event:0"), part::VALUE)]);

    // A guard reading another carrier and a param: `on clk.changed() if en.high() && k > 0.0`
    // reads the VALUE of `en` and the param `k` besides `clk`'s two cells.
    fn guarded(b: &mut IrBuilder) -> BodyId {
        let body = b.body(BodySpec { params: vec![Buck::param("k", "g.msl", 1, Some(1.0), None)], ..Buck::spec("Guarded", "g.msl", 1) });
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, "g.msl", 1, 20));
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, "g.msl", 1, 30));
        let mut x = Dx::new(b, "g.msl", 2, 22);
        let en = x.carrier(1, ValueType::Bool);
        let high = x.high(en);
        let k = x.param(0, ValueType::F64);
        let zero = x.real(0.0);
        let positive = x.bin(piperine_ir::BinaryOp::Gt, k, zero, ValueType::Bool);
        let guard = x.bin(piperine_ir::BinaryOp::And, high, positive, ValueType::Bool);
        b.event(body, Event { source: EventSource::Changed(0), guard: Some(guard), origin: Buck::origin("g.msl", 2, 8) });
        body
    }
    let (ir, body) = Alone::design(guarded);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (k, clk, en) = (Find::pin(shape, "k"), Find::pin(shape, "clk"), Find::pin(shape, "en"));
    let (reads, writes, _) = Find::function(shape, "event:0");
    let mut got = reads.clone();
    got.sort_by_key(|c| (c.index, c.part));
    let mut want = vec![pin(k, part::VALUE), pin(clk, part::VALUE), pin(clk, part::CHANGED), pin(en, part::VALUE)];
    want.sort_by_key(|c| (c.index, c.part));
    assert_eq!(got, want, "the carrier's VALUE and CHANGED and every pin of the guard, once each");
    assert_eq!(writes, [pin(Find::pin(shape, "event:0"), part::VALUE)]);
    assert!(shape.pins.get(Find::pin(shape, "event:0") as usize).is_some_and(|p| p.kind == kind::EVENT && p.role == role::PRIVATE));
    assert_eq!(plan.functions()[0].reads.iter().filter(|r| matches!(r, Source::CarrierChanged(_))).count(), 1);
}

#[test]
fn a_delayed_drive_is_a_pin_an_event_and_a_reaction() {
    // RS-12 AC 5 on `hs_gate = (hs_cmd & !ls_cmd) after tpd;` (power.msl:76): a private pin
    // holding the undelayed value, a `changed` event on it, and a reaction committing that
    // value to `hs_gate` after `tpd`.
    let (ir, body) = Alone::design(Buck::gate_driver);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (tpd, hs_cmd, ls_cmd, hs_gate) = (Find::pin(shape, "tpd"), Find::pin(shape, "hs_cmd"), Find::pin(shape, "ls_cmd"), Find::pin(shape, "hs_gate"));
    let now = Find::pin(shape, "drive:hs_gate.now");
    let changed = Find::pin(shape, "drive:hs_gate.changed");
    let now_pin = shape.pins.get(now as usize).expect("the undelayed pin");
    assert_eq!((now_pin.kind, now_pin.role, now_pin.flags), (kind::DISCRETE, role::PRIVATE, 0), "a private discrete pin");
    assert_eq!(layout(&now_pin.layout), layout(&BIT), "in the carrier's layout");
    let event_pin = shape.pins.get(changed as usize).expect("the event pin");
    assert_eq!((event_pin.kind, event_pin.role), (kind::EVENT, role::PRIVATE));

    // The drive's function writes the undelayed pin, not the carrier.
    let (reads, writes, _) = Find::function(shape, "drive:hs_gate");
    assert_eq!(reads, [pin(hs_cmd, part::VALUE), pin(ls_cmd, part::VALUE)]);
    assert_eq!(writes, [pin(now, part::VALUE)]);
    // The `changed` event on it: VALUE and CHANGED of the undelayed pin (RS-12 AC 3).
    let (reads, writes, _) = Find::function(shape, "drive:hs_gate.changed");
    assert_eq!(reads, [pin(now, part::VALUE), pin(now, part::CHANGED)]);
    assert_eq!(writes, [pin(changed, part::VALUE)]);
    // The reaction: on that event, reading the undelayed value and what the delay reads (`tpd`),
    // one statement on the whole of `hs_gate` (2 bytes, `BIT`); `tpd` is no literal: no `LATER`.
    let (on, reads, statements) = Find::reaction(shape, "drive:hs_gate");
    assert_eq!(on, [changed]);
    let mut got = reads.clone();
    got.sort_by_key(|c| c.index);
    assert_eq!(got, [pin(tpd, part::VALUE), pin(now, part::VALUE)]);
    assert_eq!(statements, [(hs_gate, 0, 2, 0, 0.0)]);
    assert_eq!(shape.reactions.len(), 2, "one reaction per delayed drive");
    assert!(plan.reactions().iter().all(|r| r.statements.len() == 1), "one statement each");

    // Nothing writes `hs_gate` but that reaction: no function writes the carrier's pin (P1).
    assert!(shape.functions.iter().all(|f| f.writes.iter().all(|w| w.index != hs_gate || w.kind != cell::PIN)));
}
