//! Scope: the IR amendments of msl-digital (design A1 to A7, contract D3 to D7): the time and
//! static reads, the `at` event, emit sites and streams, the flow of a potential branch, the
//! previous value of a carrier, a child param that reads the parent, and the carrier bindings
//! of a child. Each is an IR node that `Ir::check` types and the printer writes, with its
//! dependency set; the earlier prints stay byte-identical. Expected texts are written from the
//! MSL source each fixture transcribes.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;

#[path = "support/devices.rs"]
#[allow(dead_code)]
mod devices;

use devices::Devices;
use digital::{Buck, Dx, Ty};
use piperine_ir::{
    BodyId, BodySpec, Branch, BranchKind, CarrierRole, Child, ChildValue, Constant, Deps, Drive, EmitField, Env, Event, EventSource, Expr, ExprId, FunctionSpec, InstanceSpec, Ir,
    IrBuilder, NetSpec, NodeRef, NodeSpec, Origin, Real, RecordSite, Stmt, Stream, StreamField, Target, Terminal, ValueType,
};

/// Designs for the tests: one body placed as the top of a design.
struct Design;

impl Design {
    fn origin(file: &str, line: u32, col: u32) -> Origin {
        Origin { file: file.to_string(), line, col }
    }

    /// `body` placed as the design, its nodes on nodes of their own and its carriers on nets of
    /// their own.
    fn place(mut b: IrBuilder, body: BodyId, nodes: usize, carriers: &[ValueType], params: Vec<Constant>) -> Ir {
        let el = Buck::electrical(&mut b);
        let at = b.next_instance_id();
        let nodes = (0..nodes)
            .map(|n| NodeRef::Node(b.node(NodeSpec { path: format!("Top.n{n}"), nature: el, declared_by: at, init: None, origin: Design::origin("top.msl", 1, 1) })))
            .collect();
        let nets = carriers
            .iter()
            .enumerate()
            .map(|(c, ty)| b.net(NetSpec { path: format!("Top.c{c}"), ty: ty.clone(), declared_by: at, init: None, origin: Design::origin("top.msl", 1, 1) }))
            .collect();
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes, params, origin: Design::origin("top.msl", 1, 1) });
        b.bind_nets(top, nets);
        b.finish(top)
    }

    /// The ports `p` and `n` of an `Electrical` body.
    fn ports(b: &mut IrBuilder, file: &str) -> Vec<piperine_ir::BodyNode> {
        let el = Buck::electrical(b);
        ["p", "n"]
            .iter()
            .enumerate()
            .map(|(i, name)| piperine_ir::BodyNode { name: (*name).into(), nature: el, role: piperine_ir::NodeRole::Port, init: None, origin: Design::origin(file, 1, 10 + 8 * i as u32) })
            .collect()
    }
}

/// The reads the amendments add, written at the position of a `Dx`.
struct Read;

impl Read {
    fn time(x: &mut Dx<'_>) -> ExprId {
        let o = x.origin();
        x.b.expr(Expr::Env(Env::Time), ValueType::F64, o)
    }

    fn is_static(x: &mut Dx<'_>) -> ExprId {
        let o = x.origin();
        x.b.expr(Expr::Env(Env::Static), ValueType::Bool, o)
    }
}

/// The text a printed IR must hold, as one block of consecutive lines.
struct Printed;

impl Printed {
    fn holds(ir: &Ir, lines: &[&str]) {
        let text = ir.print_structural();
        let block = lines.join("\n");
        assert!(text.contains(&format!("{block}\n")), "missing:\n{block}\n--- in ---\n{text}");
    }
}

