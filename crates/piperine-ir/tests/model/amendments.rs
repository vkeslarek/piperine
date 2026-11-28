//! Scope: the IR amendments of msl-digital (design A1 to A7, contract D3 to D7): the time and
//! static reads, the `at` event, emit sites and streams, the flow of a potential branch, the
//! previous value of a carrier, a child param that reads the parent, and the carrier bindings
//! of a child. Each is an IR node that `Ir::check` types and the printer writes, with its
//! dependency set; the earlier prints stay byte-identical. Expected texts are written from the
//! MSL source each fixture transcribes.

use piperine_ir_fixtures::digital;

use piperine_ir_fixtures::devices;

use devices::Devices;
use digital::{Buck, Dx, ValueAliases};
use super::printed::Printed;
use piperine_ir::{
    AnalysisKind, BodyId, BodySpec, Branch, BranchKind, CarrierRole, Child, ChildValue, Constant, Deps, Drive, EmitField, Env, Event, EventSource, Expr, ExprId, FunctionSpec, InstanceSpec, Ir,
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
        b.bind_nets(top, nets).unwrap();
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
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 40)).expect("a body this builder handed out");
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(F, 3, 8) }).expect("a body this builder handed out");
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
    b.carrier(body, Buck::carrier("a", ValueType::Bool, CarrierRole::Out, None, F, 1, 20)).expect("a body this builder handed out");
    b.carrier(body, Buck::carrier("c", ValueType::Bool, CarrierRole::Out, None, F, 1, 30)).expect("a body this builder handed out");
    b.drive(body, Drive { carrier: 0, value: late, after: None, origin: Design::origin(F, 2, 3), element: None, path: vec![] }).expect("a body this builder handed out");
    b.drive(body, Drive { carrier: 1, value: via_call, after: None, origin: Design::origin(F, 4, 3), element: None, path: vec![] }).expect("a body this builder handed out");
    b.event(body, Event { source: EventSource::Changed(0), guard: Some(guard), origin: Design::origin(F, 3, 3) }).expect("a body this builder handed out");
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
    //       on at(start + dt) where dt > 0.0 { q <- true; }                           // 3
    //       on at(start) { q <- false; }                                              // 4
    //   }
    const F: &str = "pulse.msl";
    let mut b = IrBuilder::new();
    let body = b.body(BodySpec { params: vec![Buck::param("dt", F, 2, Some(1e-6), None)], ..Buck::spec("Pulse", F, 1) });
    b.carrier(body, Buck::carrier("start", ValueType::F64, CarrierRole::In, None, F, 1, 20)).expect("a body this builder handed out");
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 45)).expect("a body this builder handed out");
    let mut x = Dx::new(&mut b, F, 3, 17);
    let start = x.carrier(0, ValueType::F64);
    let dt = x.param(0, ValueType::F64);
    let when = x.bin(piperine_ir::BinaryOp::Add, start, dt, ValueType::F64);
    x.at(3, 50);
    let dt = x.param(0, ValueType::F64);
    let none = x.real(0.0);
    let guard = x.bin(piperine_ir::BinaryOp::Gt, dt, none, ValueType::Bool);
    let first = b.event(body, Event { source: EventSource::At(when), guard: Some(guard), origin: Design::origin(F, 3, 8) }).expect("a body this builder handed out");
    let at_start = Dx::new(&mut b, F, 4, 17).carrier(0, ValueType::F64);
    let second = b.event(body, Event { source: EventSource::At(at_start), guard: None, origin: Design::origin(F, 4, 8) }).expect("a body this builder handed out");
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

    // Printed after the carrier list as `at <time>`, with its guard as any event has one.
    Printed::holds(
        &ir,
        &["  event E0 at (start + dt) guard (dt > 0.0)", "  event E1 at start"],
    );
    let full = ir.to_string();
    for line in ["event E0 at (start + dt) guard (dt > 0.0)  @pulse.msl:3:8", "(at e"] {
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
        b.event(body, Event { source: EventSource::At(time), guard: None, origin: Design::origin(F, line, 8) }).expect("a body this builder handed out");
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
///         if v > 0.0 {
///             emit!("checks", kind = "min", name = "vout",
///                   pass = v > 1.0, margin = v - 1.0);                               // 6
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
        b.carrier(body, Buck::carrier("v", ValueType::F64, CarrierRole::In, None, Watch::F, 1, 20)).expect("a body this builder handed out");
        b.carrier(body, Buck::carrier("done", ValueType::Bool, CarrierRole::In, None, Watch::F, 1, 36)).expect("a body this builder handed out");
        b.carrier(body, Buck::carrier("seen", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), Watch::F, 1, 52)).expect("a body this builder handed out");
        let checks = b.stream(
            body,
            Stream {
                name: "checks".into(),
                fields: vec![StreamField { name: "pass".into(), ty: ValueType::Bool }, StreamField { name: "margin".into(), ty: ValueType::F64 }],
            },
        ).expect("a body this builder handed out");
        let sim = b.stream(
            body,
            Stream {
                name: "sim".into(),
                fields: vec![StreamField { name: "finish".into(), ty: ValueType::Bool }, StreamField { name: "failed".into(), ty: ValueType::Bool }],
            },
        ).expect("a body this builder handed out");
        let rise = Buck::rise(&mut b, body, 1, Watch::F, 3, 8);
        let mut x = Dx::new(&mut b, Watch::F, 4, 18);
        let yes = x.boolean(true);
        let seen = x.commit(2, yes, None);
        x.at(5, 12);
        let v = x.carrier(0, ValueType::F64);
        let zero = x.real(0.0);
        let seen_v = x.bin(piperine_ir::BinaryOp::Gt, v, zero, ValueType::Bool);
        x.at(6, 54);
        let level = x.carrier(0, ValueType::F64);
        let one = x.real(1.0);
        let pass = x.bin(piperine_ir::BinaryOp::Gt, level, one, ValueType::Bool);
        let level = x.carrier(0, ValueType::F64);
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
        let guarded = x.when(seen_v, vec![check], None);
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
            "    if (v > 0.0) { emit checks(kind = \"min\", name = \"vout\", pass = (v > 1.0), margin = (v - 1.0)) } else { () }",
            "    emit sim(finish = true, msg = \"done\")",
        ],
    );
    let full = ir.to_string();
    for line in ["emit s0 (kind \"min\", name \"vout\", f0 e", "emit s1 (f0 e", "stream checks (pass: bool, margin: f64)"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // What the reaction reads: the value through the guard, the emits' values included.
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
    b.carrier(body, Buck::carrier("c", ValueType::Bool, CarrierRole::Out, None, F, 1, 20)).expect("a body this builder handed out");
    b.stream(
        body,
        Stream {
            name: "s".into(),
            fields: vec![StreamField { name: "a".into(), ty: ValueType::Bool }, StreamField { name: "a".into(), ty: ValueType::Bool }],
        },
    ).expect("a body this builder handed out");
    b.stream(body, Stream { name: "t".into(), fields: vec![StreamField { name: "u".into(), ty: ValueType::Unit }] }).expect("a body this builder handed out");
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(F, 5, 8) }).expect("a body this builder handed out");
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
        x.b.expr(Expr::Previous(carrier), ty, o)
    }
}

