//! The generated code of the forms the IR amendments of msl-digital add (design "Amendments
//! behind the IR"; spec DIG-09, DIG-10, DIG-11, DIG-12, DIG-13), on hand-built IR compiled once
//! and run on the solver: `now()` and `analysis(Static)`, hoisted names, timers, records, the
//! flow of a potential branch and the previous value of a carrier. Every expected instant and
//! value is written from the rule cited at its assertion (`docs/spec/`, abi.md), never read
//! back from the printed source.
#![cfg(unix)]

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{
    Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Constant, Drive, EmitField, Env, Event, EventSource, Expr,
    ExprId, IrBuilder, Local, NodeRole, Operator, QuantityRole, Real, Stmt, Stream, StreamField, Terminal, ValueType,
};
use piperine_solver::prelude::NodeIdentifier;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};
use piperine_transpile::{Error, Transpiler};

use crate::pin_bytes::PinBytes;
use piperine_ir_fixtures::digital;
use piperine_parity::mixed;

use digital::{Buck, Dx};
use mixed::BuiltDesign;
use piperine_ir_fixtures::mixed::TopDesign;

const F: &str = "amended.msl";
/// One nanosecond in ticks of one femtosecond, the solver's default resolution.
const NS: i64 = 1_000_000;
/// Seconds per tick at that resolution.
const TICK: f64 = 1e-15;
const BIT: ValueType = ValueType::Bool;
const REAL: ValueType = ValueType::F64;

/// The oscillator the amended tests run under.
struct TopClock;

impl TopClock {
    /// The 200 MHz oscillator on the net `Top.clk`: it toggles every 2.5 ns from 2.5 ns.
    fn clock(w: &mut TopDesign) {
        let osc = Buck::osc(&mut w.b);
        w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
    }
}

/// A node of the design, as the solver names it.
struct DesignNode;

impl DesignNode {
    /// The solver node of the design node `Top.<name>`.
    fn node(ir: &piperine_ir::Ir, name: &str) -> NodeIdentifier {
        let path = format!("Top.{name}");
        NodeIdentifier::Anonymous(ir.node_at(&path).unwrap_or_else(|| panic!("no node {path}")).id().index())
    }
}

/// The bodies the amended tests compile.
struct AmendedBodies;

impl AmendedBodies {
    /// An environment read at the origin of `x`.
    fn env(x: &mut Dx<'_>, env: Env) -> ExprId {
        let o = x.origin();
        let ty = if env == Env::Static { BIT } else { REAL };
        x.b.expr(Expr::Env(env), ty, o)
    }