#[test]
fn time_and_static_reads_print_and_check() {
    // power.msl, a body that reads the clock and the analysis:
    //
    //   module Clocked(p: Electrical, n: Electrical, out q: Bit = false) {                // 1
    //       analog { i(p, n) +> v(p, n) * if analysis(Static) { 0.0 } else { now() }; }   // 2
    //   }
    //   digital for Clocked { on q.changed() { q <- if analysis(Static) { false } else { now() > 1u } } }
    const F: &str = "clocked.msl";
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, F, 2, 20);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let stat = Read::is_static(&mut x);
    let zero = x.real(0.0);
    let now = Read::time(&mut x);
    let level = x.if_(stat, zero, now, ValueType::F64);
    let value = x.bin(piperine_ir::BinaryOp::Mul, v, level, ValueType::F64);
    let contribution = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: piperine_ir::QuantityRole::Flow,
        addends: vec![piperine_ir::Addend { operator: piperine_ir::Operator::Direct, negated: false, value }],
    });
    let nodes = Design::ports(&mut b, F);
    let body = b.body(BodySpec {
        nodes,
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![contribution]),
        ..Buck::spec("Clocked", F, 1)
    });
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 40));
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(F, 3, 8) });
    let mut x = Dx::new(&mut b, F, 3, 30);
    let stat = Read::is_static(&mut x);
    let off = x.boolean(false);
    let now = Read::time(&mut x);
    let one = x.real(1.0);
    let late = x.bin(piperine_ir::BinaryOp::Gt, now, one, ValueType::Bool);
    let next = x.if_(stat, off, late, ValueType::Bool);
    let commit = x.commit(0, next, None);
    Buck::reaction(&mut b, body, vec![changed], vec![commit], vec![], F, 3);
    let ir = Design::place(b, body, 2, &[ValueType::Bool], vec![]);

    // Typed `f64` and `bool`, so the IR's own rules accept them.
    assert_eq!(ir.check(), vec![]);

    // Printed as `time()` and `static()`, like the other environment reads, and the structural
    // form writes no type note: the form implies it.
    Printed::holds(&ir, &["  analog", "    b0.flow +> direct (probe(p, n) * if static() { 0.0 } else { time() })"]);
    Printed::holds(&ir, &["  reaction R0 on (E0)", "    q <- if static() { false } else { (time() > 1.0) }"]);
    assert!(!ir.print_structural().contains(" : "), "a type note was written:\n{}", ir.print_structural());
    let full = ir.to_string();
    for line in ["env time", "env static"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // The dependency sets list both, in `Env` order, among the sources of the contribution and of
    // the reaction.
    let body = ir.top().body();
    assert_eq!(body.dependencies(Target::Flow(0)), Deps { nodes: vec![0, 1], env: vec![Env::Time, Env::Static], ..Deps::default() });
    assert_eq!(body.dependencies(Target::Reaction(0)), Deps { env: vec![Env::Time, Env::Static], ..Deps::default() });

    // A drive, an event guard and, through a function, a drive read neither: an input is read by
    // no discrete function (abi.md §4.2). One entry per read.
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, F, 2, 5);
    let now = Read::time(&mut x);
    let one = x.real(1.0);
    let late = x.bin(piperine_ir::BinaryOp::Gt, now, one, ValueType::Bool);
    let reads_static = {
        let stat = Read::is_static(x.at(1, 30));
        b.function(FunctionSpec { name: "quiet".into(), origin: Design::origin(F, 1, 1), params: vec![], ret: ValueType::Bool, locals: vec![], body: stat })
    };
    let mut x = Dx::new(&mut b, F, 4, 9);
    let via_call = x.call(reads_static, vec![], ValueType::Bool);
    let mut x = Dx::new(&mut b, F, 3, 7);
    let now = Read::time(&mut x);
    let two = x.real(2.0);
    let guard = x.bin(piperine_ir::BinaryOp::Lt, now, two, ValueType::Bool);
    let body = b.body(Buck::spec("Bad", F, 1));
    b.carrier(body, Buck::carrier("a", ValueType::Bool, CarrierRole::Out, None, F, 1, 20));
    b.carrier(body, Buck::carrier("c", ValueType::Bool, CarrierRole::Out, None, F, 1, 30));
    b.drive(body, Drive { carrier: 0, value: late, after: None, origin: Design::origin(F, 2, 3) });
    b.drive(body, Drive { carrier: 1, value: via_call, after: None, origin: Design::origin(F, 4, 3) });
    b.event(body, Event { source: EventSource::Changed(0), guard: Some(guard), origin: Design::origin(F, 3, 3) });
    let broken = Design::place(b, body, 0, &[ValueType::Bool, ValueType::Bool], vec![]).check();
    let one = |item: &str, line: u32, what: &str| {
        let hits: Vec<_> = broken.iter().filter(|e| e.item == item && e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{item}: {what}: {broken:#?}");
        assert_eq!(hits[0].origin.line, line, "{item}");
    };
    one("body Bad drive a", 2, "reads time()");
    one("body Bad drive c", 1, "reads static()");
    one("body Bad event 0", 3, "reads time()");
    assert_eq!(broken.len(), 3, "{broken:#?}");

    // A read typed against its form is one entry: `static()` is a `bool`, `time()` an `f64`.
    let mut b = IrBuilder::new();
    let o = Design::origin(F, 2, 5);
    let wrong = b.expr(Expr::Env(Env::Static), ValueType::F64, o.clone());
    let also = b.expr(Expr::Env(Env::Time), ValueType::Bool, o);
    let first = b.stmt(Stmt::Expr(wrong));
    let second = b.stmt(Stmt::Expr(also));
    let body = b.body(BodySpec { analog: Some(vec![first, second]), ..Buck::spec("Typed", F, 1) });
    let broken = Design::place(b, body, 0, &[], vec![]).check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("expression is typed f64, its form gives bool"), 1, "{broken:#?}");
    assert_eq!(has("expression is typed bool, its form gives f64"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 2, "{broken:#?}");
}

#[test]
fn an_at_event_prints_and_checks() {
    // A timer that fires once, `dt` after its start is known, and once at the start itself:
    //
    //   module Pulse(in start: Bit<f64>, out q: Bit = false) { dt: f64 = 1u; }       // 1, 2
    //   digital for Pulse {
    //       on at(start.known_or(0.0) + dt) where dt > 0.0 { q <- true; }             // 3
    //       on at(start) { q <- false; }                                              // 4
    //   }
    const F: &str = "pulse.msl";
    let mut b = IrBuilder::new();
    let body = b.body(BodySpec { params: vec![Buck::param("dt", F, 2, Some(1e-6), None)], ..Buck::spec("Pulse", F, 1) });
    b.carrier(body, Buck::carrier("start", ValueType::F64, CarrierRole::In, None, F, 1, 20));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 45));
    let mut x = Dx::new(&mut b, F, 3, 17);
    let start = x.carrier(0, ValueType::F64);
    let zero = x.real(0.0);
    let known = x.known_or(start, zero, ValueType::F64);
    let dt = x.param(0, ValueType::F64);
    let when = x.bin(piperine_ir::BinaryOp::Add, known, dt, ValueType::F64);
    x.at(3, 50);
    let dt = x.param(0, ValueType::F64);
    let none = x.real(0.0);
    let guard = x.bin(piperine_ir::BinaryOp::Gt, dt, none, ValueType::Bool);
    let first = b.event(body, Event { source: EventSource::At(when), guard: Some(guard), origin: Design::origin(F, 3, 8) });
    let at_start = Dx::new(&mut b, F, 4, 17).carrier(0, ValueType::F64);
    let second = b.event(body, Event { source: EventSource::At(at_start), guard: None, origin: Design::origin(F, 4, 8) });
    let mut x = Dx::new(&mut b, F, 3, 62);
    let yes = x.boolean(true);
    let up = x.commit(1, yes, None);
    x.at(4, 30);
    let no = x.boolean(false);
    let down = x.commit(1, no, None);
    Buck::reaction(&mut b, body, vec![first], vec![up], vec![], F, 3);
    Buck::reaction(&mut b, body, vec![second], vec![down], vec![], F, 4);
    let ir = Design::place(b, body, 0, &[ValueType::F64, ValueType::Bool], vec![Constant::Real(Real::new(1e-6))]);
    assert_eq!(ir.check(), vec![]);

    // Printed after the carrier list as `at <time>`, with its guard as any event has one; a
    // flagged time is as good as a known one.
    Printed::holds(
        &ir,
        &["  event E0 at (start.known_or(0.0) + dt) guard (dt > 0.0)", "  event E1 at start"],
    );
    let full = ir.to_string();
    for line in ["event E0 at (start.known_or(0.0) + dt) guard (dt > 0.0)  @pulse.msl:3:8", "(at e"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // The dependency set of an `at` lists the carriers and the params its time reads; its guard
    // is a target of its own, and no `Cross` target holds anything for it.
    let pulse = ir.top().body();
    assert_eq!(pulse.carrier_reads(Target::At(0)), vec![0]);
    assert_eq!(pulse.dependencies(Target::At(0)), Deps { params: vec![0], ..Deps::default() });
    assert_eq!(pulse.carrier_reads(Target::At(1)), vec![0]);
    assert_eq!(pulse.dependencies(Target::At(1)), Deps::default());
    assert_eq!(pulse.dependencies(Target::Guard(0)), Deps { params: vec![0], ..Deps::default() });
    assert_eq!(pulse.dependencies(Target::Cross(0)), Deps::default());
    assert_eq!(pulse.carrier_reads(Target::Cross(0)), Vec::<u32>::new());

    // A time that is not an `f64`, one that reads a probe and one that reads the clock are one
    // entry each; so is the clock read through a function.
    let mut b = IrBuilder::new();
    let nodes = Design::ports(&mut b, F);
    let mut x = Dx::new(&mut b, F, 2, 14);
    let flag = x.boolean(true);
    let mut x = Dx::new(&mut b, F, 3, 14);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let mut x = Dx::new(&mut b, F, 4, 14);
    let now = Read::time(&mut x);
    let mut x = Dx::new(&mut b, F, 1, 40);
    let sensed = Read::time(&mut x);
    let reads = b.function(FunctionSpec { name: "clock".into(), origin: Design::origin(F, 1, 1), params: vec![], ret: ValueType::F64, locals: vec![], body: sensed });
    let via_call = Dx::new(&mut b, F, 5, 14).call(reads, vec![], ValueType::F64);
    let body = b.body(BodySpec { nodes, ..Buck::spec("Bad", F, 1) });
    for (line, time) in [(2, flag), (3, v), (4, now), (5, via_call)] {
        b.event(body, Event { source: EventSource::At(time), guard: None, origin: Design::origin(F, line, 8) });
    }
    let broken = Design::place(b, body, 2, &[], vec![]).check();
    let one = |item: &str, line: u32, what: &str| {
        let hits: Vec<_> = broken.iter().filter(|e| e.item == item && e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{item}: {what}: {broken:#?}");
        assert_eq!(hits[0].origin.line, line, "{item}: {what}");
    };
    one("body Bad event 0", 2, "an `at` waits for a time, an `f64`, found bool");
    one("body Bad event 1", 3, "an `at` argument reads a probe");
    one("body Bad event 2", 4, "reads time()");
    one("body Bad event 3", 1, "reads time()");
    assert_eq!(broken.len(), 4, "{broken:#?}");
}

/// `Watch`: the reaction of a monitor that commits, checks and stops, with the statements the ABI
/// numbers 0 (the commit), 1 (the nested emit) and 2 (the last emit).
///
/// ```text
/// module Watch(in v: Bit<f64>, in done: Bit, out seen: Bit = false) {}               // 1
/// digital for Watch {
///     on done.rise() {                                                               // 3
///         seen <- true;                                                              // 4
///         if v.is_known() {
///             emit!("checks", kind = "min", name = "vout",
///                   pass = v.known_or(0.0) > 1.0, margin = v.known_or(0.0) - 1.0);  // 6
///         }
///         emit!("sim", finish = true, msg = "done");                                 // 8
///     }
/// }
/// ```
struct Watch;

impl Watch {
    const F: &'static str = "watch.msl";

    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        let body = b.body(Buck::spec("Watch", Watch::F, 1));
        b.carrier(body, Buck::carrier("v", ValueType::F64, CarrierRole::In, None, Watch::F, 1, 20));
        b.carrier(body, Buck::carrier("done", ValueType::Bool, CarrierRole::In, None, Watch::F, 1, 36));
        b.carrier(body, Buck::carrier("seen", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), Watch::F, 1, 52));
        let checks = b.stream(
            body,
            Stream {
                name: "checks".into(),
                fields: vec![StreamField { name: "pass".into(), ty: ValueType::Bool }, StreamField { name: "margin".into(), ty: ValueType::F64 }],
            },
        );
        let sim = b.stream(
            body,
            Stream {
                name: "sim".into(),
                fields: vec![StreamField { name: "finish".into(), ty: ValueType::Bool }, StreamField { name: "failed".into(), ty: ValueType::Bool }],
            },
        );
        let rise = Buck::rise(&mut b, body, 1, Watch::F, 3, 8);
        let mut x = Dx::new(&mut b, Watch::F, 4, 18);
        let yes = x.boolean(true);
        let seen = x.commit(2, yes, None);
        x.at(5, 12);
        let v = x.carrier(0, ValueType::F64);
        let known = x.is_known(v);
        x.at(6, 54);
        let v = x.carrier(0, ValueType::F64);
        let zero = x.real(0.0);
        let level = x.known_or(v, zero, ValueType::F64);
        let one = x.real(1.0);
        let pass = x.bin(piperine_ir::BinaryOp::Gt, level, one, ValueType::Bool);
        let v = x.carrier(0, ValueType::F64);
        let zero = x.real(0.0);
        let level = x.known_or(v, zero, ValueType::F64);
        let one = x.real(1.0);
        let margin = x.bin(piperine_ir::BinaryOp::Sub, level, one, ValueType::F64);
        let check = x.b.stmt(Stmt::Emit {
            stream: checks,
            fields: vec![
                EmitField::Text { name: "kind".into(), text: "min".into() },
                EmitField::Text { name: "name".into(), text: "vout".into() },
                EmitField::Value { field: 0, value: pass },
                EmitField::Value { field: 1, value: margin },
            ],
        });
        let guarded = x.when(known, vec![check], None);
        let guarded = x.stmt(guarded);
        x.at(8, 22);
        let stop = x.boolean(true);
        let last = x.b.stmt(Stmt::Emit {
            stream: sim,
            fields: vec![EmitField::Value { field: 0, value: stop }, EmitField::Text { name: "msg".into(), text: "done".into() }],
        });
        Buck::reaction(&mut b, body, vec![rise], vec![seen, guarded, last], vec![], Watch::F, 3);
        Design::place(b, body, 0, &[ValueType::F64, ValueType::Bool, ValueType::Bool], vec![])
    }
}

