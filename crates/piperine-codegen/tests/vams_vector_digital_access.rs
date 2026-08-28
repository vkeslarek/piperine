//! LRM-COVERS: 4.2.1
//! LRMW3-01/LRMW3-03 (`lrm-conformance-wave3`): digital access through a
//! vector index/part-select. Scalarized representation (MD-52): a width-N
//! digital vector is N independently-named nets `q[0]`..`q[N-1]`
//! (`structure.rs`'s existing element-node registration, domain-agnostic —
//! already exercised for analog vectors, reused here unchanged for
//! digital), so `q[2]` resolving is exactly `load_ident("q[2]")` through
//! the same symbol table.
//!
//! Hand-built `LoweredBody`s compiled through `DigitalKernel` and driven
//! through `DigitalInstance` directly — `digital_jit.rs`'s own style —
//! rather than parsed source, because a *vector as a module port* does not
//! expand into per-element nodes yet (`structure.rs`'s port-registration
//! loop is scalar-only; that gap is T13's, not this file's). Each bus bit
//! is instead declared as its own scalar node, addressed through
//! `Expr::Index`/`Expr::PartSelect` exactly as the parser would produce
//! for a real `q[2]`/`b[7:4]` source expression.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_lang::ast::{EventExpr, Expr, Stmt};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::{BasedNumber, NumberBase, NumberLit};

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::*;
use piperine_codegen::DigitalKernel;
use piperine_solver::abi::DigitalEvent;
use piperine_solver::prelude::{DigitalNet, LogicValue};

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

fn int(v: i64) -> Expr {
    Expr::Number { span: SP, value: NumberLit::Integer(v) }
}

/// `base[index]` — the same shape the parser builds for `q[2]`.
fn index_of(base: &str, index: i64) -> Expr {
    Expr::Index { span: SP, base: Box::new(ident(base)), index: Box::new(int(index)) }
}

/// `base[msb:lsb]` — the same shape the parser builds for `b[7:4]`.
fn part_select(base: &str, msb: i64, lsb: i64) -> Expr {
    Expr::PartSelect { span: SP, base: Box::new(ident(base)), msb: Box::new(int(msb)), lsb: Box::new(int(lsb)) }
}

#[allow(dead_code)]
fn bit(digit: &str) -> Expr {
    Expr::Number {
        span: SP,
        value: NumberLit::Based(BasedNumber {
            size: Some(1),
            signed: false,
            base: NumberBase::Binary,
            digits: digit.to_string(),
        }),
    }
}

fn assign(dest: &str, value: Expr) -> Stmt {
    Stmt::Assign { span: SP, target: ident(dest), value }
}

fn block(stmts: Vec<Stmt>) -> Stmt {
    Stmt::Block { span: SP, name: None, decls: vec![], stmts }
}

fn posedge(sig: &str, body: Stmt) -> Stmt {
    Stmt::EventControl { span: SP, events: vec![EventExpr::Posedge(ident(sig))], body: Box::new(body) }
}

/// Power-on register value 0.
#[allow(dead_code)] // used starting T11
fn zero_init(var: VarId) -> (VarId, Expr) {
    (var, Expr::Number { span: SP, value: NumberLit::Integer(0) })
}

/// A tiny testbench: holds net values, applies every event a step emits.
struct Bench {
    nets: Vec<LogicValue>,
    queue: BinaryHeap<Reverse<DigitalEvent>>,
}

impl Bench {
    fn new(num_nets: usize) -> Self {
        Self { nets: vec![LogicValue::X; num_nets], queue: BinaryHeap::new() }
    }

    fn set(&mut self, net: DigitalNet, value: LogicValue) {
        self.nets[net.0] = value;
    }

    fn init(&mut self, instance: &mut DigitalInstance) {
        instance.init(&mut self.queue);
        while let Some(Reverse(event)) = self.queue.pop() {
            self.nets[event.net.0] = event.value;
        }
    }

    fn step(&mut self, t: f64, instance: &mut DigitalInstance) {
        instance.eval(t, &self.nets, &[], &mut self.queue);
        while let Some(Reverse(event)) = self.queue.pop() {
            self.nets[event.net.0] = event.value;
        }
    }
}

/// `module Dut(input q[0..3], input clk, output o); always @(posedge clk) o
/// = q[2];` — `q[0]`..`q[3]` are four independent scalar nets (the
/// scalarized representation), `clk`/`o` two more; `o`'s digital body
/// reads `Expr::Index{base: "q", index: 2}`.
fn bit_select_module() -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let q0 = m.symbols.add_node("q[0]", Domain::Digital);
    let q1 = m.symbols.add_node("q[1]", Domain::Digital);
    let q2 = m.symbols.add_node("q[2]", Domain::Digital);
    let q3 = m.symbols.add_node("q[3]", Domain::Digital);
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let o = m.symbols.add_node("o", Domain::Digital);
    for n in [q0, q1, q2, q3, clk] {
        m.ports.push(Port { node: n, direction: Direction::In });
    }
    m.ports.push(Port { node: o, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![q0, q1, q2, q3, clk],
        outputs: vec![o],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![posedge("clk", block(vec![assign("o", index_of("q", 2))]))],
    });
    m
}

