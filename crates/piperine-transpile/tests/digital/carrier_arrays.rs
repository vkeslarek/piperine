//! Arrays of carriers compiled and run (composite-values CV-24, CV-25; abi.md §3): a hand-built `Ram` body whose
//! reactions commit an element at a run-time index, read a constant element and an element at a run-time index
//! through the `WHOLE` view, and wake on an element event, compiled by `rustc` and run on the engine. Every expected
//! instant and value is written from the rule cited at its assertion.
#![cfg(unix)]

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{BinaryOp, BodyId, Carrier, CarrierRole, CommitTarget, Constant, Event, EventSource, Expr, IrBuilder, Stmt, ValueType};

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;

use digital::{Buck, Dx, ValueAliases};
use mixed::BuiltDesign;
use piperine_ir_fixtures::mixed::TopDesign;

const F: &str = "ram.msl";
/// One nanosecond in ticks.
const NS: i64 = 1_000_000;
const BIT: ValueType = ValueType::Bool;

/// The bodies of this target, each beside the MSL it transcribes.
struct Fixture;

impl Fixture {
    /// ```text
    /// module Ram(in clk: Bit, out q: Bit<u8> = 0, out hit: Bit = false) {
    ///     signal mem: [Bit<u8>; 4] = 5;
    ///     signal k: Bit<u8> = 2;
    /// }
    /// digital for Ram {
    ///     on clk.rise() { mem[k] <- 9; mem[1] <- mem[0]; hit <- mem[k] == 9; }
    ///     on mem[2].changed() { q <- mem[2]; }
    /// }
    /// ```
    fn ram(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Ram", F, 1));
        let byte = ValueAliases::u(8);
        b.carrier(body, Buck::carrier("clk", BIT, CarrierRole::In, None, F, 1, 20));
        b.carrier(body, Carrier { count: Some(4), ..Buck::carrier("mem", byte.clone(), CarrierRole::Signal, Some(Constant::Int(5)), F, 2, 5) });
        b.carrier(body, Buck::carrier("k", byte.clone(), CarrierRole::Signal, Some(Constant::Int(2)), F, 3, 5));
        b.carrier(body, Buck::carrier("q", byte.clone(), CarrierRole::Out, Some(Constant::Int(0)), F, 1, 34));
        b.carrier(body, Buck::carrier("hit", BIT, CarrierRole::Out, Some(Constant::Bool(false)), F, 1, 52));
        let (clk, mem, k, q, hit) = (0, 1, 2, 3, 4);
        let rise = Buck::rise(b, body, clk, F, 5, 8);
        let mut x = Dx::new(b, F, 5, 20);
        // mem[k] <- 9
        let at = x.carrier(k, byte.clone());
        let nine = x.int(9, byte.clone());
        let target = CommitTarget { element: Some(at), path: vec![] };
        let first = x.b.stmt(Stmt::Commit { carrier: mem, value: nine, after: None, target: Some(target) });
        // mem[1] <- mem[0]
        x.at(5, 34);
        let zero = x.int(0, byte.clone());
        let source = x.b.expr(Expr::CarrierElement { carrier: mem, index: zero }, byte.clone(), x.origin());
        let one = x.int(1, byte.clone());
        let second = x.b.stmt(Stmt::Commit { carrier: mem, value: source, after: None, target: Some(CommitTarget { element: Some(one), path: vec![] }) });
        // hit <- mem[k] == 9
        x.at(5, 50);
        let at = x.carrier(k, byte.clone());
        let read = x.b.expr(Expr::CarrierElement { carrier: mem, index: at }, byte.clone(), x.origin());
        let nine = x.int(9, byte.clone());
        let same = x.bin(BinaryOp::Eq, read, nine, BIT);
        let third = x.commit(hit, same, None);
        Buck::reaction(b, body, vec![rise], vec![first, second, third], vec![], F, 5);
        // on mem[2].changed() { q <- mem[2]; }
        let changed = b.event(body, Event { source: EventSource::ElementChanged { carrier: mem, element: 2 }, guard: None, origin: Buck::origin(F, 6, 8) });
        let mut x = Dx::new(b, F, 6, 30);
        let two = x.int(2, byte.clone());
        let value = x.b.expr(Expr::CarrierElement { carrier: mem, index: two }, byte.clone(), x.origin());
        let copy = x.commit(q, value, None);
        Buck::reaction(b, body, vec![changed], vec![copy], vec![], F, 6);
        body
    }
}

#[test]
fn element_reads_and_commits_compile_and_run() {
    let mut w = TopDesign::new();
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![Constant::Real(piperine_ir::Real::new(200e6))], &[("clk", BIT)]);
    let ram = Fixture::ram(&mut w.b);
    w.place("ram", ram, vec![], &[("clk", BIT), ("mem", ValueAliases::u(8)), ("k", ValueAliases::u(8)), ("q", ValueAliases::u(8)), ("hit", BIT)]);
    let seen = BuiltDesign::new(w.finish()).run(10e-9, 1e-9);
    // §12.1.1 and CV-25: at the first rising edge (2.5 ns) `mem[k] <- 9` commits element 2, the commit lands at the
    // next microstep of the same instant, and the element event `mem[2].changed()` then fires the reaction that
    // copies `mem[2]` (a constant element read, 9) to `q`
    assert_eq!(seen.transient("q"), vec![(5 * NS / 2, vec![9])], "q follows element 2");
    // R2: the read `mem[k]` of the first edge sees the snapshot before the commit (5), so `hit` stays false; at the
    // second edge (7.5 ns) element 2 holds 9 and `hit` rises
    assert_eq!(seen.transient("hit"), vec![(15 * NS / 2, HIGH.to_vec())], "hit rises when the element is 9 at the next edge");
    assert_eq!(seen.series("hit").first().map(|(_, b)| b.clone()), Some(LOW.to_vec()), "and starts low");
}
