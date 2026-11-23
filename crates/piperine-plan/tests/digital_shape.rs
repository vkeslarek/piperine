//! The shapes of the digital and mixed bodies (feature `ir-rust`, RS-12): carriers as discrete
//! pins, drives and events as discrete functions, reactions and their statements, `cross`
//! watches and analog reads of carriers. Expected entries are written from spec RS-12 and
//! abi.md (§3.1 P3, §4, §5, §6, §11.4, §11.5), cited at each assertion; the bodies are the
//! hand-built buck slice of `piperine-ir/tests/support/digital.rs`.

use piperine_abi::layout::{BIT, BYTE, F64, LOW, NONE};
use piperine_abi::raw::{Cell, Layout, Pin, Shape, cell, global, kind, leaf, part, pin_flag, role};
use piperine_ir::{
    BodyId, BodySpec, CarrierRole, Constant, Event, EventSource, InstanceSpec, Ir, IrBuilder, Origin, Real, Stream, StreamField, ValueType,
};
use piperine_digital_solver::dag::{Dag, Step};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::plan::FnNode;
use piperine_plan::{BodyPlan, Mode, RecordLayout, Source, Target};

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

/// A layout as the assertions read it: size, align and, per field, name, leaf, offset and count.
fn layout(l: &Layout<'_>) -> (u32, u32, Vec<(String, u32, u32, u32)>) {
    (l.size, l.align, l.fields.iter().map(|f| (f.name.as_str().to_string(), f.leaf, f.offset, f.count)).collect())
}

/// The layout RS-12 AC 1 gives an integer: one field `v` of `leaf` at 0, the size the value's own
/// (as `BIT`, `BYTE`, `F64`).
fn integer(leaf: u32, width: u32) -> (u32, u32, Vec<(String, u32, u32, u32)>) {
    (width, width, vec![("v".to_string(), leaf, 0, 1)])
}

#[test]
fn an_option_carrier_pin_is_presence_then_payload() {
    // EU-16 AC 4 (C68), abi.md P3 and P5: the pin `piperine-plan` lays out for a carrier of `Option<u8>` is a `BOOL`
    // presence field `some` at 0 and the payload `value` (`U8`) at 1, two bytes with no field that holds a flag; an
    // `Option<f64>` is the presence byte then the eight bytes of the real
    fn options(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Options", "options.msl", 1));
        let carriers = [("o", ValueType::Option(Box::new(Ty::u(8)))), ("x", ValueType::Option(Box::new(ValueType::F64)))];
        for (k, (name, ty)) in carriers.into_iter().enumerate() {
            b.carrier(body, Buck::carrier(name, ty, CarrierRole::Signal, None, "options.msl", 2 + k as u32, 5));
        }
        body
    }
    let (ir, body) = Alone::design(options);
    let plan = BodyPlan::new(ir.body(body));
    let pins: Vec<&Pin<'_>> = plan.shape().shape().pins.iter().collect();
    assert_eq!(pins.len(), 2, "one pin per carrier");
    let some = ("some".to_string(), leaf::BOOL, 0, 1);
    assert_eq!(layout(&pins[0].layout), (2, 1, vec![some.clone(), ("value".to_string(), leaf::U8, 1, 1)]), "`Option<u8>`");
    assert_eq!(layout(&pins[1].layout), (9, 1, vec![some, ("value".to_string(), leaf::F64, 1, 1)]), "`Option<f64>`: the payload follows the presence byte");
    for pin in pins {
        assert_eq!(pin.kind, kind::DISCRETE, "pin {}", pin.name.as_str());
        assert!(pin.layout.fields.iter().all(|f| f.name.as_str() == "some" || f.name.as_str() == "value"), "pin {}: the value only", pin.name.as_str());
    }
}

