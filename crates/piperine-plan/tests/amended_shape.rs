//! The shapes of the forms the IR amendments of msl-digital add (design "Amendments behind the
//! IR", A1 to A7): the ambient reads `now()` and `analysis(Static)`, timers, record pins, the
//! flow of a potential branch, and the previous value of a carrier. Each test builds a small body
//! by hand and reads its shape entry by entry; every expected entry is written from abi.md (the
//! section cited at its assertion), and every shape passes `piperine_abi::validate`.

use piperine_abi::raw::{Cell, Shape, ambient, cell, global, kind, part, role, statement_flag};
use piperine_ir::{
    Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Env, Event, EventSource, Expr, InstanceSpec, Ir, IrBuilder,
    NodeRole, Operator, Origin, QuantityRole, Stmt, Terminal, ValueType,
};
use piperine_plan::{BodyPlan, Mode, Source};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use digital::{Buck, Dx};

const F: &str = "amended.msl";

/// One body alone in a design, under an empty top body, as the IR's own tests place it.
struct Alone;

impl Alone {
    /// The design holding the body `make` builds, which must pass `check()`.
    fn design(make: impl FnOnce(&mut IrBuilder) -> BodyId) -> (Ir, BodyId) {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let at = Origin { file: "top.msl".into(), line: 1, col: 1 };
        let top_body = b.body(BodySpec { origin: at.clone(), ..Buck::spec("Top", "top.msl", 1) });
        let top = b.instance(InstanceSpec {
            path: "Top".into(),
            name: "Top".into(),
            body: top_body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: at,
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

/// An ambient cell (abi.md §4.2: no shape entry, named by its constant).
fn ambient(index: u32) -> Cell {
    Cell { kind: cell::AMBIENT, index, part: part::VALUE }
}

/// Two `Electrical` ports `p`, `n` of a body declared at line 1.
fn ports(b: &mut IrBuilder) -> Vec<BodyNode> {
    let el = Buck::electrical(b);
    ["p", "n"]
        .iter()
        .zip([10, 18])
        .map(|(name, col)| BodyNode { name: (*name).into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) })
        .collect()
}

#[test]
fn time_and_static_are_ambient_reads() {
    // ```text
    // module Clocked(p, n, out t: Bit<f64>, out s: Bit) {
    //     analog { i(p, n) +> v(p, n) * now() + if analysis(Static) { 1.0 } else { 0.0 }; }
    // }
    // digital for Clocked { on cross(v(p, n) - now()) { t <- now(); s <- analysis(Static); } }
    // ```
    let (ir, body) = Alone::design(|b| {
        let nodes = ports(b);
        let mut x = Dx::new(b, F, 2, 20);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o.clone());
        let product = x.bin(piperine_ir::BinaryOp::Mul, v, now, ValueType::F64);
        let stat = x.b.expr(Expr::Env(Env::Static), ValueType::Bool, o);
        let (one, zero) = (x.real(1.0), x.real(0.0));
        let level = x.if_(stat, one, zero, ValueType::F64);
        let sum = x.bin(piperine_ir::BinaryOp::Add, product, level, ValueType::F64);
        let contribution = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: sum }],
        });
        let body = b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(vec![contribution]),
            ..Buck::spec("Clocked", F, 1)
        });
        b.carrier(body, Buck::carrier("t", ValueType::F64, CarrierRole::Out, None, F, 1, 30));
        b.carrier(body, Buck::carrier("s", ValueType::Bool, CarrierRole::Out, None, F, 1, 45));
        let mut x = Dx::new(b, F, 4, 30);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o);
        let crossed = x.bin(piperine_ir::BinaryOp::Sub, v, now, ValueType::F64);
        let cross = b.event(body, Event { source: EventSource::Cross(crossed), guard: None, origin: Buck::origin(F, 4, 25) });
        let mut x = Dx::new(b, F, 4, 60);
        let o = x.origin();
        let now = x.b.expr(Expr::Env(Env::Time), ValueType::F64, o.clone());
        let t = x.commit(0, now, None);
        let stat = x.b.expr(Expr::Env(Env::Static), ValueType::Bool, o);
        let s = x.commit(1, stat, None);
        Buck::reaction(b, body, vec![cross], vec![t, s], vec![], F, 4);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (p, n) = (Find::pin(shape, "p"), Find::pin(shape, "n"));
    let (time, stat) = (ambient(ambient::TIME), ambient(ambient::STATIC));

    // abi.md §4.2: `TIME` and `STATIC` are ambient inputs an analog function reads by constant;
    // they are no differentiable read, so `terms` declares its partials against p and n only
    let (reads, writes, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [pin(p, part::VALUE), pin(n, part::VALUE), stat, time]);
    assert_eq!(writes, [Cell { kind: cell::TERM, index: 0, part: part::VALUE }]);
    assert_eq!(partials, [(0, 0), (0, 1)], "no partial against TIME or STATIC");
    let terms = plan.functions().iter().find(|f| f.mode == Mode::Terms).expect("terms");
    assert_eq!(terms.reads, [Source::Node(0), Source::Node(1), Source::Static, Source::Time]);
    assert_eq!(terms.slots, 2, "two tangent slots: p and n");

    // the `cross:0` function is analog too: it reads TIME beside the pins, differentiated
    // against the pins only
    let (reads, writes, partials) = Find::function(shape, "cross:0");
    assert_eq!(reads, [pin(p, part::VALUE), pin(n, part::VALUE), time]);
    assert_eq!(writes, [Cell { kind: cell::WATCH, index: 0, part: part::VALUE }]);
    assert_eq!(partials, [(0, 0), (0, 1)]);

    // abi.md §4.1: "A reaction reads PIN cells and the ambient inputs TIME, STATIC, TICK,
    // HORIZON": the reaction reads both, and commits t then s
    let (t, s, event) = (Find::pin(shape, "t"), Find::pin(shape, "s"), Find::pin(shape, "event:0"));
    assert_eq!(Find::reaction(shape, "reaction:0"), (vec![event], vec![stat, time], vec![(t, 0, 16, 0, 0.0), (s, 0, 2, 0, 0.0)]));
    assert_eq!(plan.reactions()[0].reads, [Source::Static, Source::Time]);

    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement() {
    // ```text
    // module Timer(out fired: Bit) {
    //     signal t0: Bit<f64> = 10n;  signal dt: Bit<f64> = 5n;
    // }
    // digital for Timer {
    //     on at(t0 + dt) { fired <- true; }
    //     on at(dt) if dt.known_or(0.0) > 0.0 { fired <- false; }
    // }
    // ```
    let (ir, body) = Alone::design(|b| {
        let body = b.body(Buck::spec("Timer", F, 1));
        let init = |v: f64| Some(piperine_ir::Constant::Real(piperine_ir::Real::new(v)));
        b.carrier(body, Buck::carrier("t0", ValueType::F64, CarrierRole::Signal, init(10e-9), F, 2, 5));
        b.carrier(body, Buck::carrier("dt", ValueType::F64, CarrierRole::Signal, init(5e-9), F, 2, 30));
        b.carrier(body, Buck::carrier("fired", ValueType::Bool, CarrierRole::Out, None, F, 1, 14));
        let mut x = Dx::new(b, F, 5, 11);
        let (t0, dt) = (x.carrier(0, ValueType::F64), x.carrier(1, ValueType::F64));
        let sum = x.bin(piperine_ir::BinaryOp::Add, t0, dt, digital::Ty::flagged(ValueType::F64));
        let at = b.event(body, Event { source: EventSource::At(sum), guard: None, origin: Buck::origin(F, 5, 8) });
        let mut x = Dx::new(b, F, 5, 22);
        let yes = x.boolean(true);
        let commit = x.commit(2, yes, None);
        Buck::reaction(b, body, vec![at], vec![commit], vec![], F, 5);
        let mut x = Dx::new(b, F, 6, 11);
        let dt = x.carrier(1, ValueType::F64);
        let (zero, dt_guard) = (x.real(0.0), x.carrier(1, ValueType::F64));
        let known = x.known_or(dt_guard, zero, ValueType::F64);
        let zero = x.real(0.0);
        let positive = x.bin(piperine_ir::BinaryOp::Gt, known, zero, ValueType::Bool);
        let guarded = b.event(body, Event { source: EventSource::At(dt), guard: Some(positive), origin: Buck::origin(F, 6, 8) });
        let mut x = Dx::new(b, F, 6, 40);
        let no = x.boolean(false);
        let commit = x.commit(2, no, None);
        Buck::reaction(b, body, vec![guarded], vec![commit], vec![], F, 6);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (t0, dt, fired) = (Find::pin(shape, "t0"), Find::pin(shape, "dt"), Find::pin(shape, "fired"));
    let (event0, event1) = (Find::pin(shape, "event:0"), Find::pin(shape, "event:1"));
    let (timer1, start) = (Find::pin(shape, "at:1"), Find::pin(shape, "start"));
    let (rearm_t0, rearm_dt, rearm1_dt) = (Find::pin(shape, "at:0.t0"), Find::pin(shape, "at:0.dt"), Find::pin(shape, "at:1.dt"));
    let kind_role = |p: u32| shape.pins.get(p as usize).map(|x| (x.kind, x.role, x.global));

    // abi.md R1: "a timer is a delayed commit to an event pin" and "START is a global event pin":
    // each event has its PRIVATE event pin; a guarded one's timer fires a PRIVATE pin of its own,
    // which the event function gates as a guarded `cross` is gated
    for p in [event0, event1, timer1, rearm_t0, rearm_dt, rearm1_dt] {
        assert_eq!(kind_role(p), Some((kind::EVENT, role::PRIVATE, 0)), "{}", shape.pins.get(p as usize).map_or("", |x| x.name.as_str()));
    }
    assert_eq!(kind_role(start), Some((kind::EVENT, role::GLOBAL, global::START)), "the START pin (abi.md §3, `global`)");

    // the `changed` event function of each carrier a time reads: CHANGED in (abi.md §4.1: only a
    // function that writes event pins only may read CHANGED), its event pin out
    for (name, carrier, out) in [("at:0.t0", t0, rearm_t0), ("at:0.dt", dt, rearm_dt), ("at:1.dt", dt, rearm1_dt)] {
        assert_eq!(Find::function(shape, name), (vec![pin(carrier, part::CHANGED)], vec![pin(out, part::CHANGED)], vec![]), "{name}");
    }
    // the guarded timer's event function: the guard's carrier and the timer pin's CHANGED
    assert_eq!(Find::function(shape, "event:1"), (vec![pin(dt, part::VALUE), pin(timer1, part::CHANGED)], vec![pin(event1, part::CHANGED)], vec![]));
    assert!(shape.functions.iter().all(|f| f.name.as_str() != "event:0"), "an unguarded timer fires its event pin itself");
    assert_eq!(plan.functions().iter().filter(|f| matches!(f.mode, Mode::Rearm { .. })).count(), 3);

    // the arming reactions: on START and the re-arming pins; they read what the time reads and
    // TIME (abi.md §4.1: a reaction reads TIME); one REPLACE statement on the pin the timer fires,
    // `len` 0 (C4: a statement on an event pin), no LATER bound (a delay of 0 is legal, C1)
    let time = ambient(ambient::TIME);
    let replace = statement_flag::REPLACE;
    assert_eq!(
        Find::reaction(shape, "at:0"),
        (vec![start, rearm_t0, rearm_dt], vec![pin(t0, part::VALUE), pin(dt, part::VALUE), time], vec![(event0, 0, 0, replace, 0.0)])
    );
    assert_eq!(Find::reaction(shape, "at:1"), (vec![start, rearm1_dt], vec![pin(dt, part::VALUE), time], vec![(timer1, 0, 0, replace, 0.0)]));
    // the body's own reactions listen to the event pins and commit `fired`
    assert_eq!(Find::reaction(shape, "reaction:0"), (vec![event0], vec![], vec![(fired, 0, 2, 0, 0.0)]));
    assert_eq!(Find::reaction(shape, "reaction:1"), (vec![event1], vec![], vec![(fired, 0, 2, 0, 0.0)]));
    let names: Vec<&str> = shape.reactions.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["reaction:0", "reaction:1", "at:0", "at:1"], "the arming reactions follow the body's");

    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}