#[test]
fn an_emit_site_prints_and_checks() {
    let ir = Watch::ir();
    assert_eq!(ir.check(), vec![]);

    // The streams print after the carriers, with their value fields and types; a site prints its
    // fields in source order, values and texts as written.
    Printed::holds(&ir, &["  stream checks (pass: bool, margin: f64)", "  stream sim (finish: bool, failed: bool)"]);
    Printed::holds(
        &ir,
        &[
            "  reaction R0 on (E0)",
            "    seen <- true",
            "    if v.is_known() { emit checks(kind = \"min\", name = \"vout\", pass = (v.known_or(0.0) > 1.0), margin = (v.known_or(0.0) - 1.0)) } else { () }",
            "    emit sim(finish = true, msg = \"done\")",
        ],
    );
    let full = ir.to_string();
    for line in ["emit s0 (kind \"min\", name \"vout\", f0 e", "emit s1 (f0 e", "stream checks (pass: bool, margin: f64)"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // What the reaction reads: the value through `is_known` and `known_or`, the emits' values
    // included.
    let watch = ir.top().body();
    assert_eq!(watch.carrier_reads(Target::Reaction(0)), vec![0]);

    // An emit counts with the commits, in source order, the nested one in its place: statement 0
    // is the commit, 1 the check, 2 the stop (abi.md: one statement per write).
    let numbered: Vec<&str> = watch
        .reaction_statements(0)
        .into_iter()
        .map(|s| match ir.stmt(s) {
            Stmt::Commit { .. } => "commit",
            Stmt::Emit { stream, .. } => watch.streams()[*stream as usize].name.as_str(),
            other => panic!("not a write: {other:?}"),
        })
        .collect();
    assert_eq!(numbered, vec!["commit", "checks", "sim"]);

    // One entry per defect: a field of a type that is not a value, two fields of one name, a value
    // of another type than its field, a stream and a field out of range, an emit in `analog` and
    // one in a function.
    const F: &str = "bad.msl";
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, F, 6, 30);
    let wrong = x.real(1.0);
    x.at(7, 30);
    let yes = x.boolean(true);
    x.at(8, 30);
    let also = x.boolean(true);
    x.at(9, 30);
    let stray = x.boolean(true);
    let in_reaction = b.stmt(Stmt::Emit { stream: 0, fields: vec![EmitField::Value { field: 1, value: wrong }] });
    let no_stream = b.stmt(Stmt::Emit { stream: 9, fields: vec![EmitField::Value { field: 0, value: yes }] });
    let no_field = b.stmt(Stmt::Emit { stream: 0, fields: vec![EmitField::Value { field: 7, value: also }] });
    let in_analog = b.stmt(Stmt::Emit { stream: 0, fields: vec![EmitField::Value { field: 0, value: stray }] });
    let mut x = Dx::new(&mut b, F, 11, 30);
    let flag = x.boolean(true);
    let in_function = b.stmt(Stmt::Emit { stream: 0, fields: vec![EmitField::Value { field: 0, value: flag }] });
    let unit = Dx::new(&mut b, F, 11, 10).block(vec![in_function], None, ValueType::Unit);
    b.function(FunctionSpec { name: "loud".into(), origin: Design::origin(F, 10, 1), params: vec![], ret: ValueType::Unit, locals: vec![], body: unit });
    let body = b.body(BodySpec { analog: Some(vec![in_analog]), ..Buck::spec("Bad", F, 1) });
    b.carrier(body, Buck::carrier("c", ValueType::Bool, CarrierRole::Out, None, F, 1, 20));
    b.stream(
        body,
        Stream {
            name: "s".into(),
            fields: vec![StreamField { name: "a".into(), ty: ValueType::Bool }, StreamField { name: "a".into(), ty: ValueType::Bool }],
        },
    );
    b.stream(body, Stream { name: "t".into(), fields: vec![StreamField { name: "u".into(), ty: ValueType::Unit }] });
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(F, 5, 8) });
    Buck::reaction(&mut b, body, vec![changed], vec![in_reaction, no_stream, no_field], vec![], F, 5);
    let broken = Design::place(b, body, 0, &[ValueType::Bool], vec![]).check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("a stream field holds `bool`, `f64` or an integer type, found () for u"), 1, "{broken:#?}");
    assert_eq!(has("stream s has two fields named a"), 1, "{broken:#?}");
    assert_eq!(has("the value of an emit field must be bool, found f64"), 1, "{broken:#?}");
    assert_eq!(has("emit to stream 9, out of range (2 streams)"), 1, "{broken:#?}");
    assert_eq!(has("emit writes field 7 of stream 0, out of range (2 fields)"), 1, "{broken:#?}");
    let outside: Vec<&str> = broken.iter().filter(|e| e.what == "an emit outside a reaction").map(|e| e.item.as_str()).collect();
    assert_eq!(outside, vec!["body Bad", "function loud"], "{broken:#?}");
    assert_eq!(broken.len(), 7, "{broken:#?}");
}