#[test]
fn carriers_are_discrete_pins_that_hold_the_value_only() {
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

    // RS-12 AC 1: the layouts. `BIT`, `F64` and `BYTE` are abi.md P3's shipped layouts; the
    // rest is the narrowest leaf that holds N bits.
    let expected = [
        layout(&BIT),
        layout(&F64),
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
    // little-endian and nothing else. No initial value: no flag, no bytes.
    let init = |i: usize| pins[i].init.as_slice().to_vec();
    assert_eq!(init(1), 1.5f64.to_le_bytes(), "r = 1.5");
    assert_eq!(init(2), [1], "u1 = 1");
    assert_eq!(init(3), [255], "u8 = 255");
    assert_eq!(init(4), [44, 1], "u9 = 300 = 0x012C");
    assert_eq!(init(9), [255, 255, 255, 255, 255, 255, 255, 255], "u64 = 2^64 - 1");
    assert_eq!(init(10), [255], "i1 = -1 in two's complement, one byte");
    assert_eq!(init(11), [128], "i8 = -128");
    assert_eq!(init(12), [254, 255], "i9 = -2 in an I16");
    assert_eq!(init(15), [0, 0, 0, 0, 0, 0, 0, 128], "i64 = -2^63");
    for (i, p) in pins.iter().enumerate() {
        let given = !p.init.is_empty();
        assert_eq!(p.flags & pin_flag::HAS_INIT != 0, given, "pin {}: HAS_INIT exactly when it has an initial value", p.name.as_str());
        assert_eq!(p.flags & !pin_flag::HAS_INIT, 0, "pin {}: no other flag", p.name.as_str());
        if given {
            assert_eq!(p.init.len(), p.layout.size as usize, "pin {}: init fills the layout", p.name.as_str());
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
    let carrier_pins: Vec<PinView> =
        shape.pins.iter().filter(|p| p.kind == kind::DISCRETE && p.role != role::PARAM && p.role != role::GLOBAL).map(PinView::of).collect();
    assert!(!has_pin(shape, "resolution"), "no reaction decides on an unknown, so no `RESOLUTION` pin");
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
            ("hs", port, vec![0]),
            ("ls", port, vec![0]),
            ("sample", port, vec![]),
            ("cut", port, vec![0]),
            ("count", private, vec![0, 0]),
            ("width", private, vec![0, 0]),
            ("frame", private, vec![0]),
            ("hs_off", private, vec![0, 0]),
            ("hs_cut", private, vec![0]),
            ("ls_cut", private, vec![0]),
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
    assert_eq!(writes, [pin(3, part::CHANGED)], "fires the event pin: abi.md §4.1, an event pin is written through CHANGED");
    assert_eq!(partials, []);
    assert_eq!(plan.functions()[0].mode, Mode::Event(0));

    // The oscillator's bare `clk.changed()` (no guard) reads the same two cells of `clk`.
    let (ir, body) = Alone::design(Buck::osc);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let clk = Find::pin(shape, "clk");
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(clk, part::VALUE), pin(clk, part::CHANGED)]);
    assert_eq!(writes, [pin(Find::pin(shape, "event:0"), part::CHANGED)]);

    // A guard reading another carrier and a param: `on clk.changed() if en && k > 0.0`
    // reads the VALUE of `en` and the param `k` besides `clk`'s two cells.
    fn guarded(b: &mut IrBuilder) -> BodyId {
        let body = b.body(BodySpec { params: vec![Buck::param("k", "g.msl", 1, Some(1.0), None)], ..Buck::spec("Guarded", "g.msl", 1) });
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, "g.msl", 1, 20));
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, "g.msl", 1, 30));
        let mut x = Dx::new(b, "g.msl", 2, 22);
        let en = x.carrier(1, ValueType::Bool);
        let k = x.param(0, ValueType::F64);
        let zero = x.real(0.0);
        let positive = x.bin(piperine_ir::BinaryOp::Gt, k, zero, ValueType::Bool);
        let guard = x.bin(piperine_ir::BinaryOp::And, en, positive, ValueType::Bool);
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
    assert_eq!(writes, [pin(Find::pin(shape, "event:0"), part::CHANGED)]);
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
    assert_eq!(writes, [pin(changed, part::CHANGED)]);
    // The reaction: on that event, reading the undelayed value and what the delay reads (`tpd`),
    // one statement on the whole of `hs_gate` (2 bytes, `BIT`); `tpd` is no literal: no `LATER`.
    let (on, reads, statements) = Find::reaction(shape, "drive:hs_gate");
    assert_eq!(on, [changed]);
    let mut got = reads.clone();
    got.sort_by_key(|c| c.index);
    assert_eq!(got, [pin(tpd, part::VALUE), pin(now, part::VALUE)]);
    assert_eq!(statements, [(hs_gate, 0, 1, 0, 0.0)]);
    assert_eq!(shape.reactions.len(), 2, "one reaction per delayed drive");
    assert!(plan.reactions().iter().all(|r| r.statements.len() == 1), "one statement each");

    // Nothing writes `hs_gate` but that reaction: no function writes the carrier's pin (P1).
    assert!(shape.functions.iter().all(|f| f.writes.iter().all(|w| w.index != hs_gate || w.kind != cell::PIN)));
}