    /// `module <name>(p) { analog { v(p) +> <value>; } }`: a potential source to ground whose value
    /// `value` builds.
    fn source(b: &mut IrBuilder, name: &str, value: impl FnOnce(&mut Dx<'_>) -> ExprId) -> BodyId {
        let el = Buck::electrical(b);
        let mut x = Dx::new(b, F, 2, 20);
        let v = value(&mut x);
        let contribution = b.stmt(Stmt::Contribute {
            branch: 0,
            quantity: QuantityRole::Potential,
            addends: vec![Addend { operator: Operator::Direct, negated: false, value: v }],
        });
        b.body(BodySpec {
            nodes: vec![BodyNode { name: "p".into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, 10) }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Potential }],
            analog: Some(vec![contribution]),
            ..Buck::spec(name, F, 1)
        })
    }

    /// `module <name>(in clk: Bit, out <out>: <ty>)` with `on clk.changed() { <out> <- <value>; }`.
    fn on_clock(b: &mut IrBuilder, name: &str, out: &str, ty: ValueType, value: impl FnOnce(&mut Dx<'_>) -> ExprId) -> BodyId {
        let body = b.body(Buck::spec(name, F, 1));
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
        b.carrier(body, Buck::carrier(out, ty, CarrierRole::Out, None, F, 1, 35));
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 3, 8) });
        let mut x = Dx::new(b, F, 3, 30);
        let v = value(&mut x);
        let commit = x.commit(1, v, None);
        Buck::reaction(b, body, vec![changed], vec![commit], vec![], F, 3);
        body
    }

    /// `module <name>(in clk: Bit, out q: Bit)` with the carriers a hoisted `transition` and a
    /// hoisted `above` would give it (contract.md D9) and a `match` local:
    ///
    /// ```text
    /// signal gate.x: Bit;  signal above$0.init$0.up: Bit;
    /// gate.x = clk;
    /// on gate.x.changed() { above$0.init$0.up <- gate.x; }
    /// on above$0.init$0.up.changed() { let match$0 = above$0.init$0.up; q <- match$0; }
    /// ```
    fn hoisted(b: &mut IrBuilder, name: &str, extra: Option<&str>) -> BodyId {
        let body = b.body(Buck::spec(name, F, 1));
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
        b.carrier(body, Buck::carrier("gate.x", BIT, CarrierRole::Signal, None, F, 2, 5));
        b.carrier(body, Buck::carrier("above$0.init$0.up", BIT, CarrierRole::Signal, None, F, 3, 5));
        b.carrier(body, Buck::carrier("q", BIT, CarrierRole::Out, None, F, 1, 35));
        if let Some(extra) = extra {
            b.carrier(body, Buck::carrier(extra, BIT, CarrierRole::Signal, None, F, 4, 5));
        }
        let mut x = Dx::new(b, F, 5, 14);
        let clk = x.carrier(0, BIT);
        b.drive(body, Drive { carrier: 1, value: clk, after: None, origin: Buck::origin(F, 5, 5), element: None, path: vec![] });
        let gate = b.event(body, Event { source: EventSource::Changed(1), guard: None, origin: Buck::origin(F, 6, 8) });
        let mut x = Dx::new(b, F, 6, 40);
        let level = x.carrier(1, BIT);
        let commit = x.commit(2, level, None);
        Buck::reaction(b, body, vec![gate], vec![commit], vec![], F, 6);
        let up = b.event(body, Event { source: EventSource::Changed(2), guard: None, origin: Buck::origin(F, 7, 8) });
        let mut x = Dx::new(b, F, 7, 50);
        let level = x.carrier(2, BIT);
        let bind = x.let_(0, level);
        let local = x.local(0, BIT);
        let commit = x.commit(3, local, None);
        let locals = vec![Local { name: "match$0".into(), ty: BIT, mutable: false }];
        Buck::reaction(b, body, vec![up], vec![bind, commit], locals, F, 7);
        body
    }

    /// `module Timer(out count: Bit<u8> = 0)` with two timed signals and, when `rearm`, a second
    /// timer that moves the first:
    ///
    /// ```text
    /// signal t0: Bit<f64> = 10n;  signal dt: Bit<f64> = 5n;
    /// on at(t0 + dt) { count <- count + 1; }
    /// on at(12n) { dt <- 20n; }                 // `rearm` only
    /// ```
    fn timer(b: &mut IrBuilder, rearm: bool) -> BodyId {
        let u8 = ValueType::UInt(8);
        let body = b.body(Buck::spec("Timer", F, 1));
        b.carrier(body, Buck::carrier("t0", REAL, CarrierRole::Signal, Some(Constant::Real(Real::new(10e-9))), F, 2, 5));
        b.carrier(body, Buck::carrier("dt", REAL, CarrierRole::Signal, Some(Constant::Real(Real::new(5e-9))), F, 2, 30));
        b.carrier(body, Buck::carrier("count", u8.clone(), CarrierRole::Out, Some(Constant::Int(0)), F, 1, 14));
        let mut x = Dx::new(b, F, 3, 11);
        let (t0, dt) = (x.carrier(0, REAL), x.carrier(1, REAL));
        let at = x.bin(piperine_ir::BinaryOp::Add, t0, dt, REAL);
        let at = b.event(body, Event { source: EventSource::At(at), guard: None, origin: Buck::origin(F, 3, 8) });
        let mut x = Dx::new(b, F, 3, 30);
        let count = x.carrier(2, u8.clone());
        let one = x.int(1, u8.clone());
        let next = x.bin(piperine_ir::BinaryOp::Add, count, one, u8);
        let commit = x.commit(2, next, None);
        Buck::reaction(b, body, vec![at], vec![commit], vec![], F, 3);
        if rearm {
            let twelve = Dx::new(b, F, 4, 11).real(12e-9);
            let at = b.event(body, Event { source: EventSource::At(twelve), guard: None, origin: Buck::origin(F, 4, 8) });
            let mut x = Dx::new(b, F, 4, 25);
            let twenty = x.real(20e-9);
            let commit = x.commit(1, twenty, None);
            Buck::reaction(b, body, vec![at], vec![commit], vec![], F, 4);
        }
        body
    }

    /// `module <name>(in clk: Bit, out hit: Bit)`:
    ///
    /// ```text
    /// signal t: Bit<f64> = -1.0;               // a time before every instant: armed for nothing
    /// on clk.rise() { t <- <time>; }
    /// on at(t) { hit <- true; }
    /// ```
    fn late(b: &mut IrBuilder, name: &str, time: Time) -> BodyId {
        let body = b.body(Buck::spec(name, F, 1));
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
        b.carrier(body, Buck::carrier("t", REAL, CarrierRole::Signal, Some(Constant::Real(Real::new(-1.0))), F, 2, 5));
        b.carrier(body, Buck::carrier("hit", BIT, CarrierRole::Out, None, F, 1, 35));
        let value = match time {
            Time::Fixed(v) => Dx::new(b, F, 3, 25).real(v),
            Time::Now => AmendedBodies::env(&mut Dx::new(b, F, 3, 25), Env::Time),
        };
        let rise = Buck::rise(b, body, 0, F, 3, 8);
        let commit = Dx::new(b, F, 3, 20).commit(1, value, None);
        Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 3);
        let t = Dx::new(b, F, 4, 11).carrier(1, REAL);
        let at = b.event(body, Event { source: EventSource::At(t), guard: None, origin: Buck::origin(F, 4, 8) });
        let mut x = Dx::new(b, F, 4, 20);
        let yes = x.boolean(true);
        let commit = x.commit(2, yes, None);
        Buck::reaction(b, body, vec![at], vec![commit], vec![], F, 4);
        body
    }

    /// `module Probe(in clk: Bit, out n: Bit<u8> = 0)`, with a stream `trace { code: u8, level: f64 }`
    /// when `emits`:
    ///
    /// ```text
    /// on clk.rise() { n <- n + 1; emit!("trace", code = 7u8, note = "edge"); }    // the emit when `emits`
    /// ```
    fn probe(b: &mut IrBuilder, emits: bool) -> BodyId {
        let u8 = ValueType::UInt(8);
        let body = b.body(Buck::spec("Probe", F, 1));
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 14));
        b.carrier(body, Buck::carrier("n", u8.clone(), CarrierRole::Out, Some(Constant::Int(0)), F, 1, 26));
        let rise = Buck::rise(b, body, 0, F, 2, 8);
        let mut x = Dx::new(b, F, 2, 22);
        let n = x.carrier(1, u8.clone());
        let one = x.int(1, u8.clone());
        let next = x.bin(piperine_ir::BinaryOp::Add, n, one, u8.clone());
        let mut stmts = vec![x.commit(1, next, None)];
        if emits {
            let fields = vec![StreamField { name: "code".into(), ty: u8.clone() }, StreamField { name: "level".into(), ty: REAL }];
            let trace = b.stream(body, Stream { name: "trace".into(), fields });
            let mut x = Dx::new(b, F, 2, 40);
            let code = x.int(7, u8);
            let note = EmitField::Text { name: "note".into(), text: "edge".into() };
            stmts.push(x.b.stmt(Stmt::Emit { stream: trace, fields: vec![EmitField::Value { field: 0, value: code }, note] }));
        }
        Buck::reaction(b, body, vec![rise], stmts, vec![], F, 2);
        body
    }

    /// `module <name>(p, n) { <param>: f64 = <default>; analog { v(p, n) +> <value>; } }`, the value
    /// built over the param, the probe `v(p, n)` and the flow `i(p, n)` by `value`.
    fn potential(b: &mut IrBuilder, name: &str, param: &str, default: f64, value: impl FnOnce(&mut Dx<'_>, ExprId, ExprId) -> Addend) -> BodyId {
        let el = Buck::electrical(b);
        let mut x = Dx::new(b, F, 3, 20);
        let k = x.param(0, REAL);
        let o = x.origin();
        let flow = x.b.expr(Expr::Flow { branch: 0 }, REAL, o);
        let addend = value(&mut x, k, flow);
        let contribution = b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Potential, addends: vec![addend] });
        let port = |n: &str, col| BodyNode { name: n.into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 1, col) };
        b.body(BodySpec {
            params: vec![Buck::param(param, F, 2, Some(default), None)],
            nodes: vec![port("p", 10), port("n", 18)],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Node(1), kind: BranchKind::Potential }],
            analog: Some(vec![contribution]),
            ..Buck::spec(name, F, 1)
        })
    }

    /// `module Seq(out x: Bit<u8>, out p: Bit<u8> = 9, out g: Bit = false)`:
    ///
    /// ```text
    /// on at(1n) { x <- 3; }
    /// on at(2n) { x <- 5; }
    /// on x.changed() { p <- x.prev(); }
    /// on x.changed() if x.prev() > 2 { g <- true; }
    /// ```
    fn sequence(b: &mut IrBuilder) -> BodyId {
        let u8 = ValueType::UInt(8);
        let body = b.body(Buck::spec("Seq", F, 1));
        b.carrier(body, Buck::carrier("x", u8.clone(), CarrierRole::Out, None, F, 1, 12));
        b.carrier(body, Buck::carrier("p", u8.clone(), CarrierRole::Out, Some(Constant::Int(9)), F, 1, 28));
        b.carrier(body, Buck::carrier("g", BIT, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 48));
        for (line, at, value) in [(2, 1e-9, 3), (3, 2e-9, 5)] {
            let time = Dx::new(b, F, line, 11).real(at);
            let event = b.event(body, Event { source: EventSource::At(time), guard: None, origin: Buck::origin(F, line, 8) });
            let mut x = Dx::new(b, F, line, 20);
            let v = x.int(value, u8.clone());
            let commit = x.commit(0, v, None);
            Buck::reaction(b, body, vec![event], vec![commit], vec![], F, line);
        }
        let previous = |b: &mut IrBuilder, line, col| {
            let x = Dx::new(b, F, line, col);
            let o = x.origin();
            x.b.expr(Expr::Previous(0), u8.clone(), o)
        };
        let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(F, 4, 8) });
        let prev = previous(b, 4, 30);
        let commit = Dx::new(b, F, 4, 25).commit(1, prev, None);
        Buck::reaction(b, body, vec![changed], vec![commit], vec![], F, 4);
        let prev = previous(b, 5, 26);
        let mut x = Dx::new(b, F, 5, 26);
        let two = x.int(2, u8.clone());
        let above = x.bin(piperine_ir::BinaryOp::Gt, prev, two, ValueType::Bool);
        let guarded = b.event(body, Event { source: EventSource::Changed(0), guard: Some(above), origin: Buck::origin(F, 5, 8) });
        let mut x = Dx::new(b, F, 5, 50);
        let yes = x.boolean(true);
        let commit = x.commit(2, yes, None);
        Buck::reaction(b, body, vec![guarded], vec![commit], vec![], F, 5);
        body
    }
}