#[test]
fn a_record_site_gives_its_stream_and_texts() {
    // A host decodes a record from the instance, the reaction and the statement the solver's sink
    // gives it: statement 1 is the check, statement 2 the stop, statement 0 a commit.
    let ir = Watch::ir();
    let watch = ir.top().body();
    assert_eq!(watch.record_site(0, 1), Some(RecordSite { stream: "checks", texts: vec![("kind", "min"), ("name", "vout")] }));
    assert_eq!(watch.record_site(0, 2), Some(RecordSite { stream: "sim", texts: vec![("msg", "done")] }));
    assert_eq!(watch.record_site(0, 0), None, "a commit is no record");
    assert_eq!(watch.record_site(0, 3), None, "no fourth statement");
    assert_eq!(watch.record_site(1, 0), None, "no second reaction");
    // A stream is named for the body: its fields are the non-text ones, in first-seen order.
    let fields: Vec<(&str, &ValueType)> = watch.streams()[0].fields.iter().map(|f| (f.name.as_str(), &f.ty)).collect();
    assert_eq!(fields, vec![("pass", &ValueType::Bool), ("margin", &ValueType::F64)]);
}

/// The power stage's inductor: its flow is read by the potential it contributes.
///
/// ```text
/// module Inductor(p: Electrical, n: Electrical) { l: f64 = 10u where l > 0.0; r: f64 = 0.01; }   // 1
/// analog { v(p, n) +> ddt(l * i(p, n)), r * i(p, n); }                                           // 2
/// ```
struct Inductor;