#[test]
fn a_reaction_lists_its_events_reads_and_statements() {
    // abi.md §11.4, reaction 0 of the D flip-flop: `on [rise]  reads [d]  writes [S0 {q}]`; the
    // statement commits the whole `BIT` of `q` (offset 0, 2 bytes) at the next microstep, so
    // without `LATER` (RS-12 AC 4).
    let (ir, body) = Alone::design(Buck::dff);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    assert_eq!(shape.reactions.len(), 1);
    let (on, reads, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!(on, [3], "on the event pin `rise`");
    assert_eq!(reads, [pin(1, part::VALUE)], "reads `d`");
    assert_eq!(statements, [(2, 0, 1, 0, 0.0)], "S0 {{q}}");
    assert_eq!(shape.reactions.get(0).map(|r| (r.origin.as_str(), r.flags)), Some(("dff.msl:2:5", 0)));

    // The Dpwm's `on clk.rise()` (dpwm.msl:56-70): it reads the carriers its MSL text names and
    // no other (not `clk`, whose edge is the event's, nor `sample` or `cut`), and has one
    // statement per commit in source order: count, width, hs_off, hs_cut, ls_cut, hs, ls, then
    // under `if wrap`, frame and cut. Pin order would put `hs`, `ls`, `cut` first.
    let (ir, body) = Alone::design(Buck::dpwm);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (on, reads, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!(on, [Find::pin(shape, "event:0")]);
    let mut read: Vec<&str> = reads
        .iter()
        .map(|c| {
            assert_eq!((c.kind, c.part), (cell::PIN, part::VALUE), "a reaction reads pin VALUEs");
            shape.pins.get(c.index as usize).map_or("?", |p| p.name.as_str())
        })
        .collect();
    read.sort_unstable();
    let mut named = ["count", "run", "duty", "frame", "width", "hs_cut", "hs", "oc", "ls_cut", "ls", "sink_oc", "hs_off"];
    named.sort_unstable();
    assert_eq!(read, named);
    let committed: Vec<&str> = statements.iter().map(|st| shape.pins.get(st.0 as usize).map_or("?", |p| p.name.as_str())).collect();
    assert_eq!(committed, ["count", "width", "hs_off", "hs_cut", "ls_cut", "hs", "ls", "frame", "cut"]);
    // each statement covers its carrier's whole pin: `u9` is 2 bytes, `u3` and `bool` 1
    let lens: Vec<u32> = statements.iter().map(|st| st.2).collect();
    assert_eq!(lens, [2, 2, 2, 1, 1, 1, 1, 1, 1]);
    assert!(statements.iter().all(|st| st.1 == 0 && st.3 == 0));
    let carriers: Vec<Target> = plan.reactions()[0].statements.iter().map(|st| st.target).collect();
    assert_eq!(carriers, [9, 10, 12, 13, 14, 5, 6, 11, 8].map(Target::Carrier), "the plan's statements name the carriers, in the same order");

    // RefRamp: three commits to `reference` in one `if`/`else if` chain are three statements.
    let (ir, body) = Alone::design(Buck::ref_ramp);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let reference = Find::pin(shape, "reference");
    let (_, reads, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!(reads, [pin(Find::pin(shape, "run"), part::VALUE), pin(Find::pin(shape, "target"), part::VALUE), pin(reference, part::VALUE)]);
    assert_eq!(statements, [(reference, 0, 2, 0, 0.0); 3]);
}

#[test]
fn an_if_lists_its_then_statement_before_its_else() {
    // RS-12 AC 4 "one statement per commit in source order": `then` is written before `else`, so
    // its commit is the earlier statement. The two arms commit to one carrier with different
    // literal delays, so each statement's `LATER` bound tells the arms apart (verification F-B3):
    //   on clk.changed() {
    //       if c { b <- 2 after 1n; } else { b <- 3 after 2n; }
    //   }
    fn arms(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Arms", "a.msl", 1));
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, "a.msl", 1, 20));
        b.carrier(body, Buck::carrier("c", ValueType::Bool, CarrierRole::In, Some(Constant::Bool(false)), "a.msl", 1, 30));
        b.carrier(body, Buck::carrier("b", Ty::u(8), CarrierRole::Out, None, "a.msl", 1, 40));
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin("a.msl", 2, 8) });
        let mut x = Dx::new(b, "a.msl", 3, 9);
        let c = x.carrier(1, ValueType::Bool);
        let two = x.int(2, Ty::u(8));
        let one_ns = x.real(1e-9);
        let then = x.commit(2, two, Some(one_ns));
        let three = x.int(3, Ty::u(8));
        let two_ns = x.real(2e-9);
        let otherwise = x.commit(2, three, Some(two_ns));
        let otherwise = x.block(vec![otherwise], None, ValueType::Unit);
        let branch = x.when(c, vec![then], Some(otherwise));
        let branch = x.stmt(branch);
        Buck::reaction(b, body, vec![changed], vec![branch], vec![], "a.msl", 2);
        body
    }
    let (ir, body) = Alone::design(arms);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let later = piperine_abi::raw::statement_flag::LATER;
    let b = Find::pin(shape, "b");
    // `b`'s `Bit<u8>` pin is 1 byte; statement 0 is the `then` commit (1 ns), statement 1 the
    // `else` commit (2 ns)
    let (_, _, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!(statements, [(b, 0, 1, later, 1e-9), (b, 0, 1, later, 2e-9)]);
    let mins: Vec<(Target, Option<f64>)> = plan.reactions()[0].statements.iter().map(|st| (st.target, st.min_after)).collect();
    assert_eq!(mins, [(Target::Carrier(2), Some(1e-9)), (Target::Carrier(2), Some(2e-9))]);
}