/// Designs run under the oscillator, read back.
struct ClockedRuns;

impl ClockedRuns {
    /// The transient changes of `count` from a 40 ns run of the timer.
    fn counted(rearm: bool) -> Vec<(i64, Vec<u8>)> {
        let mut w = TopDesign::new();
        let body = AmendedBodies::timer(&mut w.b, rearm);
        w.place("timer", body, vec![], &[("timer.t0", REAL), ("timer.dt", REAL), ("count", ValueType::UInt(8))]);
        BuiltDesign::new(w.finish()).run(40e-9, 1e-9).transient("count")
    }

    /// The probe under the 200 MHz clock, compiled.
    fn probed(emits: bool) -> BuiltDesign {
        let mut w = TopDesign::new();
        TopClock::clock(&mut w);
        let body = AmendedBodies::probe(&mut w.b, emits);
        w.place("probe", body, vec![], &[("clk", BIT), ("n", ValueType::UInt(8))]);
        BuiltDesign::new(w.finish())
    }

    /// The microsteps of each instant the digital side runs through `stop` ticks, from the start of
    /// an analysis, as `(tick, microsteps)`.
    fn microsteps(built: &BuiltDesign, stop: i64) -> Vec<(i64, u32)> {
        use piperine_digital_solver::engine::{Horizon, Mode};
        use piperine_digital_solver::tick::Tick;
        let mut circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let digital = &mut circuit.bound_mut().digital;
        digital.start().unwrap_or_else(|e| panic!("{e:?}"));
        let mut ran = Vec::new();
        while let Some(at) = digital.next_due().filter(|t| t.0 <= stop) {
            let out = digital.run_instant(at, &[], Horizon::uniform(Tick(stop)), Mode::default()).unwrap_or_else(|e| panic!("{e:?}"));
            ran.push((at.0, out.microsteps));
        }
        ran
    }
}

