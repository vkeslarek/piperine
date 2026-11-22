//! The shapes of the forms the IR amendments of msl-digital add (design "Amendments behind the
//! IR", A1 to A7): the ambient reads `now()` and `analysis(Static)`, timers, record pins, the
//! flow of a potential branch, and the previous value of a carrier. Each test builds a small body
//! by hand and reads its shape entry by entry; every expected entry is written from abi.md (the
//! section cited at its assertion), and every shape passes `piperine_abi::validate`.

use piperine_abi::raw::{Cell, DEFAULT, Shape, ambient, watch, cell, field_flag, global, kind, leaf, part, pin_flag, role, statement_flag};
use piperine_ir::{
    Addend, AnalysisKind, BodyId, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, EmitField, Env, Event, EventSource, Expr, InstanceSpec, Ir,
    IrBuilder, NodeRole, Operator, Origin, QuantityRole, Stmt, Stream, StreamField, Terminal, ValueType,
};
use piperine_plan::{BodyPlan, Mode, Source, Target};

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
        let cross = b.event(body, Event { source: EventSource::Cross { e: crossed, ttol: None, etol: None }, guard: None, origin: Buck::origin(F, 4, 25) });
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
    //     on at(dt) if dt > 0.0 { fired <- false; }
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
        let sum = x.bin(piperine_ir::BinaryOp::Add, t0, dt, ValueType::F64);
        let at = b.event(body, Event { source: EventSource::At(sum), guard: None, origin: Buck::origin(F, 5, 8) });
        let mut x = Dx::new(b, F, 5, 22);
        let yes = x.boolean(true);
        let commit = x.commit(2, yes, None);
        Buck::reaction(b, body, vec![at], vec![commit], vec![], F, 5);
        let mut x = Dx::new(b, F, 6, 11);
        let dt = x.carrier(1, ValueType::F64);
        let dt_guard = x.carrier(1, ValueType::F64);
        let zero = x.real(0.0);
        let positive = x.bin(piperine_ir::BinaryOp::Gt, dt_guard, zero, ValueType::Bool);
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