/// Done-when #1: `always @(posedge clk) o = q[2];` compiles and propagates
/// that bit — driving `q[2]` high (with `q[0]`/`q[1]`/`q[3]` at other
/// values) must make `o` follow `q[2]` specifically, proving the index
/// resolved to the *right* net, not just *a* net.
#[test]
fn bit_select_reads_the_indexed_bit_on_a_clock_edge() {
    let module = bit_select_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("bit-select module must compile"));
    let (q0, q1, q2, q3, clk, o) =
        (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3), DigitalNet(4), DigitalNet(5));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![q0, q1, q2, q3, clk], vec![o], vec![]).expect("instance");

    let mut bench = Bench::new(6);
    bench.init(&mut instance);

    // q = 4'b0100 (q3 q2 q1 q0 = 0,1,0,0) — only q[2] is high.
    bench.set(q0, LogicValue::Zero);
    bench.set(q1, LogicValue::Zero);
    bench.set(q2, LogicValue::One);
    bench.set(q3, LogicValue::Zero);
    bench.set(clk, LogicValue::Zero);
    bench.step(0.0, &mut instance);

    bench.set(clk, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[o.0], LogicValue::One, "o must follow q[2] (=1), not q[0]/q[1]/q[3] (=0)");

    // Flip which bit is high — q[1] instead of q[2] — o must now read 0,
    // discriminating against an off-by-one/wrong-bit resolution.
    bench.set(q1, LogicValue::One);
    bench.set(q2, LogicValue::Zero);
    bench.set(clk, LogicValue::Zero);
    bench.step(2.0, &mut instance);
    bench.set(clk, LogicValue::One);
    bench.step(3.0, &mut instance);
    assert_eq!(bench.nets[o.0], LogicValue::Zero, "o must track q[2] specifically, which is now 0");
}

/// `module Dut(input b[4..7], output o); o = b[7:4];` — a part-select, not
/// yet lowered by this task (T10's job) — used only to prove the
/// `Expr::Index | Expr::PartSelect` arm was *narrowed*, not deleted:
/// `Expr::PartSelect` must still fail loud, by name, not silently emit 0.
fn part_select_module() -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let b4 = m.symbols.add_node("b[4]", Domain::Digital);
    let b7 = m.symbols.add_node("b[7]", Domain::Digital);
    let o = m.symbols.add_node("o", Domain::Digital);
    m.ports.push(Port { node: b4, direction: Direction::In });
    m.ports.push(Port { node: b7, direction: Direction::In });
    m.ports.push(Port { node: o, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![b4, b7],
        outputs: vec![o],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![assign("o", part_select("b", 7, 4))],
    });
    m
}

/// Done-when #3: "anything the arm still cannot lower keeps failing loud"
/// — `Expr::PartSelect` (T10's job, not this task's) must still be
/// rejected, by name, when it reaches digital codegen, not silently
/// treated as bit-select or as a no-op.
#[test]
fn part_select_still_fails_loud_after_the_bit_select_widening() {
    let module = part_select_module();
    let err = match DigitalKernel::compile(&module) {
        Ok(_) => panic!("a part-select must still fail loud (T10, not this task)"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(msg.contains("part-select"), "error must name the actual unsupported construct, got: {msg}");
}

/// Done-when #2: reading a bit of an undriven vector yields `X` (Table
/// 7-3), not a silent `0` — `q[2]` is never `set` on the bench at all, so
/// its net value is `LogicValue::X` (the bench's own initial fill), and
/// `o` must capture that `X`, not default to `Zero`.
#[test]
fn bit_select_of_an_undriven_net_reads_x_not_zero() {
    let module = bit_select_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("bit-select module must compile"));
    let (q0, q1, q3, clk, o) = (DigitalNet(0), DigitalNet(1), DigitalNet(3), DigitalNet(4), DigitalNet(5));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![q0, q1, DigitalNet(2), q3, clk], vec![o], vec![]).expect("instance");

    let mut bench = Bench::new(6);
    bench.init(&mut instance);
    // Every other bit driven; q[2] deliberately left untouched (X).
    bench.set(q0, LogicValue::Zero);
    bench.set(q1, LogicValue::Zero);
    bench.set(q3, LogicValue::Zero);
    bench.set(clk, LogicValue::Zero);
    bench.step(0.0, &mut instance);

    bench.set(clk, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[o.0], LogicValue::X, "an undriven q[2] must read back X (Table 7-3), never a silent 0");
}