#[test]
fn a_previous_value_read_prints_and_checks() {
    // A rising edge as `std/values.msl` writes `rise`: a change from low to high, here as a guard
    // of the event of a flip-flop's clock, and a guard that only reads the previous value.
    //
    //   module Edge(in clk: Bit, out q: Bit = false) {}                                   // 1
    //   digital for Edge {
    //       on clk.changed() where !clk.prev() && clk { q <- true; }                        // 3
    //       on clk.changed() where clk.prev() { q <- false; }                               // 4
    //   }
    const F: &str = "edge.msl";
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Edge", F, 1));
    b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, F, 1, 20)).expect("a body this builder handed out");
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 35)).expect("a body this builder handed out");
    let mut x = Dx::new(&mut b, F, 3, 30);
    let prev = Read::previous(&mut x, 0, ValueType::Bool);
    let was_low = x.un(piperine_ir::UnaryOp::Not, prev, ValueType::Bool);
    let clk = x.carrier(0, ValueType::Bool);
    let rising = x.bin(piperine_ir::BinaryOp::And, was_low, clk, ValueType::Bool);
    x.at(4, 30);
    let was_high = Read::previous(&mut x, 0, ValueType::Bool);
    let edge = b.event(body, Event { source: EventSource::Changed(0), guard: Some(rising), origin: Design::origin(F, 3, 8) }).expect("a body this builder handed out");
    let known = b.event(body, Event { source: EventSource::Changed(0), guard: Some(was_high), origin: Design::origin(F, 4, 8) }).expect("a body this builder handed out");
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

    // Printed as the source writes it, typed by its form: no type note.
    Printed::holds(
        &ir,
        &[
            "  event E0 changed clk guard ((!clk.prev()) && clk)",
            "  event E1 changed clk guard clk.prev()",
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

    // a carrier out of range and a read typed against its form are entries; a read of the previous value in
    // `analog` is as plain as any other (§3.3.6).
    const B: &str = "bad.msl";
    let mut b = IrBuilder::new();
    let nodes = Design::ports(&mut b, B);
    let mut x = Dx::new(&mut b, B, 2, 20);
    let prev = Read::previous(&mut x, 0, ValueType::Bool);
    let one = x.real(1.0);
    let two = x.real(2.0);
    let level = x.if_(prev, one, two, ValueType::F64);
    x.at(3, 14);
    let bare = Read::previous(&mut x, 0, ValueType::Bool);
    x.at(4, 14);
    let missing = Read::previous(&mut x, 9, ValueType::Bool);
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
    b.carrier(body, Buck::carrier("clk", ValueType::Bool, CarrierRole::In, None, B, 1, 20)).expect("a body this builder handed out");
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(B, 6, 8) }).expect("a body this builder handed out");
    Buck::reaction(&mut b, body, vec![changed], vec![wrong], vec![], B, 6);
    let broken = Design::place(b, body, 2, &[ValueType::Bool], vec![]).check();
    let one = |what: &str, line: u32| {
        let hits: Vec<_> = broken.iter().filter(|e| e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "{what}: {broken:#?}");
        assert_eq!(hits[0].origin.line, line, "{what}");
    };
    one("previous value of carrier 9, out of range (1 carriers)", 4);
    one("expression is typed f64, its form gives bool", 5);
    assert_eq!(broken.len(), 2, "{broken:#?}");
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
            b.child_param(body, child, param, value).expect("a body this builder handed out");
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