#[test]
fn only_a_literal_positive_delay_is_later() {
    // abi.md §6 C1 and RS-12 AC 4: `LATER` with `min_after` only when the delay is a literal
    // constant greater than 0. A body committing `q` six ways, and a drive with a literal delay:
    //   on clk.changed() {
    //       q <- x after 2n;  q <- x after 0.0;  q <- x after -1n;  q <- x;
    //       q <- x after t;   q <- x after 0.5 / t;
    //   }
    //   y = x after 3n;
    fn delays(b: &mut IrBuilder) -> BodyId {
        let body = b.body(BodySpec { params: vec![Buck::param("t", "d.msl", 1, Some(1e-9), None)], ..Buck::spec("Delays", "d.msl", 1) });
        b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, "d.msl", 1, 20));
        b.carrier(body, Buck::carrier("x", ValueType::Bool, CarrierRole::In, None, "d.msl", 1, 30));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, "d.msl", 1, 40));
        b.carrier(body, Buck::carrier("y", ValueType::Bool, CarrierRole::Out, None, "d.msl", 1, 50));
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin("d.msl", 2, 8) });
        let mut x = Dx::new(b, "d.msl", 3, 9);
        let mut stmts = Vec::new();
        for delay in 0..6 {
            let value = x.carrier(1, ValueType::Bool);
            let after = match delay {
                0 => Some(x.real(2e-9)),
                1 => Some(x.real(0.0)),
                2 => Some(x.real(-1e-9)),
                3 => None,
                4 => Some(x.param(0, ValueType::F64)),
                _ => {
                    let half = x.real(0.5);
                    let t = x.param(0, ValueType::F64);
                    Some(x.bin(piperine_ir::BinaryOp::Div, half, t, ValueType::F64))
                }
            };
            stmts.push(x.commit(2, value, after));
        }
        Buck::reaction(b, body, vec![changed], stmts, vec![], "d.msl", 2);
        let mut x = Dx::new(b, "d.msl", 5, 9);
        let value = x.carrier(1, ValueType::Bool);
        let after = x.real(3e-9);
        b.drive(body, piperine_ir::Drive { carrier: 3, value, after: Some(after), origin: Buck::origin("d.msl", 5, 5), element: None, path: vec![] });
        body
    }
    let (ir, body) = Alone::design(delays);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let later = piperine_abi::raw::statement_flag::LATER;
    let (_, _, statements) = Find::reaction(shape, "reaction:0");
    let flags: Vec<(u32, f64)> = statements.iter().map(|st| (st.3, st.4)).collect();
    assert_eq!(flags, [(later, 2e-9), (0, 0.0), (0, 0.0), (0, 0.0), (0, 0.0), (0, 0.0)]);
    let mins: Vec<Option<f64>> = plan.reactions()[0].statements.iter().map(|st| st.min_after).collect();
    assert_eq!(mins, [Some(2e-9), None, None, None, None, None]);
    // the delayed drive's reaction follows the same rule
    let (_, _, statements) = Find::reaction(shape, "drive:y");
    assert_eq!(statements, [(Find::pin(shape, "y"), 0, 1, later, 3e-9)]);
    // the oscillator's `after 0.5 / f` reads a param: no flag
    let (ir, body) = Alone::design(Buck::osc);
    let plan = BodyPlan::new(ir.body(body));
    let (_, _, statements) = Find::reaction(plan.shape().shape(), "reaction:0");
    assert_eq!(statements.iter().map(|st| st.3).collect::<Vec<_>>(), [0]);
}

