//! Commit semantics of compiled reactions on the solver (feature `ir-rust`, RS-13 AC 1 to 5;
//! checks C86 to C89): four small designs, each the example of its rule in
//! `docs/spec/part_iv_behavior.md` §12.1, driven by the compiled 200 MHz oscillator (rising
//! edges at 2.5 ns, 7.5 ns, 12.5 ns, ...), and read from the changes the solver observed on
//! each net, in ticks of one femtosecond. Every expected instant and value is written from the
//! rule cited at its assertion, never read back from the model.
#![cfg(unix)]

use piperine_abi::layout::{HIGH, LOW};
use piperine_ir::{BinaryOp, BodyId, CarrierRole, Constant, IrBuilder, Real, ValueType};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/mixed.rs"]
mod mixed;

use digital::{Buck, Dx, Ty};
use mixed::{Built, Wire};

const F: &str = "commit.msl";
/// One nanosecond in ticks.
const NS: i64 = 1_000_000;
const BIT: ValueType = ValueType::Bool;

fn clock(w: &mut Wire) {
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![Constant::Real(Real::new(200e6))], &[("clk", BIT)]);
}

fn input(b: &mut IrBuilder, body: BodyId, name: &str, ty: ValueType, col: u32) {
    b.carrier(body, Buck::carrier(name, ty, CarrierRole::In, None, F, 1, col));
}

fn output(b: &mut IrBuilder, body: BodyId, name: &str, ty: ValueType, init: Constant, col: u32) {
    b.carrier(body, Buck::carrier(name, ty, CarrierRole::Out, Some(init), F, 1, col));
}

/// `module Swap(in clk: Bit, out a: Bit = true, out b: Bit = false)`:
///
/// ```text
/// on clk.rise() { a <- b; b <- a; }
/// ```
fn swap(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Swap", F, 1));
    input(b, body, "clk", BIT, 20);
    output(b, body, "a", BIT, Constant::Bool(true), 30);
    output(b, body, "b", BIT, Constant::Bool(false), 40);
    let rise = Buck::rise(b, body, 0, F, 2, 8);
    let mut x = Dx::new(b, F, 2, 22);
    let other = x.carrier(2, BIT);
    let first = x.commit(1, other, None);
    x.at(2, 34);
    let other = x.carrier(1, BIT);
    let second = x.commit(2, other, None);
    Buck::reaction(b, body, vec![rise], vec![first, second], vec![], F, 2);
    body
}

#[test]
fn two_commits_exchange_two_carriers() {
    let mut w = Wire::new();
    clock(&mut w);
    let swap = swap(&mut w.b);
    w.place("swap", swap, vec![], &[("clk", BIT), ("a", BIT), ("b", BIT)]);
    let seen = Built::new(w.finish()).run(10e-9, 1e-9);
    // §12.1.1: every read sees the values at the start of the microstep and the commits of the
    // microstep apply together, so `a <- b; b <- a;` exchanges them. a = true, b = false at the
    // start; at the first rising edge (2.5 ns) a becomes false and b true; at the second
    // (7.5 ns) they exchange back. A `b <- a` that read the new `a` would leave both false at
    // 2.5 ns and give `b` no change.
    assert_eq!(seen.transient("a"), vec![(5 * NS / 2, LOW.to_vec()), (15 * NS / 2, HIGH.to_vec())]);
    assert_eq!(seen.transient("b"), vec![(5 * NS / 2, HIGH.to_vec()), (15 * NS / 2, LOW.to_vec())]);
}

/// `module Later(in clk: Bit, out x: Bit<u8> = 0)`:
///
/// ```text
/// on clk.rise() { x <- 1; x <- 2; }
/// ```
fn later(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Later", F, 1));
    input(b, body, "clk", BIT, 20);
    output(b, body, "x", Ty::u(8), Constant::Int(0), 30);
    let rise = Buck::rise(b, body, 0, F, 2, 8);
    let mut x = Dx::new(b, F, 2, 22);
    let one = x.int(1, Ty::u(8));
    let first = x.commit(1, one, None);
    x.at(2, 34);
    let two = x.int(2, Ty::u(8));
    let second = x.commit(1, two, None);
    Buck::reaction(b, body, vec![rise], vec![first, second], vec![], F, 2);
    body
}

#[test]
fn the_later_statement_wins() {
    let mut w = Wire::new();
    clock(&mut w);
    let later = later(&mut w.b);
    w.place("later", later, vec![], &[("clk", BIT), ("x", Ty::u(8))]);
    let seen = Built::new(w.finish()).run(10e-9, 1e-9);
    // §12.1.2: commits to one level that apply in the same microstep are ordered by the instant
    // they were made, then by source order; the last one is the value. At 2.5 ns `x` takes 2,
    // and 1 is never seen. At 7.5 ns the same commits leave the value as it is: no change.
    assert_eq!(seen.transient("x"), vec![(5 * NS / 2, vec![2])]);
}