#[test]
fn now_is_the_time_of_the_instant_and_of_the_solve() {
    let mut w = TopDesign::new();
    TopClock::clock(&mut w);
    let stamp = AmendedBodies::on_clock(&mut w.b, "Stamp", "t", REAL, |x| AmendedBodies::env(x, Env::Time));
    w.place("stamp", stamp, vec![], &[("clk", BIT), ("t", REAL)]);
    let ramp = AmendedBodies::source(&mut w.b, "Ramp", |x| AmendedBodies::env(x, Env::Time));
    w.place_analog("ramp", ramp, &["x"], &[]);
    let built = BuiltDesign::new(w.finish());
    let (seen, result) = built.run_with(20e-9, 1e-9);

    // part IV §12.1.12: `now()` in a reaction is the time of its instant. abi.md §4.2: `TIME` is
    // the time of the instant, which is a tick of the session's resolution (one femtosecond by
    // default), so the reaction fired by the clock's change at 15 ns commits 15e-9 as that tick
    // in seconds; the commit applies at the next microstep of the same instant
    let fifteen = seen.transient("t").into_iter().find(|(at, _)| *at == 15 * NS);
    assert_eq!(fifteen, Some((15 * NS, PinBytes::real_pin_bytes((15 * NS) as f64 * TICK))), "t at 15 ns: {:?}", seen.transient("t"));

    // abi.md §4.2: in `analog`, `TIME` is the time of the solve: `v(x) +> now()` holds the node
    // at the time of each accepted point
    let x = DesignNode::node(&built.ir, "x");
    let points: Vec<(f64, f64)> =
        result.iter().map(|s| (s.time(), s.get_node(&x).expect("v(x) is solved"))).filter(|(t, _)| *t > 0.0).collect();
    assert!(points.len() >= 3, "three accepted times: {points:?}");
    for (t, v) in [points[0], points[points.len() / 2], points[points.len() - 1]] {
        assert_eq!(v, t, "v(x) is the time of the solve at t = {t:e}");
    }
}

