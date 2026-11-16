//! Scope: the IR amendments of msl-digital (design A1 to A7, contract D3 to D7): the time and
//! static reads, the `at` event, emit sites and streams, the flow of a potential branch, the
//! previous value of a carrier, a child param that reads the parent, and the carrier bindings
//! of a child. Each is an IR node that `Ir::check` types and the printer writes, with its
//! dependency set; the earlier prints stay byte-identical. Expected texts are written from the
//! MSL source each fixture transcribes.

#[path = "support/digital.rs"]
#[allow(dead_code)]
mod digital;

use digital::{Buck, Dx};
use piperine_ir::{
    BodyId, BodySpec, Branch, BranchKind, CarrierRole, Constant, Deps, Drive, Env, Event, EventSource, Expr, ExprId, FunctionSpec, InstanceSpec, Ir,
    IrBuilder, NodeRef, NodeSpec, Origin, Real, Stmt, Target, Terminal, ValueType, NetSpec,
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