#[test]
fn a_stream_is_a_record_pin_with_one_field_per_value() {
    // ```text
    // module Probe(in go: Bit, out x: Bit) {}
    // digital for Probe {
    //     on go.changed() {
    //         emit!("trace", code = 7u8, level = 1.5);
    //         x <- true;
    //         emit!("trace", ok = true, note = "hi");
    //     }
    // }
    // ```
    let (ir, body) = Alone::design(|b| {
        let body = b.body(Buck::spec("Probe", F, 1));
        b.carrier(body, Buck::carrier("go", ValueType::Bool, CarrierRole::In, None, F, 1, 14));
        b.carrier(body, Buck::carrier("x", ValueType::Bool, CarrierRole::Out, None, F, 1, 26));
        let fields = vec![
            StreamField { name: "code".into(), ty: ValueType::UInt(8) },
            StreamField { name: "level".into(), ty: ValueType::F64 },
            StreamField { name: "ok".into(), ty: ValueType::Bool },
        ];
        let trace = b.stream(body, Stream { name: "trace".into(), fields });
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 3, 8) });
        let mut x = Dx::new(b, F, 4, 9);
        let (code, level) = (x.int(7, ValueType::UInt(8)), x.real(1.5));
        let first = x.b.stmt(Stmt::Emit {
            stream: trace,
            fields: vec![EmitField::Value { field: 0, value: code }, EmitField::Value { field: 1, value: level }],
        });
        x.at(5, 9);
        let yes = x.boolean(true);
        let commit = x.commit(1, yes, None);
        x.at(6, 9);
        let ok = x.boolean(true);
        let second = x.b.stmt(Stmt::Emit {
            stream: trace,
            fields: vec![EmitField::Value { field: 2, value: ok }, EmitField::Text { name: "note".into(), text: "hi".into() }],
        });
        Buck::reaction(b, body, vec![changed], vec![first, commit, second], vec![], F, 3);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (trace, x, event) = (Find::pin(shape, "trace"), Find::pin(shape, "x"), Find::pin(shape, "event:0"));
    let p = shape.pins.get(trace as usize).expect("the record pin");

    // abi.md P7, C5: a stream is one PRIVATE discrete pin per instance, flagged RECORD, named by
    // the stream
    assert_eq!((p.kind, p.role, p.flags), (kind::DISCRETE, role::PRIVATE, pin_flag::RECORD));
    // one field per value field, each with a known byte (P3), laid out as a carrier of its type:
    // `code` a `BYTE` at 0 (known 1), `level` a `REAL` at the next multiple of 8 (value 8, known
    // 16), `ok` a `BIT` at 24 (known 25), then the `maybe` of §19 rule 1, a `BIT` at 26 (known
    // 27); 28 bytes rounded up to the alignment 8. The text `note` is no field: text never
    // crosses the ABI (design "Amendments behind the IR")
    let fields: Vec<(String, u32, u32, u32, u32, u32)> =
        p.layout.fields.iter().map(|f| (f.name.as_str().to_string(), f.leaf, f.offset, f.count, f.flags, f.known)).collect();
    let known = field_flag::HAS_KNOWN;
    assert_eq!(
        fields,
        [
            ("code".into(), leaf::U8, 0, 1, known, 1),
            ("level".into(), leaf::F64, 8, 1, known, 16),
            ("ok".into(), leaf::BOOL, 24, 1, known, 25),
            ("maybe".into(), leaf::BOOL, 26, 1, known, 27),
        ]
    );
    assert_eq!((p.layout.size, p.layout.align), (32, 8));

    // the two sites share the pin: each is a statement over its whole layout, in the order of
    // the reaction's commits (C5: a record is a commit to a RECORD pin); a site writes the
    // fields it leaves out with their known byte 0, which the layout makes room for
    let (on, reads, statements) = Find::reaction(shape, "reaction:0");
    assert_eq!((on, reads), (vec![event], vec![]));
    assert_eq!(statements, [(trace, 0, 32, 0, 0.0), (x, 0, 2, 0, 0.0), (trace, 0, 32, 0, 0.0)]);
    assert_eq!(plan.reactions()[0].statements.iter().map(|s| s.target).collect::<Vec<_>>(), [Target::Record(0), Target::Carrier(1), Target::Record(0)]);
    // nothing reads the pin (P7: RECORD implies QUIET)
    let read = |c: &Cell| c.kind == cell::PIN && c.index == trace;
    assert!(shape.functions.iter().all(|f| !f.reads.iter().any(read)) && shape.reactions.iter().all(|r| !r.reads.iter().any(read)));

    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

/// `module <name>(p, n) { <param>: f64 = 1.0; analog { v(p, n) +> <operator>(<param> * i(p, n)); } }`:
/// a potential branch whose one addend reads the branch's own flow.
fn reads_its_flow(b: &mut IrBuilder, name: &str, param: &str, operator: Operator) -> BodyId {
    let nodes = ports(b);
    let mut x = Dx::new(b, F, 3, 30);
    let k = x.param(0, ValueType::F64);
    let o = x.origin();
    let flow = x.b.expr(Expr::Flow { branch: 0 }, ValueType::F64, o);
    let value = x.bin(piperine_ir::BinaryOp::Mul, k, flow, ValueType::F64);
    let contribution = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: QuantityRole::Potential,
        addends: vec![Addend { operator, negated: false, value }],
    });
    b.body(BodySpec {
        params: vec![Buck::param(param, F, 2, Some(1.0), None)],
        nodes,
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Potential }],
        analog: Some(vec![contribution]),
        ..Buck::spec(name, F, 1)
    })
}