#[test]
fn a_probing_reaction_reads_the_continuous_pin() {
    // RS-12 AC 8 on the Adc (sense.msl:28-33): `v(ain)` makes the continuous pin `ain` a read of
    // the reaction, beside the params its body reads (`vref`; `tconv`, `tstrobe` in the delays).
    let (ir, body) = Alone::design(Buck::adc);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let ain = Find::pin(shape, "ain");
    assert!(shape.pins.get(ain as usize).is_some_and(|p| p.kind == kind::CONTINUOUS));
    let (on, reads, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!(on, [Find::pin(shape, "event:0")]);
    assert_eq!(
        reads,
        [pin(ain, part::VALUE), pin(Find::pin(shape, "vref"), part::VALUE), pin(Find::pin(shape, "tconv"), part::VALUE), pin(Find::pin(shape, "tstrobe"), part::VALUE)]
    );
    assert!(plan.reactions()[0].reads.contains(&Source::Node(0)));
    // code (a `u10`: 4 bytes), done, done: the three commits in source order
    let (code, done) = (Find::pin(shape, "code"), Find::pin(shape, "done"));
    assert_eq!(statements, [(code, 0, 2, 0, 0.0), (done, 0, 1, 0, 0.0), (done, 0, 1, 0, 0.0)]);
    // a reaction that probes nothing reads no continuous pin: the flip-flop's
    let (ir, body) = Alone::design(Buck::dff);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (_, reads, _) = Find::reaction(shape, "reaction:0");
    assert!(reads.iter().all(|c| shape.pins.get(c.index as usize).is_some_and(|p| p.kind != kind::CONTINUOUS)));
}

#[test]
fn a_cross_event_is_a_watch_with_partials() {
    // abi.md §11.5, the comparator: two `CROSS` watches, each firing its `PRIVATE` event pin,
    // written by an analog function with a partial against each continuous pin it reads; one
    // reaction per event, committing `HIGH` or `LOW` to `out` (init `LOW`) with no reads. The
    // buck's comparator is std `A2D` (convert.msl:9-13): one node `a` and the params `vth`,
    // `hyst` where §11.5 has `inp`, `inn`; one function per watch (F4: granularity is the
    // library's).
    let (ir, body) = Alone::design(Buck::comparator);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (a, vth, hyst, d) = (Find::pin(shape, "a"), Find::pin(shape, "vth"), Find::pin(shape, "hyst"), Find::pin(shape, "d"));
    let (up, down) = (Find::pin(shape, "event:0"), Find::pin(shape, "event:1"));
    let d_pin = shape.pins.get(d as usize).expect("d");
    assert_eq!((d_pin.kind, d_pin.role, d_pin.init.as_slice()), (kind::DISCRETE, role::PORT, &LOW[..]), "out (PORT BIT, init LOW)");
    for e in [up, down] {
        let p = shape.pins.get(e as usize).expect("event pin");
        assert_eq!((p.kind, p.role), (kind::EVENT, role::PRIVATE), "up, down (PRIVATE event)");
    }
    // watches: W0 CROSS -> up, W1 CROSS -> down; a CROSS watch holds one F64 (abi.md §3)
    let watches: Vec<(String, u32, u32, u32, u32)> =
        shape.watches.iter().map(|w| (w.name.as_str().to_string(), w.kind, w.event, w.ttol, w.etol)).collect();
    let (cross, default) = (piperine_abi::raw::watch::CROSS, piperine_abi::raw::DEFAULT);
    assert_eq!(watches, [("cross:0".into(), cross, up, default, default), ("cross:1".into(), cross, down, default, default)]);
    for w in shape.watches.iter() {
        assert_eq!(layout(&w.layout), layout(&piperine_abi::layout::F64));
    }
    // the analog functions: each writes its watch, with a partial against `a` only (the one
    // continuous pin of its set); `vth` and `hyst` are read, not differentiated
    let watch = |w: u32| Cell { kind: cell::WATCH, index: w, part: part::VALUE };
    for (k, w) in [(0, 0), (1, 1)] {
        let (reads, writes, partials) = Find::function(shape, &format!("cross:{k}"));
        assert_eq!(reads, [pin(a, part::VALUE), pin(vth, part::VALUE), pin(hyst, part::VALUE)]);
        assert_eq!(writes, [watch(w)]);
        assert_eq!(partials, [(0, 0)], "(0 <- a)");
    }
    assert_eq!(Find::function_names(shape), ["cross:0", "cross:1"], "no function writes an event pin: the watches do (P1)");
    assert_eq!(plan.functions()[0].mode, Mode::Cross(0));
    // reaction 0: on [up] writes [S0 {out}]; reaction 1: on [down] writes [S0 {out}]
    assert_eq!(Find::reaction(shape, "reaction:0"), (vec![up], vec![], vec![(d, 0, 1, 0, 0.0)]));
    assert_eq!(Find::reaction(shape, "reaction:1"), (vec![down], vec![], vec![(d, 0, 1, 0, 0.0)]));

    // A guarded `cross` (`on cross(v(a)) if en`): the watch fires a private event pin
    // `cross:0`, and the event function fires the event pin when that pin fired and the guard
    // holds (R1: the non-probing part of a guard is a discrete function).
    fn guarded(b: &mut IrBuilder) -> BodyId {
        let el = Buck::electrical(b);
        let body = b.body(BodySpec {
            nodes: vec![piperine_ir::BodyNode { name: "a".into(), nature: el, role: piperine_ir::NodeRole::Port, init: None, origin: Buck::origin("gc.msl", 1, 10) }],
            ..Buck::spec("GuardedCross", "gc.msl", 1)
        });
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, "gc.msl", 1, 30));
        let mut x = Dx::new(b, "gc.msl", 2, 12);
        let v = x.probe(piperine_ir::Terminal::Node(0), piperine_ir::Terminal::Ground);
        let en = x.carrier(0, ValueType::Bool);
        let guard = en;
        b.event(body, Event { source: EventSource::Cross { e: v, ttol: None, etol: None }, guard: Some(guard), origin: Buck::origin("gc.msl", 2, 8) });
        body
    }
    let (ir, body) = Alone::design(guarded);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (a, en, event, crossed) = (Find::pin(shape, "a"), Find::pin(shape, "en"), Find::pin(shape, "event:0"), Find::pin(shape, "cross:0"));
    assert!(shape.pins.get(crossed as usize).is_some_and(|p| p.kind == kind::EVENT && p.role == role::PRIVATE));
    assert_eq!(shape.watches.get(0).map(|w| w.event), Some(crossed), "the watch fires its own pin");
    assert_eq!(Find::function(shape, "cross:0"), (vec![pin(a, part::VALUE)], vec![watch(0)], vec![(0, 0)]));
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(en, part::VALUE), pin(crossed, part::CHANGED)]);
    assert_eq!(writes, [pin(event, part::CHANGED)]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn an_analog_read_of_a_carrier_is_a_read_of_terms() {
    // RS-12 AC 7 on the switch (`i(p, n) +> v(p, n) * if g { 1.0 / ron } else { 1.0 /
    // roff }`): `terms` reads the carrier `g`'s VALUE beside the nodes and params, with no
    // partial against it (a discrete value is no differentiable read); the term stays affine.
    let (ir, body) = Alone::design(Buck::switch);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (p, n, ron, roff, g) = (Find::pin(shape, "p"), Find::pin(shape, "n"), Find::pin(shape, "ron"), Find::pin(shape, "roff"), Find::pin(shape, "g"));
    let (reads, writes, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [pin(p, part::VALUE), pin(n, part::VALUE), pin(ron, part::VALUE), pin(roff, part::VALUE), pin(g, part::VALUE)]);
    assert_eq!(writes, [Cell { kind: cell::TERM, index: 0, part: part::VALUE }]);
    assert_eq!(partials, [(0, 0), (0, 1)], "against p and n only");
    let terms = plan.functions().iter().find(|f| f.mode == Mode::Terms).expect("terms");
    assert_eq!(terms.reads.last(), Some(&Source::Carrier(0)));
    assert_eq!(terms.slots, 2, "two tangent slots: p and n");
    assert_eq!(terms.affine, [true], "a carrier read is constant in the node potentials");
}

#[test]
fn every_buck_slice_shape_validates() {
    // RS-12 AC 9: the ten bodies of the buck slice, each planned alone, pass the ABI's
    // shape-local rules.
    let bodies: [(&str, fn(&mut IrBuilder) -> BodyId); 10] = [
        ("osc", Buck::osc),
        ("dpwm", Buck::dpwm),
        ("setpoint_reg", Buck::setpoint_reg),
        ("ref_ramp", Buck::ref_ramp),
        ("pid", Buck::pid),
        ("gate_driver", Buck::gate_driver),
        ("adc", Buck::adc),
        ("comparator", Buck::comparator),
        ("switch", Buck::switch),
        ("dff", Buck::dff),
    ];
    for (name, make) in bodies {
        let (ir, body) = Alone::design(make);
        let plan = BodyPlan::new(ir.body(body));
        let shape = plan.shape().shape();
        assert_eq!(piperine_abi::validate::validate(shape), Ok(()), "{name}");
        // the plan's tables run parallel to the shape's lists
        assert_eq!(plan.functions().len(), shape.functions.len(), "{name}: functions");
        assert_eq!(plan.reactions().len(), shape.reactions.len(), "{name}: reactions");
        // every event pin has exactly one writer (P1): a function or a watch
        for (i, p) in shape.pins.iter().enumerate() {
            if p.kind != kind::EVENT {
                continue;
            }
            let by_functions = shape.functions.iter().filter(|f| f.writes.iter().any(|w| w.kind == cell::PIN && w.index == i as u32)).count();
            let by_watches = shape.watches.iter().filter(|w| w.event == i as u32).count();
            assert_eq!(by_functions + by_watches, 1, "{name}: event pin {} has one writer", p.name.as_str());
        }
    }
}

#[test]
fn every_record_layout_holds_its_declared_fields_only() {
    // §19 rule 1 (EU-15 AC 1): a record layout is the stream's declared fields and no other; each
    // is a carrier (abi.md P3: the value byte, then its known byte) at the next offset its
    // alignment allows, the size rounded up to the largest alignment, so the last field of the
    // pin is the stream's last field, for a stream of any shape.
    let streams: [(&str, Vec<(&str, ValueType)>); 3] = [
        ("none", vec![]),
        ("level", vec![("level", ValueType::F64)]),
        ("mixed", vec![("code", ValueType::UInt(8)), ("ok", ValueType::Bool), ("count", ValueType::Int(32))]),
    ];
    let (ir, body) = Alone::design(|b| {
        let body = b.body(Buck::spec("Streams", "streams.msl", 1));
        for (name, fields) in [
            ("none", vec![]),
            ("level", vec![StreamField { name: "level".into(), ty: ValueType::F64 }]),
            (
                "mixed",
                vec![
                    StreamField { name: "code".into(), ty: ValueType::UInt(8) },
                    StreamField { name: "ok".into(), ty: ValueType::Bool },
                    StreamField { name: "count".into(), ty: ValueType::Int(32) },
                ],
            ),
        ] {
            b.stream(body, Stream { name: name.into(), fields });
        }
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    // (field names, the size, the alignment) of each stream's pin
    let want = [
        (vec![], 0, 1),
        // `level`: an `F64` at 0, 8 bytes
        (vec!["level"], 8, 8),
        // `code` a `BYTE` at 0, `ok` a `BIT` at 1, `count` an `i32` at 4 (4 bytes), 8 bytes
        (vec!["code", "ok", "count"], 8, 4),
    ];
    for ((name, _), (names, size, align)) in streams.iter().zip(want.clone()) {
        let p = shape.pins.get(Find::pin(shape, name) as usize).expect("the record pin");
        let fields: Vec<String> = p.layout.fields.iter().map(|f| f.name.as_str().to_string()).collect();
        assert_eq!(fields.iter().map(String::as_str).collect::<Vec<_>>(), names, "{name}: the declared fields and nothing after them");
        assert_eq!((p.layout.size, p.layout.align), (size, align), "{name}");
        assert_eq!(piperine_abi::validate::validate(shape), Ok(()), "{name}");
    }
    // the plan's own layout names the same size, one field per field of the stream
    for (stream, (_, size, _)) in ir.body(body).streams().iter().zip(want) {
        let layout = RecordLayout::of(stream).expect("a layout");
        assert_eq!(layout.size, size, "{}", stream.name);
        assert_eq!(layout.fields.len(), stream.fields.len(), "{}", stream.name);
    }
}

/// Bodies for the reads of the merge of unknown control: an `in go`, an `in en`, an `out q`, all
/// `Bit`, and one reaction on `go.changed()` that commits to `q`.
struct Merging;

impl Merging {
    const F: &'static str = "merging.msl";

    /// `module <name>(in go: Bit, in en: Bit, out q: Bit)`.
    fn body(b: &mut IrBuilder, name: &str) -> BodyId {
        let body = b.body(Buck::spec(name, Merging::F, 1));
        for (carrier, role, col) in [("go", CarrierRole::In, 14), ("en", CarrierRole::In, 28), ("q", CarrierRole::Out, 42)] {
            b.carrier(body, Buck::carrier(carrier, ValueType::Bool, role, None, Merging::F, 1, col));
        }
        body
    }

    /// `on go.changed() && <guard> { q <- <value>; }`.
    fn react(b: &mut IrBuilder, body: BodyId, guard: Option<piperine_ir::ExprId>, value: piperine_ir::ExprId) {
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard, origin: Buck::origin(Merging::F, 2, 8) });
        let commit = Dx::new(b, Merging::F, 3, 9).commit(2, value, None);
        Buck::reaction(b, body, vec![changed], vec![commit], vec![], Merging::F, 2);
    }

    /// `on go.changed() && <guard> { q <- true; }` with the guard `en` when `guarded`, or none.
    fn guarded(b: &mut IrBuilder, name: &str, guarded: bool) -> BodyId {
        let body = Merging::body(b, name);
        let mut x = Dx::new(b, Merging::F, 2, 20);
        let en = x.carrier(1, ValueType::Bool);
        let guard = guarded.then_some(en);
        let yes = Dx::new(b, Merging::F, 3, 14).boolean(true);
        Merging::react(b, body, guard, yes);
        body
    }

    /// `on go.changed() { q <- if en { true } else { false }; }`.
    fn deciding(b: &mut IrBuilder, name: &str) -> BodyId {
        let body = Merging::body(b, name);
        let mut x = Dx::new(b, Merging::F, 3, 14);
        let cond = x.carrier(1, ValueType::Bool);
        let (yes, no) = (x.boolean(true), x.boolean(false));
        let value = x.if_(cond, yes, no, ValueType::Bool);
        Merging::react(b, body, None, value);
        body
    }
}

/// Whether the shape has a pin called `name`.
fn has_pin(shape: &Shape<'_>, name: &str) -> bool {
    shape.pins.iter().any(|p| p.name.as_str() == name)
}

#[test]
fn a_guard_or_a_decision_adds_no_read_and_no_resolution_pin() {
    // §3.3, §13 rule 1: a value is never unknown, so a guard and a decision on a `bool` leave the reads a reaction
    // had: what its body reads and nothing of the guard, and no `RESOLUTION` pin
    for (name, make) in [
        ("Sure", (|b: &mut IrBuilder| Merging::guarded(b, "Sure", true)) as fn(&mut IrBuilder) -> BodyId),
        ("Plain", |b: &mut IrBuilder| Merging::guarded(b, "Plain", false)),
    ] {
        let (ir, body) = Alone::design(make);
        let plan = BodyPlan::new(ir.body(body));
        let shape = plan.shape().shape();
        assert_eq!(plan.reactions()[0].reads, [], "{name}");
        assert_eq!(Find::reaction(shape, "reaction:0").1, [], "{name}");
        assert!(!has_pin(shape, "resolution"), "{name}");
    }
    let (ir, body) = Alone::design(|b| Merging::deciding(b, "Known"));
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    assert_eq!(plan.reactions()[0].reads, [Source::Carrier(1)]);
    assert!(!has_pin(shape, "resolution"));
    assert_eq!(shape.pins.len(), 4, "go, en, q and the event pin");
}

/// Bodies with a `settle` event (§13 rule 17): an `in ready: Bit` and an `out q: Bit`.
struct Settling;

impl Settling {
    const F: &'static str = "settling.msl";

    /// `on settle { q <- true; }`, or `on settle && ready { q <- true; }` when `guarded`.
    fn body(b: &mut IrBuilder, guarded: bool) -> BodyId {
        let body = b.body(Buck::spec("Settling", Settling::F, 1));
        b.carrier(body, Buck::carrier("ready", ValueType::Bool, CarrierRole::In, None, Settling::F, 1, 14));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Settling::F, 1, 28));
        let mut x = Dx::new(b, Settling::F, 2, 20);
        let ready = x.carrier(0, ValueType::Bool);
        let guard = guarded.then_some(ready);
        let event = b.event(body, Event { source: EventSource::Settle, guard, origin: Buck::origin(Settling::F, 2, 8) });
        let yes = Dx::new(b, Settling::F, 3, 14).boolean(true);
        let commit = Dx::new(b, Settling::F, 3, 9).commit(1, yes, None);
        Buck::reaction(b, body, vec![event], vec![commit], vec![], Settling::F, 2);
        body
    }
}