/// The power stage: the placements connect nodes and carriers, and the authored tree says which.
///
/// ```text
/// module Stage(vin: Electrical, sw: Electrical, in hs_cmd: Bit, in ls_cmd: Bit) {      // 1
///     signal hs_gate: Bit; signal ls_gate: Bit;                                         // 2
///     GateDriver(hs_cmd, ls_cmd, hs_gate, ls_gate) as drv;                              // 3
///     Switch(vin, sw, hs_gate) as hs;                                                   // 4
///     Switch(sw, gnd, ls_gate) as ls;                                                   // 5
///     GateDriver(hs_cmd, ls_cmd, _, _) as spare;                                        // 6
///     Switch(vin, gnd, _) as bare;                                                      // 7
/// }
/// ```
struct Stage;

impl Stage {
    const F: &'static str = "stage.msl";

    /// `bindings` are the `(child, carriers)` the test records; `extra` is a fifth carrier.
    fn ir(bindings: Vec<(u32, Vec<Option<u32>>)>, extra: Option<ValueType>) -> Ir {
        let mut b = IrBuilder::new();
        let driver = Buck::gate_driver(&mut b);
        let switch = Buck::switch(&mut b);
        let el = Buck::electrical(&mut b);
        let child = |name: &str, body: BodyId, nodes: Vec<Terminal>, line: u32| Child { name: name.into(), body, nodes, params: vec![], origin: Design::origin(Stage::F, line, 5) };
        let port = |name: &str, col| piperine_ir::BodyNode { name: name.into(), nature: el, role: piperine_ir::NodeRole::Port, init: None, origin: Design::origin(Stage::F, 1, col) };
        let body = b.body(BodySpec {
            nodes: vec![port("vin", 14), port("sw", 32)],
            children: vec![
                child("drv", driver, vec![], 3),
                child("hs", switch, vec![Terminal::Node(0), Terminal::Node(1)], 4),
                child("ls", switch, vec![Terminal::Node(1), Terminal::Ground], 5),
                child("spare", driver, vec![], 6),
                child("bare", switch, vec![Terminal::Node(0), Terminal::Ground], 7),
            ],
            ..Buck::spec("Stage", Stage::F, 1)
        });
        let mut types = vec![ValueType::Bool; 4];
        for (name, role, col) in [("hs_cmd", CarrierRole::In, 50), ("ls_cmd", CarrierRole::In, 66), ("hs_gate", CarrierRole::Signal, 15), ("ls_gate", CarrierRole::Signal, 35)] {
            b.carrier(body, Buck::carrier(name, ValueType::Bool, role.clone(), None, Stage::F, if role == CarrierRole::Signal { 2 } else { 1 }, col)).expect("a body this builder handed out");
        }
        if let Some(ty) = extra {
            b.carrier(body, Buck::carrier("count", ty.clone(), CarrierRole::Signal, None, Stage::F, 2, 55)).expect("a body this builder handed out");
            types.push(ty);
        }
        for (child, carriers) in bindings {
            b.bind_child_carriers(body, child, carriers).expect("a body this builder handed out");
        }
        Design::place(b, body, 2, &types, vec![])
    }
}