#[test]
fn a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term() {
    // the inductor of the power stage: `v(p, n) +> ddt(l * i(p, n))`
    let (ir, body) = Alone::design(|b| reads_its_flow(b, "Inductor", "l", Operator::Ddt));
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (l, p, n, f) = (Find::pin(shape, "l"), Find::pin(shape, "p"), Find::pin(shape, "n"), Find::pin(shape, "b0.flow"));
    // abi.md A1: a potential branch is its private flow pin `f`, the term `FLOW {p, n}` that
    // carries it into KCL, and its equation row `FLOW {f, GROUND}`; a `ddt` addend of a
    // potential contribution is a `DDT {f, GROUND}` term on that same row (abi.md §11.6, the
    // DAC's `eq` row, with the charge as its own term)
    let terms: Vec<(String, u32, u32, u32)> = shape.terms.iter().map(|t| (t.name.as_str().to_string(), t.op, t.plus, t.minus)).collect();
    let (flow, ddt, ground) = (piperine_abi::raw::op::FLOW, piperine_abi::raw::op::DDT, piperine_abi::raw::GROUND);
    assert_eq!(terms, [("b0.flow".into(), flow, p, n), ("b0.eq".into(), flow, f, ground), ("b0.charge".into(), ddt, f, ground)]);
    // `i(p, n)` is a read of the flow pin, differentiable like a node (a tangent slot of its
    // own, after the nodes): `terms` reads p, n, f, then l
    let (reads, _, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [pin(p, part::VALUE), pin(n, part::VALUE), pin(f, part::VALUE), pin(l, part::VALUE)]);
    let terms_plan = plan.functions().iter().find(|x| x.mode == Mode::Terms).expect("terms");
    assert_eq!(terms_plan.reads, [Source::Node(0), Source::Node(1), Source::BranchFlow(0), Source::Param(0)]);
    assert_eq!(terms_plan.slots, 3);
    // write 0 (KCL) against f; write 1 (the row, v(p, n) with no direct addend) against p and n;
    // write 2 (the charge `l·f`) against f
    assert_eq!(partials, [(0, 2), (1, 0), (1, 1), (2, 2)]);
    // INT-12: the row and the KCL term are affine in the pins, a charge never is
    assert_eq!(terms_plan.affine, [true, true, false]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));

    // the same read in a direct addend (`v(p, n) +> r * i(p, n)`: a resistor written as a
    // potential): the row `v(p, n) - r·f` reads f, and stays affine in the pins
    let (ir, body) = Alone::design(|b| reads_its_flow(b, "FlowResistor", "r", Operator::Direct));
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (_, _, partials) = Find::function(shape, "terms");
    assert_eq!(partials, [(0, 2), (1, 0), (1, 1), (1, 2)], "the row against p, n and f");
    let terms_plan = plan.functions().iter().find(|x| x.mode == Mode::Terms).expect("terms");
    assert_eq!(terms_plan.affine, [true, true]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_previous_value_is_the_previous_part_of_the_pin() {
    // the flip-flop of abi.md §11.4 with `rise()` as the stdlib writes it (design A7, part IV
    // §22.1): the previous level is low and the level is high
    //
    // ```text
    // module Dff(in clk: Bit, in d: Bit, out q: Bit) {}
    // digital for Dff {
    //     on clk.changed() if !clk.prev() && clk { q <- d; }
    //     on d.changed() { q <- d.prev(); }
    // }
    // ```
    let (ir, body) = Alone::design(|b| {
        let body = b.body(Buck::spec("Dff", F, 1));
        for (name, role, col) in [("clk", CarrierRole::In, 12), ("d", CarrierRole::In, 25), ("q", CarrierRole::Out, 36)] {
            b.carrier(body, Buck::carrier(name, ValueType::Bool, role, None, F, 1, col));
        }
        let mut x = Dx::new(b, F, 3, 25);
        let o = x.origin();
        let prev = x.b.expr(Expr::Previous(0), ValueType::Bool, o);
        let not_was_high = x.un(piperine_ir::UnaryOp::Not, prev, ValueType::Bool);
        let clk = x.carrier(0, ValueType::Bool);
        let guard = x.bin(piperine_ir::BinaryOp::And, not_was_high, clk, ValueType::Bool);
        let rise = b.event(body, Event { source: EventSource::Changed(0), guard: Some(guard), origin: Buck::origin(F, 3, 8) });
        let mut x = Dx::new(b, F, 3, 90);
        let d = x.carrier(1, ValueType::Bool);
        let commit = x.commit(2, d, None);
        Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 3);
        let changed = b.event(body, Event { source: EventSource::Changed(1), guard: None, origin: Buck::origin(F, 4, 8) });
        let mut x = Dx::new(b, F, 4, 30);
        let o = x.origin();
        let prev = x.b.expr(Expr::Previous(1), ValueType::Bool, o);
        let commit = x.commit(2, prev, None);
        Buck::reaction(b, body, vec![changed], vec![commit], vec![], F, 4);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (clk, d, q, event) = (Find::pin(shape, "clk"), Find::pin(shape, "d"), Find::pin(shape, "q"), Find::pin(shape, "event:0"));

    // abi.md §11.4: fn "rise" reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED] and writes [rise]
    let (reads, writes, _) = Find::function(shape, "event:0");
    assert_eq!(reads, [pin(clk, part::VALUE), pin(clk, part::PREVIOUS), pin(clk, part::CHANGED)]);
    assert_eq!(writes, [pin(event, part::CHANGED)]);
    assert_eq!(plan.functions().iter().find(|f| f.mode == Mode::Event(0)).map(|f| f.reads.clone()), Some(vec![
        Source::Carrier(0),
        Source::CarrierPrevious(0),
        Source::CarrierChanged(0),
    ]));
    // abi.md §4.1: "PREVIOUS ... changes exactly when VALUE does, so any function or reaction may
    // read it": the reaction reads d's PREVIOUS (not its VALUE) and commits q
    let (_, reads, statements) = Find::reaction(shape, "reaction:1");
    assert_eq!(reads, [pin(d, part::PREVIOUS)]);
    assert_eq!(statements, [(q, 0, 2, 0, 0.0)]);
    // P7: a QUIET pin keeps no PREVIOUS, so a carrier whose previous value is read is not QUIET
    for p in [clk, d] {
        assert_eq!(shape.pins.get(p as usize).map(|x| x.flags & pin_flag::QUIET), Some(0));
    }

    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn freq_reads_the_ambient_and_analysis_reads_its_global_pin() {
    // ```text
    // module Probe(p, n) {
    //     analog { i(p, n) +> v(p, n) * if analysis(Dc) { 1.0 } else if analysis(Ac) { freq() } else { 0.0 }; }
    // }
    // ```
    let (ir, body) = Alone::design(|b| {
        let nodes = ports(b);
        let mut x = Dx::new(b, F, 2, 20);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let o = x.origin();
        let dc = x.b.expr(Expr::Env(Env::Analysis(AnalysisKind::Dc)), ValueType::Bool, o.clone());
        let ac = x.b.expr(Expr::Env(Env::Analysis(AnalysisKind::Ac)), ValueType::Bool, o.clone());
        let freq = x.b.expr(Expr::Env(Env::Freq), ValueType::F64, o);
        let (one, zero) = (x.real(1.0), x.real(0.0));
        let inner = x.if_(ac, freq, zero, ValueType::F64);
        let level = x.if_(dc, one, inner, ValueType::F64);
        let value = x.bin(piperine_ir::BinaryOp::Mul, v, level, ValueType::F64);
        let contribution = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Flow,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value }],
        });
        b.body(BodySpec {
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
            analog: Some(vec![contribution]),
            ..Buck::spec("Probe", F, 1)
        })
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (p, n) = (Find::pin(shape, "p"), Find::pin(shape, "n"));

    // abi.md §3: a `GLOBAL` pin names its global; `DC`, `TRAN`, `AC` and `NOISE` are flags the
    // host sets to 1 during the analysis. One pin per kind read, in kind order (`dc` before `ac`),
    // each a `BIT`; a kind the body does not read has none.
    let (dc, ac) = (Find::pin(shape, "analysis:dc"), Find::pin(shape, "analysis:ac"));
    assert!(dc < ac, "the pins follow the order of the kinds");
    let global_of = |pin: u32| shape.pins.get(pin as usize).map(|x| (x.kind, x.role, x.global));
    assert_eq!(global_of(dc), Some((kind::DISCRETE, role::GLOBAL, global::DC)));
    assert_eq!(global_of(ac), Some((kind::DISCRETE, role::GLOBAL, global::AC)));
    for name in ["analysis:tran", "analysis:noise"] {
        assert!(shape.pins.iter().all(|x| x.name.as_str() != name), "{name} is declared only when read");
    }

    // abi.md §4.2: `FREQ` is an ambient, read by constant and never differentiated; a global
    // pins are read as any pin is, after the continuous reads and before the ambients.
    let (reads, _, partials) = Find::function(shape, "terms");
    assert_eq!(reads, [pin(p, part::VALUE), pin(n, part::VALUE), ambient(ambient::FREQ), pin(dc, part::VALUE), pin(ac, part::VALUE)]);
    assert_eq!(partials, [(0, 0), (0, 1)], "no partial against FREQ or a global");
    let terms = plan.functions().iter().find(|f| f.mode == Mode::Terms).expect("terms");
    assert_eq!(
        terms.reads,
        [Source::Node(0), Source::Node(1), Source::Freq, Source::Analysis(AnalysisKind::Dc), Source::Analysis(AnalysisKind::Ac)]
    );
    assert_eq!(terms.slots, 2, "two tangent slots: p and n");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn bound_step_is_a_cap_watch_and_cross_tolerances_are_pins() {
    // ```text
    // module Step(p, n, out q: Bit, tt: f64 = 1p, et: f64 = 1u) {
    //     analog { bound_step(1n * (1.0 + v(p, n))); }
    // }
    // digital for Step { on cross(v(p, n), tt, et) { q <- true; } }
    // ```
    let (ir, body) = Alone::design(|b| {
        let nodes = ports(b);
        let mut x = Dx::new(b, F, 3, 20);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let (one, base) = (x.real(1.0), x.real(1e-9));
        let sum = x.bin(piperine_ir::BinaryOp::Add, one, v, ValueType::F64);
        let step = x.bin(piperine_ir::BinaryOp::Mul, base, sum, ValueType::F64);
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: None });
        let body = b.body(BodySpec {
            nodes,
            params: vec![Buck::param("tt", F, 1, Some(1e-12), None), Buck::param("et", F, 1, Some(1e-6), None)],
            analog: Some(vec![cap]),
            ..Buck::spec("Step", F, 1)
        });
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, F, 1, 30));
        let mut x = Dx::new(b, F, 5, 30);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let (tt, et) = (x.param(0, ValueType::F64), x.param(1, ValueType::F64));
        let cross = b.event(body, Event { source: EventSource::Cross { e: v, ttol: Some(tt), etol: Some(et) }, guard: None, origin: Buck::origin(F, 5, 25) });
        let mut x = Dx::new(b, F, 5, 60);
        let yes = x.boolean(true);
        let commit = x.commit(0, yes, None);
        Buck::reaction(b, body, vec![cross], vec![commit], vec![], F, 5);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (p, n, tt, et) = (Find::pin(shape, "p"), Find::pin(shape, "n"), Find::pin(shape, "tt"), Find::pin(shape, "et"));
    let (ttol, etol) = (Find::pin(shape, "ttol:0"), Find::pin(shape, "etol:0"));

    // C11: a `PRIVATE` discrete pin per tolerance, no pin for a tolerance not given.
    let kind_role = |pin: u32| shape.pins.get(pin as usize).map(|x| (x.kind, x.role));
    assert_eq!(kind_role(ttol), Some((kind::DISCRETE, role::PRIVATE)));
    assert_eq!(kind_role(etol), Some((kind::DISCRETE, role::PRIVATE)));

    // C9, C11: the `CROSS` watch names both pins; one `CAP` watch per `bound_step` site, after the
    // `CROSS` watches, firing no event (abi.md: only a `CROSS` watch has an event).
    let watches: Vec<(&str, u32, u32, u32)> = shape.watches.iter().map(|w| (w.name.as_str(), w.kind, w.ttol, w.etol)).collect();
    assert_eq!(watches, [("cross:0", watch::CROSS, ttol, etol), ("cap:0", watch::CAP, DEFAULT, DEFAULT)]);

    // The tolerances are written by discrete functions reading the params they name; the cap by
    // an analog function reading the probe, writing the watch's `VALUE`.
    let value = |k: u32, index: u32| Cell { kind: k, index, part: part::VALUE };
    assert_eq!(Find::function(shape, "ttol:0"), (vec![pin(tt, part::VALUE)], vec![pin(ttol, part::VALUE)], vec![]));
    assert_eq!(Find::function(shape, "etol:0"), (vec![pin(et, part::VALUE)], vec![pin(etol, part::VALUE)], vec![]));
    assert_eq!(Find::function(shape, "cap:0"), (vec![pin(p, part::VALUE), pin(n, part::VALUE)], vec![value(cell::WATCH, 1)], vec![]));
    let modes: Vec<Mode> = plan.functions().iter().map(|f| f.mode).collect();
    assert!(modes.contains(&Mode::Cap(0)) && modes.contains(&Mode::Ttol(0)) && modes.contains(&Mode::Etol(0)), "{modes:?}");
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}

