//! The shapes of the forms the IR amendments of msl-digital add (design "Amendments behind the
//! IR", A1 to A7): the ambient reads `now()` and `analysis(Static)`, timers, record pins, the
//! flow of a potential branch, and the previous value of a carrier. Each test builds a small body
//! by hand and reads its shape entry by entry; every expected entry is written from abi.md (the
//! section cited at its assertion), and every shape passes `piperine_abi::validate`.

use piperine_abi::raw::{Cell, Shape, ambient, cell, part};
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