#[test]
fn a_child_records_its_carrier_bindings() {
    let all = vec![
        (0, vec![Some(0), Some(1), Some(2), Some(3)]),
        (1, vec![Some(2)]),
        (2, vec![Some(3)]),
        (3, vec![Some(0), Some(1), None, None]),
    ];
    let ir = Stage::ir(all, None);
    assert_eq!(ir.check(), vec![]);

    // The authored tree shows the carriers a placement connects after its nodes and params, by the
    // parent's names (`_` for an open port); a placement with no record writes no line.
    Printed::holds(
        &ir,
        &[
            "  child drv: GateDriver  nodes ()  params ()  carriers (hs_cmd, ls_cmd, hs_gate, ls_gate)",
            "  child hs: Switch  nodes (vin, sw)  params ()  carriers (hs_gate)",
            "  child ls: Switch  nodes (sw, gnd)  params ()  carriers (ls_gate)",
            "  child spare: GateDriver  nodes ()  params ()  carriers (hs_cmd, ls_cmd, _, _)",
            "  child bare: Switch  nodes (vin, gnd)  params ()",
        ],
    );
    let full = ir.to_string();
    assert!(full.contains(" carriers (c0, c1, c2, c3)"), "the arena binding missing from:\n{full}");
    assert!(full.contains(" carriers (c0, c1, -, -)"), "the open ports missing from the arena:\n{full}");

    // The accessor gives one entry per carrier port in the child's order: the parent's carrier or
    // open. A child with no record, or none at all, has none; the tree is `Child` plus this.
    let stage = ir.top().body();
    assert_eq!(stage.child_carriers(0), &[Some(0), Some(1), Some(2), Some(3)]);
    assert_eq!(stage.child_carriers(3), &[Some(0), Some(1), None, None]);
    assert_eq!(stage.child_carriers(4), &[]);
    assert_eq!(stage.child_carriers(9), &[]);

    // A record short of a port, a carrier out of range, a carrier of another type and a child out
    // of range are one entry each.
    let broken = Stage::ir(
        vec![(3, vec![Some(0)]), (1, vec![Some(9)]), (2, vec![Some(4)]), (8, vec![Some(0)])],
        Some(ValueAliases::u(4)),
    )
    .check();
    let has = |what: &str| broken.iter().filter(|e| e.what.contains(what)).count();
    assert_eq!(has("child spare binds 1 carriers, body GateDriver has 4 carrier ports"), 1, "{broken:#?}");
    assert_eq!(has("child hs: carrier 9 out of range (5 carriers)"), 1, "{broken:#?}");
    assert_eq!(has("child ls binds carrier count: u4 to the port g: bool"), 1, "{broken:#?}");
    assert_eq!(has("carrier bindings of child 8, out of range (5 children)"), 1, "{broken:#?}");
    assert_eq!(broken.len(), 4, "{broken:#?}");
}