impl Inductor {
    const F: &'static str = "inductor.msl";

    fn ir() -> Ir {
        let mut b = IrBuilder::new();
        let check = Buck::positive(&mut b, Inductor::F, 1, 60, 0);
        let mut x = Dx::new(&mut b, Inductor::F, 2, 20);
        let l = x.param(0, ValueType::F64);
        let i = Read::flow(&mut x, 0);
        let flux = x.bin(piperine_ir::BinaryOp::Mul, l, i, ValueType::F64);
        let r = x.param(1, ValueType::F64);
        let i = Read::flow(&mut x, 0);
        let drop = x.bin(piperine_ir::BinaryOp::Mul, r, i, ValueType::F64);
        let contribution = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: piperine_ir::QuantityRole::Potential,
            addends: vec![
                piperine_ir::Addend { operator: piperine_ir::Operator::Ddt, negated: false, value: flux },
                piperine_ir::Addend { operator: piperine_ir::Operator::Direct, negated: false, value: drop },
            ],
        });
        let nodes = Design::ports(&mut b, Inductor::F);
        let body = b.body(BodySpec {
            params: vec![Buck::param("l", Inductor::F, 1, Some(10e-6), Some(check)), Buck::param("r", Inductor::F, 1, Some(0.01), None)],
            nodes,
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Potential }],
            analog: Some(vec![contribution]),
            ..Buck::spec("Inductor", Inductor::F, 1)
        });
        Design::place(b, body, 2, &[], vec![Constant::Real(Real::new(10e-6)), Constant::Real(Real::new(0.01))])
    }
}

impl Read {
    fn flow(x: &mut Dx<'_>, branch: u32) -> ExprId {
        let o = x.origin();
        x.b.expr(Expr::Flow { branch }, ValueType::F64, o)
    }
}