#[test]
fn static_is_true_only_in_the_equilibrium() {
    let mut w = TopDesign::new();
    TopClock::clock(&mut w);
    let mode = AmendedBodies::on_clock(&mut w.b, "Mode", "s", BIT, |x| AmendedBodies::env(x, Env::Static));
    w.place("mode", mode, vec![], &[("clk", BIT), ("s", BIT)]);
    let level = AmendedBodies::source(&mut w.b, "Level", |x| {
        let stat = AmendedBodies::env(x, Env::Static);
        let (one, zero) = (x.real(1.0), x.real(0.0));
        x.if_(stat, one, zero, REAL)
    });
    w.place_analog("level", level, &["x"], &[]);
    let built = BuiltDesign::new(w.finish());
    let (seen, result) = built.run_with(6e-9, 1e-9);

    // abi.md §4.2: `STATIC` is true in an equilibrium solve and in its instants. The clock's
    // initial value is a change at (0, 0) of the operating point, so the reaction commits true
    // there; its first change in the transient (2.5 ns) commits false
    let at_rest: Vec<Vec<u8>> = seen
        .all()
        .into_iter()
        .filter(|o| o.path == "Top.s" && o.analysis == AnalysisKind::OperatingPoint)
        .filter_map(|o| match o.kind {
            ObservationKind::Change { bytes } => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(at_rest, [HIGH.to_vec()], "s at the operating point");
    assert_eq!(seen.transient("s").first(), Some(&(5 * NS / 2, LOW.to_vec())), "s at the first transient instant");

    // in `analog`: the operating point solves `v(x) = 1`, every transient point `v(x) = 0`
    let x = DesignNode::node(&built.ir, "x");
    let points: Vec<(f64, f64)> = result.iter().map(|s| (s.time(), s.get_node(&x).expect("v(x) is solved"))).collect();
    assert_eq!(points.first(), Some(&(0.0, 1.0)), "the operating point: {points:?}");
    assert!(points.len() > 1 && points[1..].iter().all(|(_, v)| *v == 0.0), "every transient point: {points:?}");
}

#[test]
fn hoisted_names_print_as_rust_identifiers() {
    let mut w = TopDesign::new();
    TopClock::clock(&mut w);
    let body = AmendedBodies::hoisted(&mut w.b, "Hoisted", None);
    // every carrier is bound to a net, a signal to the one its instance declares
    w.place("h", body, vec![], &[("clk", BIT), ("h.gate.x", BIT), ("h.above$0.init$0.up", BIT), ("q", BIT)]);
    // `BuiltDesign::new` asserts the compiler's standard error is empty: no warning on `gate_dx`,
    // `above_s0_dinit_s0_dup` or `match_s0`
    let built = BuiltDesign::new(w.finish());
    let seen = built.run(4e-9, 1e-9);
    // q follows clk through the two hoisted carriers: high from the clock's first rise (2.5 ns)
    assert_eq!(seen.transient("q"), [(5 * NS / 2, HIGH.to_vec())]);
    // the ABI pins keep the IR names: the solver names each private pin `<instance>.<pin>`
    for pin in ["Top.h.gate.x", "Top.h.above$0.init$0.up"] {
        assert!(seen.series_of(pin).contains(&(5 * NS / 2, HIGH.to_vec())), "{pin}: {:?}", seen.series_of(pin));
    }
    assert!(seen.series_of("Top.h.gate_dx").is_empty(), "no pin is named by its Rust spelling");

    // a name that spells like a hoisted one is refused, not shadowed
    let mut w = TopDesign::new();
    let body = AmendedBodies::hoisted(&mut w.b, "Clash", Some("gate_dx"));
    w.place("h", body, vec![], &[("clk", BIT), ("h.gate.x", BIT), ("h.above$0.init$0.up", BIT), ("q", BIT), ("h.gate_dx", BIT)]);
    let transpiler = Transpiler::new(std::sync::Arc::new(w.finish())).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        transpiler.source().err(),
        Some(Error::Unsupported {
            form: "two names that print as `gate_dx`".into(),
            path: "body Clash gate_dx".into(),
            origin: Buck::origin(F, 4, 5),
        })
    );
}

#[test]
fn at_fires_once_at_its_time() {
    // part IV §13 `at(t)`: the event fires once, at `t`. Armed at the start with t0 = 10 ns and
    // dt = 5 ns, it fires at 15 ns and the counter, which adds one per firing, reads 1 from then
    // on (`Bit<u8>`: its value byte)
    assert_eq!(ClockedRuns::counted(false), [(15 * NS, vec![1])]);
}

#[test]
fn a_rearmed_at_drops_the_earlier_firing() {
    // part IV §13: `at(t)` re-arms when `t` changes; abi.md C3: the arming statement is `REPLACE`,
    // so the firing armed for 15 ns is removed when `dt <- 20n` at 12 ns re-arms the timer for
    // t0 + dt = 30 ns: no firing at 15 ns, one at 30 ns
    assert_eq!(ClockedRuns::counted(true), [(30 * NS, vec![1])]);
}

/// What a timed signal `t` of `Late` holds when the clock first rises.
#[derive(Clone, Copy)]
enum Time {
    /// `t <- <seconds>;`
    Fixed(f64),
    /// `t <- now();`
    Now,
}

#[test]
fn a_past_time_does_not_fire() {
    // a 100 MHz clock: its first rise is at 5 ns, the next at 15 ns, after the run
    let mut w = TopDesign::new();
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![Constant::Real(Real::new(100e6))], &[("clk", BIT)]);
    for (name, module, time) in [
        ("past", "Past", Time::Fixed(0.0)),
        ("now", "Now", Time::Now),
        ("later", "Later", Time::Fixed(8e-9)),
    ] {
        let body = AmendedBodies::late(&mut w.b, module, time);
        let hit = format!("{name}_hit");
        w.place(name, body, vec![], &[("clk", BIT), (&format!("{name}.t"), REAL), (&hit, BIT)]);
    }
    let seen = BuiltDesign::new(w.finish()).run(12e-9, 1e-9);
    // part IV §13: a time before the instant that arms it (0 armed at 5 ns) never fires
    assert_eq!(seen.transient("past_hit"), []);
    // a time equal to the instant fires at its next microstep (abi.md C1: a delay of 0), so
    // within the same instant, 5 ns
    assert_eq!(seen.transient("now_hit"), [(5 * NS, HIGH.to_vec())]);
    // the control: the same arming at 5 ns for 8 ns fires at 8 ns
    assert_eq!(seen.transient("later_hit"), [(8 * NS, HIGH.to_vec())]);
}