#[test]
fn a_settle_event_reads_the_global_settle_pin() {
    // §13 rule 17, design D5: the event function of `settle` reads the `CHANGED` byte of the
    // `GLOBAL` `SETTLE` pin (abi.md §4) and writes the event's own pin; a guard adds its reads
    let (ir, body) = Alone::design(|b| Settling::body(b, false));
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let settle = Find::pin(shape, "settle");
    let view = shape.pins.get(settle as usize).expect("the settle pin");
    assert_eq!((view.kind, view.role, view.global), (kind::EVENT, role::GLOBAL, global::SETTLE));
    let event = Find::pin(shape, "event:0");
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(settle, part::CHANGED)], "an unguarded settle reads the settle pin alone");
    assert_eq!(writes, [pin(event, part::CHANGED)]);
    assert_eq!(plan.functions()[0].reads, [Source::Settle]);
    assert_eq!(Find::reaction(shape, "reaction:0").0, vec![event], "the reaction listens to the event pin, never to SETTLE");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));

    let (ir, body) = Alone::design(|b| Settling::body(b, true));
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (ready, settle) = (Find::pin(shape, "ready"), Find::pin(shape, "settle"));
    let (reads, _, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(ready, part::VALUE), pin(settle, part::CHANGED)], "§13 rule 1: the guard is read in the same microstep");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_body_without_settle_has_no_settle_pin() {
    // abi.md §4: a `GLOBAL` pin exists only for a body that asks for it
    for make in [Buck::dff, Buck::osc] {
        let (ir, body) = Alone::design(make);
        let plan = BodyPlan::new(ir.body(body));
        assert!(!has_pin(plan.shape().shape(), "settle"));
    }
}