/// `module Pulse(in clk: Bit, out p: Bit = false, out q: Bit = false)`:
///
/// ```text
/// on clk.rise() {
///     p <- true after 1e-9;  p <- false after 3e-9;     // t = 1 ns, t + w = 3 ns
///     q <- false after 3e-9; q <- true after 1e-9;      // the same, the statements swapped
/// }
/// ```
fn pulse(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Pulse", F, 1));
    input(b, body, "clk", BIT, 20);
    output(b, body, "p", BIT, Constant::Bool(false), 30);
    output(b, body, "q", BIT, Constant::Bool(false), 40);
    let rise = Buck::rise(b, body, 0, F, 2, 8);
    let mut x = Dx::new(b, F, 3, 9);
    let mut stmts = Vec::new();
    for (carrier, value, delay) in [(1, true, 1e-9), (1, false, 3e-9), (2, false, 3e-9), (2, true, 1e-9)] {
        let v = x.boolean(value);
        let d = x.real(delay);
        stmts.push(x.commit(carrier, v, Some(d)));
    }
    Buck::reaction(b, body, vec![rise], stmts, vec![], F, 2);
    body
}

#[test]
fn two_statements_make_a_pulse() {
    let mut w = Wire::new();
    clock(&mut w);
    let pulse = pulse(&mut w.b);
    w.place("pulse", pulse, vec![], &[("clk", BIT), ("p", BIT), ("q", BIT)]);
    // 7 ns: the second rising edge (7.5 ns) is not reached
    let seen = Built::new(w.finish()).run(7e-9, 1e-9);
    // §12.1.4: `p <- true after t; p <- false after t + w;` holds `p` true from `t` to `t + w`,
    // in either order of the two statements: the reaction runs at 2.5 ns, t = 1 ns, w = 2 ns, so
    // true from 3.5 ns to 5.5 ns.
    let pulse = vec![(7 * NS / 2, HIGH.to_vec()), (11 * NS / 2, LOW.to_vec())];
    assert_eq!(seen.transient("p"), pulse);
    assert_eq!(seen.transient("q"), pulse);
}

/// `module Latch(in clk: Bit, out q: Bit = false)` and `module Count(in q: Bit, out n: Bit<u8> = 0)`:
///
/// ```text
/// Latch: on clk.rise() { q <- true; }
/// Count: on q.changed() { n <- n + 1; }
/// ```
fn latch(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Latch", F, 1));
    input(b, body, "clk", BIT, 20);
    output(b, body, "q", BIT, Constant::Bool(false), 30);
    let rise = Buck::rise(b, body, 0, F, 2, 8);
    let mut x = Dx::new(b, F, 2, 22);
    let yes = x.boolean(true);
    let commit = x.commit(1, yes, None);
    Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 2);
    body
}

fn count(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Count", F, 10));
    input(b, body, "q", BIT, 20);
    output(b, body, "n", Ty::u(8), Constant::Int(0), 30);
    let changed = b.event(
        body,
        piperine_ir::Event { source: piperine_ir::EventSource::Changed(0), guard: None, origin: Buck::origin(F, 11, 8) },
    );
    let mut x = Dx::new(b, F, 11, 22);
    let n = x.carrier(1, Ty::u(8));
    let one = x.int(1, Ty::u(8));
    let next = x.bin(BinaryOp::Add, n, one, Ty::u(8));
    let commit = x.commit(1, next, None);
    Buck::reaction(b, body, vec![changed], vec![commit], vec![], F, 11);
    body
}

#[test]
fn an_unchanged_commit_fires_nothing() {
    let mut w = Wire::new();
    clock(&mut w);
    let (latch, count) = (latch(&mut w.b), count(&mut w.b));
    w.place("latch", latch, vec![], &[("clk", BIT), ("q", BIT)]);
    w.place("count", count, vec![], &[("q", BIT), ("n", Ty::u(8))]);
    // 20 ns: rising edges at 2.5, 7.5, 12.5 and 17.5 ns
    let seen = Built::new(w.finish()).run(20e-9, 1e-9);
    // §12.1.18: a commit that changes a level fires `changed()` where it applies, and one that
    // leaves the value as it was fires nothing. The latch commits true at four rising edges; it
    // changes `q` only at the first (2.5 ns)
    assert_eq!(seen.transient("q"), vec![(5 * NS / 2, HIGH.to_vec())]);
    // ... all four commits were made (a commit is a record of the reaction, changed or not)
    let made = seen.commits("Top.latch");
    assert_eq!(made.iter().filter(|t| **t % (5 * NS) == 5 * NS / 2).count(), 4, "four commits: {made:?}");
    // ... and `changed()` fired once in the transient, so the counter, which adds one per
    // firing, moved once, at 2.5 ns, to one more than the value it held (the operating point
    // already counted the initial value of `q`)
    let moved = seen.transient("n");
    assert_eq!(moved.len(), 1, "the counter moved once: {moved:?}");
    assert_eq!(moved[0].0, 5 * NS / 2);
    // (§12.1.18: one firing in the transient on top of the one the operating point counted, so
    // the counter reads 2 -- three more firings from the unchanged commits would read 5)
    assert_eq!(moved[0].1, vec![2], "the counter holds 2 after the one firing: {moved:?}");
}