#[test]
fn an_emit_commits_one_record_and_adds_no_microstep() {
    let emitting = ClockedRuns::probed(true);
    let seen = emitting.run(4e-9, 1e-9);
    // abi.md C5: the sink delivers each commit to a RECORD pin as a record of the stream
    // `<instance path>.<pin name>`, stamped with the instant it was made: the reaction runs once
    // in the transient, at the clock's rise (2.5 ns)
    let records: Vec<(i64, Vec<u8>)> = seen
        .all()
        .into_iter()
        .filter(|o| o.analysis == AnalysisKind::Transient)
        .filter_map(|o| match o.kind {
            ObservationKind::Record { bytes, .. } if o.path == "Top.probe.trace" => Some(((o.time * 1e15).round() as i64, bytes)),
            _ => None,
        })
        .collect();
    // the layout of `trace` (msl-digital A4): `code` a `BYTE` at 0, `level` an `F64` at 8, 16
    // bytes; the site writes `code = 7`, and `level`, which it leaves out, stays all zero (C5)
    let mut bytes = vec![0u8; 16];
    bytes[0] = 7;
    assert_eq!(records, [(5 * NS / 2, bytes)]);
    // the commit beside it applies as before: n counts the rise (the clock's initial `false` at the
    // operating point is no rise), 0 to 1
    assert_eq!(seen.transient("n"), [(5 * NS / 2, vec![1])]);

    // P7: a RECORD pin is QUIET, so its commit never keeps an instant's microstep loop alive:
    // every instant through the rise runs as many microsteps as without the emit
    let silent = ClockedRuns::probed(false);
    let (with, without) = (ClockedRuns::microsteps(&emitting, 5 * NS / 2), ClockedRuns::microsteps(&silent, 5 * NS / 2));
    assert_eq!(with.last().map(|(t, _)| *t), Some(5 * NS / 2), "the rise is run: {with:?}");
    assert_eq!(with, without);
}