/// A body whose first event is guarded by whether the second fired (§13 rule 9), declared as the
/// elaborator does: the guarded event of the head first, the event the guard reads appended after
/// it. An `in en: Bit`, an `in d: Bit` and an `out q: Bit`; `on d.changed() && !en.changed() { q <-
/// true; }`.
struct Guarding;

impl Guarding {
    const F: &'static str = "guarding.msl";

    fn body(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Guarding", Guarding::F, 1));
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, Guarding::F, 1, 14));
        b.carrier(body, Buck::carrier("d", ValueType::Bool, CarrierRole::In, None, Guarding::F, 1, 28));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, Guarding::F, 1, 40));
        // the guard reads the event declared next: `Fired(1)`
        let mut x = Dx::new(b, Guarding::F, 2, 30);
        let still = x.b.expr(piperine_ir::Expr::Fired(1), ValueType::Bool, x.origin());
        let guard = x.un(piperine_ir::UnaryOp::Not, still, ValueType::Bool);
        let d_changed = b.event(body, Event { source: EventSource::Changed(1), guard: Some(guard), origin: Buck::origin(Guarding::F, 2, 8) });
        b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(Guarding::F, 2, 8) });
        let yes = Dx::new(b, Guarding::F, 3, 14).boolean(true);
        let commit = Dx::new(b, Guarding::F, 3, 9).commit(2, yes, None);
        Buck::reaction(b, body, vec![d_changed], vec![commit], vec![], Guarding::F, 2);
        body
    }
}

