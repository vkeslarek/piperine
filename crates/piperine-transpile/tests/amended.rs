//! The generated code of the forms the IR amendments of msl-digital add (design "Amendments
//! behind the IR"; spec DIG-09, DIG-10, DIG-11, DIG-12, DIG-13), on hand-built IR compiled once
//! and run on the solver: `now()` and `analysis(Static)`, hoisted names, timers, records, the
//! flow of a potential branch and the previous value of a carrier. Every expected instant and
//! value is written from the rule cited at its assertion (`docs/spec/`, abi.md), never read
//! back from the printed source.
#![cfg(unix)]

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{
    Addend, BodyId, BodyNode, BodySpec, Branch, BranchKind, CarrierRole, Constant, Drive, Env, Event, EventSource, Expr, ExprId,
    IrBuilder, Local, NodeRole, Operator, QuantityRole, Real, Stmt, Terminal, ValueType,
};
use piperine_solver::prelude::NodeIdentifier;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};
use piperine_transpile::{Error, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/mixed.rs"]
mod mixed;

use digital::{Buck, Dx};
use mixed::{Built, Wire};

const F: &str = "amended.msl";
/// One nanosecond in ticks of one femtosecond, the solver's default resolution.
const NS: i64 = 1_000_000;
/// Seconds per tick at that resolution.
const TICK: f64 = 1e-15;
const BIT: ValueType = ValueType::Bool;
const REAL: ValueType = ValueType::F64;

/// The 200 MHz oscillator on the net `Top.clk`: it toggles every 2.5 ns from 2.5 ns.
fn clock(w: &mut Wire) {
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
}

/// The bytes of a known `f64` in its pin (abi.md P3, `REAL`: the value at 0, known at 8).
fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

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

/// The solver node of the design node `Top.<name>`.
fn node(ir: &piperine_ir::Ir, name: &str) -> NodeIdentifier {
    let path = format!("Top.{name}");
    NodeIdentifier::Anonymous(ir.node_at(&path).unwrap_or_else(|| panic!("no node {path}")).id().index())
}

#[test]
fn now_is_the_time_of_the_instant_and_of_the_solve() {
    let mut w = Wire::new();
    clock(&mut w);
    let stamp = on_clock(&mut w.b, "Stamp", "t", REAL, |x| env(x, Env::Time));
    w.place("stamp", stamp, vec![], &[("clk", BIT), ("t", REAL)]);
    let ramp = source(&mut w.b, "Ramp", |x| env(x, Env::Time));
    w.place_analog("ramp", ramp, &["x"], &[]);
    let built = Built::new(w.finish());
    let (seen, result) = built.run_with(20e-9, 1e-9);

    // part IV §12.1.12: `now()` in a reaction is the time of its instant. abi.md §4.2: `TIME` is
    // the time of the instant, which is a tick of the session's resolution (one femtosecond by
    // default), so the reaction fired by the clock's change at 15 ns commits 15e-9 as that tick
    // in seconds; the commit applies at the next microstep of the same instant
    let fifteen = seen.transient("t").into_iter().find(|(at, _)| *at == 15 * NS);
    assert_eq!(fifteen, Some((15 * NS, real_bytes((15 * NS) as f64 * TICK))), "t at 15 ns: {:?}", seen.transient("t"));

    // abi.md §4.2: in `analog`, `TIME` is the time of the solve: `v(x) +> now()` holds the node
    // at the time of each accepted point
    let x = node(&built.ir, "x");
    let points: Vec<(f64, f64)> =
        result.iter().map(|s| (s.time(), s.get_node(&x).expect("v(x) is solved"))).filter(|(t, _)| *t > 0.0).collect();
    assert!(points.len() >= 3, "three accepted times: {points:?}");
    for (t, v) in [points[0], points[points.len() / 2], points[points.len() - 1]] {
        assert_eq!(v, t, "v(x) is the time of the solve at t = {t:e}");
    }
}

#[test]
fn static_is_true_only_in_the_equilibrium() {
    let mut w = Wire::new();
    clock(&mut w);
    let mode = on_clock(&mut w.b, "Mode", "s", BIT, |x| env(x, Env::Static));
    w.place("mode", mode, vec![], &[("clk", BIT), ("s", BIT)]);
    let level = source(&mut w.b, "Level", |x| {
        let stat = env(x, Env::Static);
        let (one, zero) = (x.real(1.0), x.real(0.0));
        x.if_(stat, one, zero, REAL)
    });
    w.place_analog("level", level, &["x"], &[]);
    let built = Built::new(w.finish());
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
    let x = node(&built.ir, "x");
    let points: Vec<(f64, f64)> = result.iter().map(|s| (s.time(), s.get_node(&x).expect("v(x) is solved"))).collect();
    assert_eq!(points.first(), Some(&(0.0, 1.0)), "the operating point: {points:?}");
    assert!(points.len() > 1 && points[1..].iter().all(|(_, v)| *v == 0.0), "every transient point: {points:?}");
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
    b.drive(body, Drive { carrier: 1, value: clk, after: None, origin: Buck::origin(F, 5, 5) });
    let gate = b.event(body, Event { source: EventSource::Changed(1), guard: None, origin: Buck::origin(F, 6, 8) });
    let mut x = Dx::new(b, F, 6, 40);
    let level = x.carrier(1, BIT);
    let commit = x.commit(2, level, None);
    Buck::reaction(b, body, vec![gate], vec![commit], vec![], F, 6);
    let up = b.event(body, Event { source: EventSource::Changed(2), guard: None, origin: Buck::origin(F, 7, 8) });
    let mut x = Dx::new(b, F, 7, 50);
    let level = x.carrier(2, BIT);
    let bind = x.let_(0, level);
    let local = x.local(0, digital::Ty::flagged(BIT));
    let commit = x.commit(3, local, None);
    let locals = vec![Local { name: "match$0".into(), ty: digital::Ty::flagged(BIT) }];
    Buck::reaction(b, body, vec![up], vec![bind, commit], locals, F, 7);
    body
}

#[test]
fn hoisted_names_print_as_rust_identifiers() {
    let mut w = Wire::new();
    clock(&mut w);
    let body = hoisted(&mut w.b, "Hoisted", None);
    // every carrier is bound to a net, a signal to the one its instance declares
    w.place("h", body, vec![], &[("clk", BIT), ("h.gate.x", BIT), ("h.above$0.init$0.up", BIT), ("q", BIT)]);
    // `Built::new` asserts the compiler's standard error is empty: no warning on `gate_dx`,
    // `above_s0_dinit_s0_dup` or `match_s0`
    let built = Built::new(w.finish());
    let seen = built.run(4e-9, 1e-9);
    // q follows clk through the two hoisted carriers: high from the clock's first rise (2.5 ns)
    assert_eq!(seen.transient("q"), [(5 * NS / 2, HIGH.to_vec())]);
    // the ABI pins keep the IR names: the solver names each private pin `<instance>.<pin>`
    for pin in ["Top.h.gate.x", "Top.h.above$0.init$0.up"] {
        assert!(seen.series_of(pin).contains(&(5 * NS / 2, HIGH.to_vec())), "{pin}: {:?}", seen.series_of(pin));
    }
    assert!(seen.series_of("Top.h.gate_dx").is_empty(), "no pin is named by its Rust spelling");

    // a name that spells like a hoisted one is refused, not shadowed
    let mut w = Wire::new();
    let body = hoisted(&mut w.b, "Clash", Some("gate_dx"));
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