#[test]
fn a_fired_read_names_the_event_pin() {
    // ```text
    // module Which(in a: Bit, out q: Bit = false) {}
    // digital for Which {
    //     on a.changed(), cross(v(p, n)) { q <- fired(0); }
    // }
    // ```
    let (ir, body) = Alone::design(|b| {
        let nodes = ports(b);
        let body = b.body(BodySpec { nodes, ..Buck::spec("Which", F, 1) });
        b.carrier(body, Buck::carrier("a", ValueType::Bool, CarrierRole::In, None, F, 1, 30));
        b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, None, F, 1, 45));
        let mut x = Dx::new(b, F, 3, 30);
        let v = x.probe(Terminal::Node(0), Terminal::Node(1));
        let cross = b.event(body, Event { source: EventSource::Cross { e: v, ttol: None, etol: None }, guard: None, origin: Buck::origin(F, 3, 25) });
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 3, 8) });
        let mut x = Dx::new(b, F, 3, 60);
        let fired = x.b.expr(Expr::Fired(0), ValueType::Bool, x.origin());
        let commit = x.commit(1, fired, None);
        Buck::reaction(b, body, vec![cross, changed], vec![commit], vec![], F, 3);
        body
    });
    let plan = BodyPlan::new(ir.body(body));
    let shape = plan.shape().shape();
    let (event0, event1, q) = (Find::pin(shape, "event:0"), Find::pin(shape, "event:1"), Find::pin(shape, "q"));

    // abi.md §4.1: a reaction reads the `CHANGED` byte of an event pin: here event 0's, and
    // nothing of event 1 (which the reaction only waits on).
    assert_eq!(Find::reaction(shape, "reaction:0"), (vec![event0, event1], vec![pin(event0, part::CHANGED)], vec![(q, 0, 2, 0, 0.0)]));
    assert_eq!(plan.reactions()[0].reads, [Source::Fired(0)]);
    assert_eq!(piperine_abi::validate::validate(shape), Ok(()));
}