#[test]
fn the_inductor_functions_pass_verify_partials() {
    use piperine_abi::caller::Handle;
    use piperine_abi::verify::{Verdict, verify_partials};
    use piperine_plan::Modules;

    let mut w = TopDesign::new();
    // the power stage's inductor: `v(p, n) +> ddt(l * i(p, n))`
    let inductor = AmendedBodies::potential(&mut w.b, "Inductor", "l", 10e-6, |x, l, flow| {
        let charge = x.bin(piperine_ir::BinaryOp::Mul, l, flow, REAL);
        Addend { operator: Operator::Ddt, negated: false, value: charge }
    });
    // a voltage-controlled voltage source: `v(p, n) +> g * v(p, gnd)`, no flow read
    let vcvs = AmendedBodies::potential(&mut w.b, "Vcvs", "g", 2.0, |x, g, _| {
        let v = x.probe(Terminal::Node(0), Terminal::Ground);
        let value = x.bin(piperine_ir::BinaryOp::Mul, g, v, REAL);
        Addend { operator: Operator::Direct, negated: false, value }
    });
    // the flow read in a direct addend: `v(p, n) +> r * i(p, n)`
    let resistor = AmendedBodies::potential(&mut w.b, "FlowResistor", "r", 50.0, |x, r, flow| {
        let value = x.bin(piperine_ir::BinaryOp::Mul, r, flow, REAL);
        Addend { operator: Operator::Direct, negated: false, value }
    });
    for (name, body, nodes, value) in [("l", inductor, ["a", "b"], 10e-6), ("e", vcvs, ["c", "gnd"], 2.0), ("r", resistor, ["d", "gnd"], 50.0)] {
        w.place_analog(name, body, &nodes, &[value]);
    }
    let built = BuiltDesign::new(w.finish());
    // every read of these functions is an `F64` cell (a node, the flow pin, the param): three
    // points, each read its own value, the flow and the param on either side of 0
    let points: [[f64; 4]; 3] = [[0.3, -0.1, 2e-3, 10e-6], [1.2, 0.4, -0.5, 3.0], [-2.0, 0.7, 1.5e-6, -0.25]];
    let mut checked = 0;
    for (name, body) in [("Inductor", inductor), ("Vcvs", vcvs), ("FlowResistor", resistor)] {
        let module = Modules::module(&built.compiled, body).unwrap_or_else(|| panic!("{name} has a module"));
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        for (k, f) in handle.shape().functions.iter().enumerate() {
            for point in &points {
                let input: Vec<Vec<u8>> = (0..f.reads.len()).map(|i| point[i % point.len()].to_le_bytes().to_vec()).collect();
                // abi.md §8 (F5): every declared partial agrees with its finite difference
                match verify_partials(&handle, k as u32, &input, 1e-6) {
                    Ok(Verdict::Exact) => {}
                    other => panic!("{name} {} at {point:?}: {other:?}", f.name.as_str()),
                }
            }
            checked += 1;
        }
        // the flow read is a differentiable read: the terms function of a body that reads it
        // declares a partial against the flow pin for every term that reads it
        let terms = handle.shape().functions.iter().find(|f| f.name.as_str() == "terms").expect("a terms function");
        let flow_pin = handle.shape().pins.iter().position(|p| p.name.as_str() == "b0.flow").expect("the flow pin") as u32;
        let slot = terms.reads.iter().position(|c| c.index == flow_pin).expect("terms reads the flow pin") as u32;
        let against: Vec<u32> = terms.partials.iter().filter(|p| p.read == slot).map(|p| p.write).collect();
        let expected: &[u32] = match name {
            // KCL (b0.flow) and the charge `l·f` (b0.charge)
            "Inductor" => &[0, 2],
            // KCL only
            "Vcvs" => &[0],
            // KCL and the row `v(p, n) - r·f`
            _ => &[0, 1],
        };
        assert_eq!(against, expected, "{name}: the terms written against the flow pin");
    }
    assert_eq!(checked, 3, "one terms function per body");
}

#[test]
fn a_previous_value_is_read_in_a_guard_and_in_a_reaction() {
    let mut w = TopDesign::new();
    let body = AmendedBodies::sequence(&mut w.b);
    let u8 = ValueType::UInt(8);
    w.place("seq", body, vec![], &[("x", u8.clone()), ("p", u8), ("g", BIT)]);
    let seen = BuiltDesign::new(w.finish()).run(4e-9, 1e-9);
    // x takes 3 at 1 ns and 5 at 2 ns
    assert_eq!(seen.transient("x"), [(NS, vec![3]), (2 * NS, vec![5])]);
    // part II §5.3.8: the first commit is a change; `prev()` is the value before the most recent
    // change, so the initial value at the first (the carrier held no other before it): the
    // reaction commits 0 to p at 1 ns, and 3 at 2 ns
    assert_eq!(seen.transient("p"), [(NS, vec![0]), (2 * NS, vec![3])]);
    // the guard reads the same previous value: 0 at 1 ns (no firing), 3 at 2 ns
    assert_eq!(seen.transient("g"), [(2 * NS, HIGH.to_vec())]);
}