#[test]
fn the_new_method_rows_type_their_operands_and_results() {
    // The rows of Appendix H.1 and H.2 the IR gains, each written as the MSL call it stands for
    // (`x.sin()`, `x.powi(2)`, `a.add_overflows(b)`), typed by the operand and result types of
    // the spec's signature: `powi` takes an `i32`, `pow` a `u32`, `is_nan` and the `*_overflows`
    // tests give `bool`, the rest give their receiver's type.
    use piperine_ir::Method as M;
    const F: &str = "rows.msl";
    let f = ValueType::F64;
    let (i8_, i32_, u8_, u32_) = (ValueAliases::i(8), ValueAliases::i(32), ValueAliases::u(8), ValueAliases::u(32));
    // (row, MSL name, receiver, argument types, result)
    let rows: Vec<(M, &str, ValueType, Vec<ValueType>, ValueType)> = vec![
        (M::Sin, "sin", f.clone(), vec![], f.clone()),
        (M::Cos, "cos", f.clone(), vec![], f.clone()),
        (M::Tan, "tan", f.clone(), vec![], f.clone()),
        (M::Asin, "asin", f.clone(), vec![], f.clone()),
        (M::Acos, "acos", f.clone(), vec![], f.clone()),
        (M::Atan, "atan", f.clone(), vec![], f.clone()),
        (M::Atan2, "atan2", f.clone(), vec![f.clone()], f.clone()),
        (M::Sinh, "sinh", f.clone(), vec![], f.clone()),
        (M::Cosh, "cosh", f.clone(), vec![], f.clone()),
        (M::Tanh, "tanh", f.clone(), vec![], f.clone()),
        (M::Powf, "powf", f.clone(), vec![f.clone()], f.clone()),
        (M::Powi, "powi", f.clone(), vec![i32_.clone()], f.clone()),
        (M::Log10, "log10", f.clone(), vec![], f.clone()),
        (M::Log2, "log2", f.clone(), vec![], f.clone()),
        (M::Ceil, "ceil", f.clone(), vec![], f.clone()),
        (M::Round, "round", f.clone(), vec![], f.clone()),
        (M::Trunc, "trunc", f.clone(), vec![], f.clone()),
        (M::Fract, "fract", f.clone(), vec![], f.clone()),
        (M::Signum, "signum", f.clone(), vec![], f.clone()),
        (M::Signum, "signum", i8_.clone(), vec![], i8_.clone()),
        (M::RemEuclid, "rem_euclid", f.clone(), vec![f.clone()], f.clone()),
        (M::RemEuclid, "rem_euclid", u8_.clone(), vec![u8_.clone()], u8_.clone()),
        (M::Hypot, "hypot", f.clone(), vec![f.clone()], f.clone()),
        (M::IsNan, "is_nan", f.clone(), vec![], ValueType::Bool),
        (M::IsFinite, "is_finite", f.clone(), vec![], ValueType::Bool),
        (M::IsInfinite, "is_infinite", f.clone(), vec![], ValueType::Bool),
        (M::IntAbs, "abs", i8_.clone(), vec![], i8_.clone()),
        (M::Pow, "pow", i8_.clone(), vec![u32_.clone()], i8_.clone()),
        (M::Pow, "pow", u8_.clone(), vec![u32_.clone()], u8_.clone()),
        (M::SaturatingAdd, "saturating_add", u8_.clone(), vec![u8_.clone()], u8_.clone()),
        (M::SaturatingSub, "saturating_sub", i8_.clone(), vec![i8_.clone()], i8_.clone()),
        (M::SaturatingMul, "saturating_mul", i8_.clone(), vec![i8_.clone()], i8_.clone()),
        (M::AddOverflows, "add_overflows", u8_.clone(), vec![u8_.clone()], ValueType::Bool),
        (M::SubOverflows, "sub_overflows", i8_.clone(), vec![i8_.clone()], ValueType::Bool),
        (M::MulOverflows, "mul_overflows", i8_.clone(), vec![i8_.clone()], ValueType::Bool),
    ];
    let value = |x: &mut Dx<'_>, ty: &ValueType| match ty {
        ValueType::F64 => x.real(0.5),
        other => x.int(1, other.clone()),
    };

    // Every row, written with its signature's operand types, checks clean and prints its name.
    let mut b = IrBuilder::new();
    let mut stmts = Vec::new();
    for (k, (method, _, recv, args, result)) in rows.iter().enumerate() {
        let mut x = Dx::new(&mut b, F, 1 + k as u32, 1);
        let r = value(&mut x, recv);
        let a: Vec<ExprId> = args.iter().map(|t| value(&mut x, t)).collect();
        let call = x.method(*method, r, a, result.clone());
        stmts.push(b.stmt(Stmt::Expr(call)));
    }
    let body = b.body(BodySpec { analog: Some(stmts), ..Buck::spec("Rows", F, 1) });
    let ir = Design::place(b, body, 0, &[], vec![]);
    assert_eq!(ir.check(), vec![]);
    // The printed calls, one per row in order, written from the MSL each stands for.
    Printed::holds(
        &ir,
        &[
            "  analog",
            "    0.5.sin()",
            "    0.5.cos()",
            "    0.5.tan()",
            "    0.5.asin()",
            "    0.5.acos()",
            "    0.5.atan()",
            "    0.5.atan2(0.5)",
            "    0.5.sinh()",
            "    0.5.cosh()",
            "    0.5.tanh()",
            "    0.5.powf(0.5)",
            "    0.5.powi(1i32)",
            "    0.5.log10()",
            "    0.5.log2()",
            "    0.5.ceil()",
            "    0.5.round()",
            "    0.5.trunc()",
            "    0.5.fract()",
            "    0.5.signum()",
            "    1i8.signum()",
            "    0.5.rem_euclid(0.5)",
            "    1u8.rem_euclid(1u8)",
            "    0.5.hypot(0.5)",
            "    0.5.is_nan()",
            "    0.5.is_finite()",
            "    0.5.is_infinite()",
            "    1i8.abs()",
            "    1i8.pow(1u32)",
            "    1u8.pow(1u32)",
            "    1u8.saturating_add(1u8)",
            "    1i8.saturating_sub(1i8)",
            "    1i8.saturating_mul(1i8)",
            "    1u8.add_overflows(1u8)",
            "    1i8.sub_overflows(1i8)",
            "    1i8.mul_overflows(1i8)",
        ],
    );

    // A row written against its signature is one entry each: `powi` with an `f64` exponent,
    // `pow` with a signed exponent, `abs` and `signum` of an unsigned integer (H.1: signed types
    // only), an `f64` row on an integer, an integer row on an `f64`, a result typed other than
    // the signature's, and a wrong count.
    let wrong: Vec<(M, ValueType, Vec<ValueType>, ValueType)> = vec![
        (M::Powi, f.clone(), vec![f.clone()], f.clone()),
        (M::Pow, i8_.clone(), vec![i32_.clone()], i8_.clone()),
        (M::IntAbs, u8_.clone(), vec![], u8_.clone()),
        (M::Signum, u8_.clone(), vec![], u8_.clone()),
        (M::Sin, i8_.clone(), vec![], i8_.clone()),
        (M::SaturatingAdd, f.clone(), vec![f.clone()], f.clone()),
        (M::IsNan, f.clone(), vec![], f.clone()),
        (M::AddOverflows, u8_.clone(), vec![u8_.clone()], u8_.clone()),
        (M::Atan2, f.clone(), vec![], f.clone()),
    ];
    let mut b = IrBuilder::new();
    let mut stmts = Vec::new();
    for (k, (method, recv, args, result)) in wrong.iter().enumerate() {
        let mut x = Dx::new(&mut b, F, 1 + k as u32, 1);
        let r = value(&mut x, recv);
        let a: Vec<ExprId> = args.iter().map(|t| value(&mut x, t)).collect();
        let call = x.method(*method, r, a, result.clone());
        stmts.push(b.stmt(Stmt::Expr(call)));
    }
    let body = b.body(BodySpec { analog: Some(stmts), ..Buck::spec("Wrong", F, 1) });
    let broken = Design::place(b, body, 0, &[], vec![]).check();
    for k in 0..wrong.len() {
        let hits = broken.iter().filter(|e| e.origin.line == 1 + k as u32).count();
        assert_eq!(hits, 1, "row {k} ({:?}): {broken:#?}", wrong[k].0);
    }
    assert_eq!(broken.len(), wrong.len(), "{broken:#?}");
}