#[test]
fn a_flow_read_and_a_potential_ddt_print_and_check() {
    let ir = Inductor::ir();
    // A potential contribution with a `ddt` addend, reading its own branch's flow, is well formed
    // (contract D5: nothing in `check()` refuses `ddt` under a potential).
    assert_eq!(ir.check(), vec![]);

    // Printed as `flow(b0)`, typed `f64` by its form (no type note in the structural form).
    Printed::holds(&ir, &["  analog", "    b0.potential +> ddt (l * flow(b0)), direct (r * flow(b0))"]);
    assert!(!ir.print_structural().contains(" : "), "a type note was written:\n{}", ir.print_structural());
    let full = ir.to_string();
    assert!(full.contains("flow b0"), "flow b0 missing from:\n{full}");

    // The `ddt` part reads `l` and the flow of branch 0; the direct part reads `r` and the flow.
    // Neither reads a node: the flow is an unknown of its own, and a sum of flows scaled by a
    // param stays affine.
    let body = ir.top().body();
    assert_eq!(body.dependencies(Target::Charge(0)), Deps { params: vec![0], ..Deps::default() });
    assert_eq!(body.flow_reads(Target::Charge(0)), vec![0]);
    assert_eq!(body.dependencies(Target::Flow(0)), Deps { params: vec![1], ..Deps::default() });
    assert_eq!(body.flow_reads(Target::Flow(0)), vec![0]);
    assert!(body.is_affine(0));
    assert_eq!(body.carrier_reads(Target::Flow(0)), Vec::<u32>::new());
    assert_eq!(body.flow_reads(Target::Check(0)), Vec::<u32>::new());

    // A flow read of a branch the body does not drive with a potential, of a branch out of range,
    // and one in a function are one entry each.
    const F: &str = "bad.msl";
    let mut b = IrBuilder::new();
    let mut x = Dx::new(&mut b, F, 3, 14);
    let flow_branch = Read::flow(&mut x, 0);
    x.at(4, 14);
    let missing = Read::flow(&mut x, 5);
    x.at(2, 20);
    let sensed = x.probe(Terminal::Node(0), Terminal::Node(1));
    x.at(7, 30);
    let in_function = Read::flow(&mut x, 0);
    let first = b.stmt(Stmt::Expr(flow_branch));
    let second = b.stmt(Stmt::Expr(missing));
    let drive = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: piperine_ir::QuantityRole::Flow,
        addends: vec![piperine_ir::Addend { operator: piperine_ir::Operator::Direct, negated: false, value: sensed }],
    });
    b.function(FunctionSpec { name: "peek".into(), origin: Design::origin(F, 7, 1), params: vec![], ret: ValueType::F64, locals: vec![], body: in_function });
    let nodes = Design::ports(&mut b, F);
    let body = b.body(BodySpec {
        nodes,
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![drive, first, second]),
        ..Buck::spec("Bad", F, 1)
    });
    let broken = Design::place(b, body, 2, &[], vec![]).check();
    let one = |what: &str, line: u32| {
        let hits: Vec<_> = broken.iter().filter(|e| e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{what}: {broken:#?}");
        assert_eq!(hits[0].origin.line, line, "{what}");
    };
    one("flow of branch 0, which the body does not drive with a potential contribution", 3);
    one("flow of branch 5, out of range (1 branches)", 4);
    one("a flow read outside a body", 7);
    assert_eq!(broken.len(), 3, "{broken:#?}");
}

impl Read {
    fn previous(x: &mut Dx<'_>, carrier: u32, ty: ValueType) -> ExprId {
        let o = x.origin();
        x.b.expr(Expr::Previous(carrier), Ty::flagged(ty), o)
    }
}

#[test]
fn a_previous_value_read_prints_and_checks() {
    // IEEE 1364 posedge, as `std/values.msl` writes `rise`: a change from low or unknown to high
    // or unknown, here as a guard of the event of a flip-flop's clock.
    //
    //   module Edge(in clk: Bit, out q: Bit = false) {}                                   // 1
    //   digital for Edge {
    //       on clk.changed() where !clk.prev().high() && clk.known_or(true) { q <- true; }  // 3
    //       on clk.changed() where clk.prev().is_known() { q <- false; }                    // 4
    //   }
    const F: &str = "edge.msl";
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Edge", F, 1));
    b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, F, 1, 20));
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 35));
    let mut x = Dx::new(&mut b, F, 3, 30);
    let prev = Read::previous(&mut x, 0, ValueType::Bool);
    let was_high = x.high(prev);
    let was_low = x.un(piperine_ir::UnaryOp::Not, was_high, ValueType::Bool);
    let clk = x.carrier(0, ValueType::Bool);
    let yes = x.boolean(true);
    let not_low = x.known_or(clk, yes, ValueType::Bool);
    let rising = x.bin(piperine_ir::BinaryOp::And, was_low, not_low, ValueType::Bool);
    x.at(4, 30);
    let prev = Read::previous(&mut x, 0, ValueType::Bool);
    let was_known = x.is_known(prev);
    let edge = b.event(body, Event { source: EventSource::Changed(0), guard: Some(rising), origin: Design::origin(F, 3, 8) });
    let known = b.event(body, Event { source: EventSource::Changed(0), guard: Some(was_known), origin: Design::origin(F, 4, 8) });
    let mut x = Dx::new(&mut b, F, 3, 70);
    let yes = x.boolean(true);
    let up = x.commit(1, yes, None);
    x.at(4, 50);
    let no = x.boolean(false);
    let down = x.commit(1, no, None);
    Buck::reaction(&mut b, body, vec![edge], vec![up], vec![], F, 3);
    Buck::reaction(&mut b, body, vec![known], vec![down], vec![], F, 4);
    let ir = Design::place(b, body, 0, &[ValueType::Bool, ValueType::Bool], vec![]);
    assert_eq!(ir.check(), vec![]);

    // Printed as the source writes it, typed `flagged<bool>` by its form: no type note.
    Printed::holds(
        &ir,
        &[
            "  event E0 changed clk guard ((!clk.prev().high()) && clk.known_or(true))",
            "  event E1 changed clk guard clk.prev().is_known()",
        ],
    );
    assert!(!ir.print_structural().contains(" : "), "a type note was written:\n{}", ir.print_structural());
    assert!(ir.to_string().contains("previous 0"), "previous 0 missing from:\n{}", ir.to_string());

    // The previous value is read apart from the value: the first guard reads both, the second only
    // the previous one.
    let edge = ir.top().body();
    assert_eq!(edge.carrier_reads(Target::Guard(0)), vec![0]);
    assert_eq!(edge.previous_reads(Target::Guard(0)), vec![0]);
    assert_eq!(edge.carrier_reads(Target::Guard(1)), Vec::<u32>::new());
    assert_eq!(edge.previous_reads(Target::Guard(1)), vec![0]);
    assert_eq!(edge.previous_reads(Target::Reaction(0)), Vec::<u32>::new());
    assert_eq!(edge.dependencies(Target::Guard(1)), Deps::default());

    // `analog` reads a previous value only as the receiver of a flag read, as it reads a carrier
    // (§3.3.5); a carrier out of range and a read typed against its form are entries too. One entry
    // each: `if clk.prev().high() { 1.0 } else { 2.0 }` is accepted, the bare read is not.
    const B: &str = "bad.msl";
    let mut b = IrBuilder::new();
    let nodes = Design::ports(&mut b, B);
    let mut x = Dx::new(&mut b, B, 2, 20);
    let prev = Read::previous(&mut x, 0, ValueType::Bool);
    let was_high = x.high(prev);
    let one = x.real(1.0);
    let two = x.real(2.0);
    let level = x.if_(was_high, one, two, ValueType::F64);
    x.at(3, 14);
    let bare = Read::previous(&mut x, 0, ValueType::Bool);
    x.at(4, 14);
    let missing = Read::previous(&mut x, 9, ValueType::Bool);
    let missing = x.high(missing);
    x.at(5, 14);
    let wrong = Read::previous(&mut x, 0, ValueType::F64);
    let drive = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: piperine_ir::QuantityRole::Flow,
        addends: vec![piperine_ir::Addend { operator: piperine_ir::Operator::Direct, negated: false, value: level }],
    });
    let bare = b.stmt(Stmt::Expr(bare));
    let missing = b.stmt(Stmt::Expr(missing));
    let wrong = b.stmt(Stmt::Expr(wrong));
    let body = b.body(BodySpec {
        nodes,
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![drive, bare, missing]),
        ..Buck::spec("Bad", B, 1)
    });
    b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, B, 1, 20));
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(B, 6, 8) });
    Buck::reaction(&mut b, body, vec![changed], vec![wrong], vec![], B, 6);
    let broken = Design::place(b, body, 2, &[ValueType::Bool], vec![]).check();
    let one = |what: &str, line: u32| {
        let hits: Vec<_> = broken.iter().filter(|e| e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{what}: {broken:#?}");
        assert_eq!(hits[0].origin.line, line, "{what}");
    };
    one("`analog` reads the previous value of carrier clk outside `known_or`, `is_known` and `high` (§3.3.5)", 3);
    one("previous value of carrier 9, out of range (1 carriers)", 4);
    one("expression is typed flagged<f64>, its form gives flagged<bool>", 5);
    assert_eq!(broken.len(), 3, "{broken:#?}");
}

/// A parent that hands its own param to a child, beside one that gives a constant.
///
/// ```text
/// pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0 }          // devices.msl
/// module Pair(p: Electrical, n: Electrical) {
///     r_top: f64 = 2k where r_top > 0.0;                                               // 2
///     Resistor(p, n) { r: r_top * 2.0 } as ra;                                          // 3
///     Resistor(p, gnd) { r: 50.0 } as rb;                                               // 4
/// }
/// ```
struct Pair;

impl Pair {
    const F: &'static str = "pair.msl";

    /// The design, and the expression of `ra`'s `r` for the tests that read it.
    fn ir(mutate: impl FnOnce(&mut IrBuilder, BodyId, ExprId) -> Vec<(u32, u32, ChildValue)>) -> Ir {
        let mut b = IrBuilder::new();
        let resistor = Devices::resistor(&mut b);
        let el = Buck::electrical(&mut b);
        let check = Buck::positive(&mut b, Pair::F, 2, 30, 0);
        let mut x = Dx::new(&mut b, Pair::F, 3, 30);
        let r_top = x.param(0, ValueType::F64);
        let two = x.real(2.0);
        let doubled = x.bin(piperine_ir::BinaryOp::Mul, r_top, two, ValueType::F64);
        let child = |name: &str, nodes: Vec<Terminal>, params: Vec<(u32, Constant)>, line| Child {
            name: name.into(),
            body: resistor,
            nodes,
            params,
            origin: Design::origin(Pair::F, line, 5),
        };
        let port = |name: &str, col| piperine_ir::BodyNode { name: name.into(), nature: el, role: piperine_ir::NodeRole::Port, init: None, origin: Design::origin(Pair::F, 1, col) };
        let body = b.body(BodySpec {
            params: vec![Buck::param("r_top", Pair::F, 2, Some(2000.0), Some(check))],
            nodes: vec![port("p", 10), port("n", 28)],
            children: vec![
                child("ra", vec![Terminal::Node(0), Terminal::Node(1)], vec![], 3),
                child("rb", vec![Terminal::Node(0), Terminal::Ground], vec![(0, Constant::Real(Real::new(50.0)))], 4),
            ],
            ..Buck::spec("Pair", Pair::F, 1)
        });
        for (child, param, value) in mutate(&mut b, body, doubled) {
            b.child_param(body, child, param, value);
        }
        let at = b.next_instance_id();
        let nodes: Vec<NodeRef> =
            ["p", "n"].iter().map(|n| NodeRef::Node(b.node(NodeSpec { path: format!("Pair.{n}"), nature: el, declared_by: at, init: None, origin: Design::origin(Pair::F, 1, 1) }))).collect();
        let top = b.instance(InstanceSpec { path: "Pair".into(), name: "Pair".into(), body, parent: None, nodes: nodes.clone(), params: vec![Constant::Real(Real::new(2000.0))], origin: Design::origin(Pair::F, 1, 1) });
        // The flat table holds the constant each value elaborates to.
        for (name, ports, r) in [("ra", vec![nodes[0], nodes[1]], 4000.0), ("rb", vec![nodes[0], NodeRef::Ground], 50.0)] {
            b.instance(InstanceSpec {
                path: format!("Pair.{name}"),
                name: name.into(),
                body: resistor,
                parent: Some(top),
                nodes: ports,
                params: vec![Constant::Real(Real::new(r))],
                origin: Design::origin(Pair::F, 3, 5),
            });
        }
        b.finish(top)
    }
}

#[test]
fn a_child_param_may_read_the_parents_params() {
    // `ra`'s `r` is `r_top * 2.0`, an expression over the parent's param; `rb`'s is the constant.
    let ir = Pair::ir(|_, _, doubled| vec![(0, 0, ChildValue::Expr(doubled))]);
    assert_eq!(ir.check(), vec![]);
    // The authored tree shows the expression where the placement wrote it, and the constant
    // where it wrote one; the flat instances hold the constants.
    Printed::holds(
        &ir,
        &[
            "  child ra: Resistor  nodes (p, n)  params (r = (r_top * 2.0))",
            "  child rb: Resistor  nodes (p, gnd)  params (r = 50.0)",
        ],
    );
    Printed::holds(&ir, &["instance Pair.ra: Resistor  nodes (Pair.p, Pair.n)  params (r = 4000.0)"]);
    let full = ir.to_string();
    assert!(full.contains("params (r = (r_top * 2.0))"), "the expression missing from:\n{full}");
    assert!(full.contains(" values (p0=e"), "the arena value missing from:\n{full}");
    // The accessor gives the entries of each child, in order; a child with none has none.
    let pair = ir.top().body();
    assert!(matches!(pair.child_params(0), [(0, ChildValue::Expr(_))]));
    assert_eq!(pair.child_params(1), &[]);
    assert_eq!(pair.child_params(7), &[]);
    // `Child::params` keeps its constants: the literal `Child` is unchanged.
    assert_eq!(pair.children()[1].params, vec![(0, Constant::Real(Real::new(50.0)))]);
    assert_eq!(pair.children()[0].params, vec![]);

    // A constant set beside the child prints as the constant does.
    let ir = Pair::ir(|_, _, _| vec![(0, 0, ChildValue::Const(Constant::Real(Real::new(75.0))))]);
    assert_eq!(ir.check(), vec![]);
    Printed::holds(&ir, &["  child ra: Resistor  nodes (p, n)  params (r = 75.0)"]);

    // An expression that reads more than the parent's params is one entry each: a node, a carrier
    // (through a function), the environment and a local; one of the wrong type, a param out of
    // range, a param set again, a constant of the wrong type, a child out of range.
    let broken = Pair::ir(|b, body, _| {
        let mut x = Dx::new(b, Pair::F, 5, 20);
        let probe = x.probe(Terminal::Node(0), Terminal::Node(1));
        x.at(6, 20);
        let o = x.origin();
        let temperature = x.b.expr(Expr::Env(Env::Temperature), ValueType::F64, o);
        x.at(7, 20);
        let local = x.local(0, ValueType::F64);
        x.at(8, 20);
        let flag = x.boolean(true);
        let _ = body;
        vec![
            (0, 0, ChildValue::Expr(probe)),
            (1, 0, ChildValue::Expr(temperature)),
            (1, 0, ChildValue::Expr(local)),
            (0, 3, ChildValue::Const(Constant::Real(Real::new(1.0)))),
            (0, 0, ChildValue::Expr(flag)),
            (0, 0, ChildValue::Const(Constant::Bool(true))),
            (4, 0, ChildValue::Const(Constant::Real(Real::new(1.0)))),
        ]
    })
    .check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("reads a probe: only the parent's params may be read"), 1, "{broken:#?}");
    assert_eq!(has("reads an environment read: only the parent's params may be read"), 1, "{broken:#?}");
    assert_eq!(has("reads a local, a block or a closure: only the parent's params may be read"), 1, "{broken:#?}");
    assert_eq!(has("child ra: param index 3 out of range"), 1, "{broken:#?}");
    assert_eq!(has("expression must be f64, found bool"), 1, "{broken:#?}");
    assert_eq!(has("child rb: param r is set twice"), 1, "{broken:#?}");
    assert_eq!(has("child ra: param r is set twice"), 2, "the second and the third value of `r`: {broken:#?}");
    assert_eq!(has("local 0 out of range (0 locals)"), 1, "{broken:#?}");
    assert_eq!(has("the value true of param r is not a value of its type f64"), 1, "{broken:#?}");
    assert_eq!(has("param values of child 4, out of range (2 children)"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 11, "{broken:#?}");
}

#[test]
fn the_earlier_prints_are_unchanged() {
    // No value beside a child, no line: every M1 fixture and every buck-slice fixture prints, in
    // both forms, the bytes of the golden text it was committed with.
    let read = |dir: &str, name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(dir).join(format!("{name}.txt"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    let device = |make: fn(&mut IrBuilder) -> BodyId, name: &str, line: u32, params: Vec<f64>| {
        let mut b = IrBuilder::new();
        let body = make(&mut b);
        let top = Devices::alone(&mut b, body, name, line, params);
        b.finish(top)
    };
    let mut b = IrBuilder::new();
    let top = Devices::divider(&mut b);
    let m1 = [
        ("resistor", device(Devices::resistor, "resistor", 5, vec![1000.0])),
        ("capacitor", device(Devices::capacitor, "capacitor", 11, vec![1e-12])),
        ("vsource", device(Devices::vsource, "vsource", 17, vec![10.0, 0.0])),
        ("isource", device(Devices::isource, "isource", 24, vec![1e-3, 0.0])),
        ("diode", device(Devices::diode, "diode", 31, vec![1e-14, 1.0, 0.0])),
        ("divider", b.finish(top)),
    ];
    for (name, ir) in m1 {
        assert_eq!(ir.check(), vec![], "{name}");
        assert_eq!(ir.print_structural(), read("tests/golden", name), "{name}: structural form");
        assert_eq!(ir.to_string(), read("tests/golden/full", name), "{name}: full text");
    }
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
        let mut b = IrBuilder::new();
        let _body = make(&mut b);
        let top_body = b.body(Buck::spec("Top", "top.msl", 1));
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body: top_body, parent: None, nodes: vec![], params: vec![], origin: Design::origin("top.msl", 1, 1) });
        let ir = b.finish(top);
        assert_eq!(ir.check(), vec![], "{name}");
        assert_eq!(ir.print_structural(), read("tests/golden/buck", name), "{name}: structural form");
        assert_eq!(ir.to_string(), read("tests/golden/buck/full", name), "{name}: full text");
    }
}