/// The levelized order of a shape's discrete functions, by the dag the digital solver runs them in
/// (abi.md §9 D1): every pin is a net, a function reads and writes the nets of its cells.
struct Levelized;

impl Levelized {
    /// The names of the functions of `shape` in the order the dag runs them.
    fn order(shape: &Shape<'_>) -> Vec<String> {
        let nets: Vec<NetSpec> = shape.pins.iter().map(|p| NetSpec::discrete(p.name.as_str(), 1, 1)).collect();
        let nodes: Vec<FnNode> = shape
            .functions
            .iter()
            .enumerate()
            .map(|(k, f)| FnNode {
                name: f.name.as_str().to_string(),
                instance: 0,
                k: k as u32,
                reads: f.reads.iter().filter(|c| c.kind == cell::PIN).map(|c| NetId(c.index)).collect(),
                writes: f.writes.iter().filter(|c| c.kind == cell::PIN).map(|c| NetId(c.index)).collect(),
            })
            .collect();
        let dag = Dag::build(&nodes, &nets).expect("the functions have no cycle");
        dag.order()
            .iter()
            .map(|step| match step {
                Step::Run(f) => dag.function(*f).name.clone(),
                Step::Relax { .. } => panic!("no relaxation in this body"),
            })
            .collect()
    }
}

#[test]
fn an_event_function_reading_another_event_runs_after_it() {
    // §13 rule 9, design D8: the guarded event is declared first (`event:0`) and reads the
    // `CHANGED` byte of the event its guard names, declared after it (`Source::Fired(1)`, `event:1`),
    // so the order of the shape's functions is the wrong one; the dependency of abi.md D1, from
    // what each function reads and writes, runs `event:1` first; the shape validates (no cycle)
    let (ir, body) = Alone::design(Guarding::body);
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (guarded, read) = (Find::pin(shape, "event:0"), Find::pin(shape, "event:1"));
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert!(reads.contains(&pin(read, part::CHANGED)), "the guarded event reads the other event's pin: {reads:?}");
    assert_eq!(writes, [pin(guarded, part::CHANGED)]);
    let (_, writes, _) = Find::function(shape, "event:1");
    assert_eq!(writes, [pin(read, part::CHANGED)], "the read event writes its own pin");
    assert_eq!(plan.functions()[0].reads.iter().filter(|s| **s == Source::Fired(1)).count(), 1, "{:?}", plan.functions()[0].reads);
    let declared = Find::function_names(shape);
    assert!(declared.iter().position(|n| n == "event:0") < declared.iter().position(|n| n == "event:1"), "declaration order is the wrong one: {declared:?}");
    let levelized = Levelized::order(shape);
    assert!(levelized.iter().position(|n| n == "event:1") < levelized.iter().position(|n| n == "event:0"), "the dependency order runs the read event first: {levelized:?}");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}