/// The forms of msl-fixes (design C9 to C12, C6): `freq()` and `analysis(kind)` reads, a
/// `bound_step` statement, a `fired` read, an `unwrap` and `expect` of an `Option` pair, and the
/// tolerances of a `cross`.
#[test]
fn the_environment_bound_step_fired_and_unwrap_forms_print_and_check() {
    // ambient.msl, a comparator that bounds its step and reads the analysis:
    //
    //   module Cmp(p: Electrical, n: Electrical, in flag: Bit, out q: Bit = false,
    //              tt: f64 = 1p, et: f64 = 1u) {}                                      // 1
    //   analog { bound_step(2.0);                                                       // 2
    //            i(p, n) +> v(p, n) * if analysis(Ac) { freq() } else { 0.0 }; }       // 3
    //   digital for Cmp {
    //       on cross(v(p, n), tt, et), flag.changed() {                                  // 5
    //           q <- if fired(0) { flag.unwrap() } else { flag.expect("no flag") };      // 6
    //       }
    //   }
    const F: &str = "ambient.msl";
    let mut b = IrBuilder::new();
    let nodes = Design::ports(&mut b, F);
    let mut x = Dx::new(&mut b, F, 2, 20);
    let step = x.real(2.0);
    let bound = b.stmt(Stmt::BoundStep { site: 0, value: step, when: None });
    let mut x = Dx::new(&mut b, F, 3, 20);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let ac = x.b.expr(Expr::Env(Env::Analysis(AnalysisKind::Ac)), ValueType::Bool, x.origin());
    let freq = x.b.expr(Expr::Env(Env::Freq), ValueType::F64, x.origin());
    let zero = x.real(0.0);
    let level = x.if_(ac, freq, zero, ValueType::F64);
    let value = x.bin(piperine_ir::BinaryOp::Mul, v, level, ValueType::F64);
    let contribution = b.stmt(Stmt::Contribute {
        branch: 0,
        quantity: piperine_ir::QuantityRole::Flow,
        addends: vec![piperine_ir::Addend { operator: piperine_ir::Operator::Direct, negated: false, value }],
    });
    let body = b.body(BodySpec {
        nodes,
        params: vec![Buck::param("tt", F, 1, Some(1e-12), None), Buck::param("et", F, 1, Some(1e-6), None)],
        branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Flow }],
        analog: Some(vec![bound, contribution]),
        ..Buck::spec("Cmp", F, 1)
    });
    b.carrier(body, Buck::carrier("flag", ValueType::Bool, CarrierRole::In, None, F, 1, 40)).expect("a body this builder handed out");
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 52)).expect("a body this builder handed out");
    let mut x = Dx::new(&mut b, F, 5, 14);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let tt = x.param(0, ValueType::F64);
    let et = x.param(1, ValueType::F64);
    let cross = b.event(body, Event { source: EventSource::Cross { e: v, ttol: Some(tt), etol: Some(et) }, guard: None, origin: Design::origin(F, 5, 8) }).expect("a body this builder handed out");
    let flag = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Design::origin(F, 5, 45) }).expect("a body this builder handed out");
    let mut x = Dx::new(&mut b, F, 6, 17);
    let fired = x.b.expr(Expr::Fired(0), ValueType::Bool, x.origin());
    let c = x.carrier(0, ValueType::Bool);
    let off = x.boolean(false);
    let present = x.b.expr(Expr::Unwrap { is: c, value: off, message: None }, ValueType::Bool, x.origin());
    let c = x.carrier(0, ValueType::Bool);
    let off = x.boolean(false);
    let expected = x.b.expr(Expr::Unwrap { is: c, value: off, message: Some("no flag".into()) }, ValueType::Bool, x.origin());
    let pick = x.if_(fired, present, expected, ValueType::Bool);
    let commit = x.commit(1, pick, None);
    Buck::reaction(&mut b, body, vec![cross, flag], vec![commit], vec![], F, 5);
    let ir = Design::place(b, body, 2, &[ValueType::Bool, ValueType::Bool], vec![Constant::Real(Real::new(1e-12)), Constant::Real(Real::new(1e-6))]);
    assert_eq!(ir.check(), vec![]);

    // Written as the source reads: `freq()`, `analysis(Ac)`, `bound_step(..)`, `fired(E0)`,
    // `unwrap(..)` and `expect(.., "..")` of the pair, the tolerances after the watched value.
    Printed::holds(
        &ir,
        &["  analog", "    bound_step(2.0)", "    b0.flow +> direct (probe(p, n) * if analysis(Ac) { freq() } else { 0.0 })"],
    );
    Printed::holds(&ir, &["  event E0 cross probe(p, n) ttol tt etol et", "  event E1 changed flag"]);
    Printed::holds(
        &ir,
        &["  reaction R0 on (E0, E1)", "    q <- if fired(E0) { unwrap(flag, false) } else { expect(flag, false, \"no flag\") }"],
    );
    assert!(!ir.print_structural().contains(" : "), "a type note was written:\n{}", ir.print_structural());
    let full = ir.to_string();
    for line in ["bound_step B0 e", "env analysis Ac", "env freq", "fired 0", "unwrap e", "expect e", " ttol e"] {
        assert!(full.contains(line), "{line} missing from:\n{full}");
    }

    // The flow of the branch lists both reads in `Env` order: `freq()` before `analysis(..)`.
    let cmp = ir.top().body();
    assert_eq!(cmp.dependencies(Target::Flow(0)), Deps { nodes: vec![0, 1], env: vec![Env::Freq, Env::Analysis(AnalysisKind::Ac)], ..Deps::default() });

    // Each form written outside its place or against its type is one entry: `bound_step` in a
    // reaction, with a `bool` step; `fired` in the analog block and out of range; an `unwrap`
    // with an `f64` flag; the clock-typed reads of the new environment forms; a `cross`
    // tolerance that is a `bool`, one that reads a probe; `freq()` read by a drive.
    let mut b = IrBuilder::new();
    let nodes = Design::ports(&mut b, F);
    let mut x = Dx::new(&mut b, F, 2, 5);
    let yes = x.boolean(true);
    let bad_step = b.stmt(Stmt::BoundStep { site: 0, value: yes, when: None });
    let x = Dx::new(&mut b, F, 4, 5);
    let in_analog = x.b.expr(Expr::Fired(0), ValueType::Bool, x.origin());
    let fired_stmt = b.stmt(Stmt::Expr(in_analog));
    let x = Dx::new(&mut b, F, 5, 5);
    let ac_real = x.b.expr(Expr::Env(Env::Analysis(AnalysisKind::Dc)), ValueType::F64, x.origin());
    let freq_bool = x.b.expr(Expr::Env(Env::Freq), ValueType::Bool, x.origin());
    let wrong_reads = b.stmt(Stmt::Expr(ac_real));
    let wrong_freq = b.stmt(Stmt::Expr(freq_bool));
    let body = b.body(BodySpec {
        nodes,
        analog: Some(vec![bad_step, fired_stmt, wrong_reads, wrong_freq]),
        ..Buck::spec("Bad", F, 1)
    });
    b.carrier(body, Buck::carrier("q", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 20)).expect("a body this builder handed out");
    b.carrier(body, Buck::carrier("r", ValueType::Bool, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 30)).expect("a body this builder handed out");
    let mut x = Dx::new(&mut b, F, 6, 5);
    let late = x.real(1.0);
    let late_step_in_reaction = x.b.stmt(Stmt::BoundStep { site: 2, value: late, when: None });
    let out_of_range = x.b.expr(Expr::Fired(7), ValueType::Bool, x.origin());
    let flag_f64 = x.real(1.0);
    let value = x.boolean(true);
    let unwrapped = x.b.expr(Expr::Unwrap { is: flag_f64, value, message: None }, ValueType::F64, x.origin());
    let o = x.commit(0, out_of_range, None);
    let u = x.stmt(unwrapped);
    let reaction_stmts = vec![late_step_in_reaction, o, u];
    let mut x = Dx::new(&mut b, F, 7, 5);
    let v = x.probe(Terminal::Node(0), Terminal::Node(1));
    let flag_tol = x.boolean(true);
    let probe_tol = x.probe(Terminal::Node(0), Terminal::Node(1));
    let e0 = b.event(body, Event { source: EventSource::Cross { e: v, ttol: Some(flag_tol), etol: Some(probe_tol) }, guard: None, origin: Design::origin(F, 7, 8) }).expect("a body this builder handed out");
    Buck::reaction(&mut b, body, vec![e0], reaction_stmts, vec![], F, 6);
    let mut x = Dx::new(&mut b, F, 8, 5);
    let freq = x.b.expr(Expr::Env(Env::Freq), ValueType::F64, x.origin());
    let level = x.real(0.0);
    let over = x.bin(piperine_ir::BinaryOp::Gt, freq, level, ValueType::Bool);
    b.drive(body, Drive { carrier: 1, value: over, after: None, origin: Design::origin(F, 8, 3), element: None, path: vec![] }).expect("a body this builder handed out");
    let broken = Design::place(b, body, 2, &[ValueType::Bool, ValueType::Bool], vec![]).check();
    let one = |line: u32, what: &str| {
        let hits: Vec<_> = broken.iter().filter(|e| e.origin.line == line && e.what.contains(what)).collect();
        assert_eq!(hits.len(), 1, "line {line}: {what}: {broken:#?}");
    };
    one(2, "the step of a `bound_step` must be f64, found bool");
    one(4, "`fired` outside a reaction");
    one(5, "expression is typed f64, its form gives bool");
    one(5, "expression is typed bool, its form gives f64");
    one(6, "`bound_step` outside the top level of an analog block");
    one(6, "`fired` of event 7, out of range (1 events)");
    one(6, "the presence flag of an `unwrap` must be bool, found f64");
    one(6, "expression is typed f64, its form gives bool");
    one(7, "the `ttol` of a `cross` is an `f64`, found bool");
    one(7, "the `etol` of a `cross` reads an input");
    one(8, "reads freq()");
    assert_eq!(broken.len(), 11, "{broken:#?}");
}

#[test]
fn the_guard_of_an_event_reads_another_events_fired_never_its_own() {
    // §13 rule 9, design D8: the guard of an event reads whether another event of the body fired in
    // the microstep (`!fired(0)` here); it never reads its own, and no other code outside a
    // reaction reads one
    const F: &str = "guard.msl";
    for (own, expected) in [(false, vec![]), (true, vec!["the guard of event 1 reads its own `fired`"])] {
        let mut b = IrBuilder::new();
        let body = b.body(Buck::spec("Guarding", F, 1));
        b.carrier(body, Buck::carrier("d", ValueType::Bool, CarrierRole::In, None, F, 1, 14)).expect("a body this builder handed out");
        b.carrier(body, Buck::carrier("en", ValueType::Bool, CarrierRole::In, None, F, 1, 28)).expect("a body this builder handed out");
        let first = b.event(body, Event { source: EventSource::Changed(1), guard: None, origin: Design::origin(F, 2, 8) }).expect("a body this builder handed out");
        let mut x = Dx::new(&mut b, F, 3, 8);
        let read = x.b.expr(Expr::Fired(if own { 1 } else { first }), ValueType::Bool, x.origin());
        let guard = x.un(piperine_ir::UnaryOp::Not, read, ValueType::Bool);
        let second = b.event(body, Event { source: EventSource::Changed(0), guard: Some(guard), origin: Design::origin(F, 3, 8) }).expect("a body this builder handed out");
        let mut x = Dx::new(&mut b, F, 4, 8);
        let yes = x.boolean(true);
        let commit = x.commit(0, yes, None);
        Buck::reaction(&mut b, body, vec![second], vec![commit], vec![], F, 4);
        let found: Vec<String> = Design::place(b, body, 0, &[ValueType::Bool, ValueType::Bool], vec![]).check().into_iter().map(|e| e.what).collect();
        assert_eq!(found.iter().filter(|w| w.contains("reads its own `fired`") || w.contains("`fired` outside")).cloned().collect::<Vec<_>>(), expected, "own: {own}: {found:?}");
    }
}
