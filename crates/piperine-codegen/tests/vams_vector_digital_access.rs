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

/// T10 (LRMW3-03): `n = b[7:4];` (`n` a plain `integer`-typed variable,
/// the natural single-value consumer of a packed part-select read) packs
/// the four selected bits declared-order-MSB-first, then a scalar output
/// `result` reports whether `n` matches the expected 4-bit pattern —
/// `DigitalNet`/`LogicValue` are strictly single-bit, so a packed
/// multi-bit value can only be *observed* here through a derived
/// comparison, not read back directly as one net.
fn part_select_pack_module() -> LoweredBody {
    use piperine_lang::ast::BinOp as VBinOp;
    let mut m = LoweredBody::new("Dut");
    let b4 = m.symbols.add_node("b[4]", Domain::Digital);
    let b5 = m.symbols.add_node("b[5]", Domain::Digital);
    let b6 = m.symbols.add_node("b[6]", Domain::Digital);
    let b7 = m.symbols.add_node("b[7]", Domain::Digital);
    let result = m.symbols.add_node("result", Domain::Digital);
    let n = m.symbols.add_var("n", Type::Integer);
    for node in [b4, b5, b6, b7] {
        m.ports.push(Port { node, direction: Direction::In });
    }
    m.ports.push(Port { node: result, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![b4, b5, b6, b7],
        outputs: vec![result],
        regs: vec![n],
        reg_inits: vec![],
        stmts: vec![
            assign("n", part_select("b", 7, 4)),
            assign(
                "result",
                Expr::Binary { span: SP, op: VBinOp::Eq, lhs: Box::new(ident("n")), rhs: Box::new(int(11)) },
            ),
        ],
    });
    m
}

/// Done-when #1: `n = b[7:4];` assigns the four selected bits in declared
/// order — `b7,b6,b5,b4 = 1,0,1,1` packs (MSB-first, declared order) to
/// `0b1011 = 11`, so `result` (`n == 11`) reads `One`; flipping one input
/// bit (a discriminating check against an off-by-one/wrong-order pack)
/// must flip `result` to `Zero`.
#[test]
fn part_select_packs_bits_in_declared_order() {
    let module = part_select_pack_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("part-select read must compile"));
    let (b4, b5, b6, b7, result) = (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3), DigitalNet(4));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![b4, b5, b6, b7], vec![result], vec![]).expect("instance");
    let mut bench = Bench::new(5);
    bench.init(&mut instance);

    // b7 b6 b5 b4 = 1 0 1 1 -> 0b1011 = 11.
    bench.set(b7, LogicValue::One);
    bench.set(b6, LogicValue::Zero);
    bench.set(b5, LogicValue::One);
    bench.set(b4, LogicValue::One);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[result.0], LogicValue::One, "b[7:4] = 1011 = 11 must match n == 11");

    // Flip b5 (the declared-order-2nd-from-lsb bit): 0b1001 = 9 != 11.
    bench.set(b5, LogicValue::Zero);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[result.0], LogicValue::Zero, "b[7:4] = 1001 = 9 must not match n == 11");
}

/// Done-when #2: a descending part-select (`b[7:4]`) and an ascending
/// part-select of the same four physical positions (`c[4:7]`, legal on a
/// net declared with an ascending range) agree bit for bit — feeding the
/// same values at the same *declared-order position* (first-listed name to
/// first-listed name, and so on) on both must pack to the same value.
fn part_select_direction_module() -> LoweredBody {
    use piperine_lang::ast::BinOp as VBinOp;
    let mut m = LoweredBody::new("Dut");
    let b4 = m.symbols.add_node("b[4]", Domain::Digital);
    let b5 = m.symbols.add_node("b[5]", Domain::Digital);
    let b6 = m.symbols.add_node("b[6]", Domain::Digital);
    let b7 = m.symbols.add_node("b[7]", Domain::Digital);
    let c4 = m.symbols.add_node("c[4]", Domain::Digital);
    let c5 = m.symbols.add_node("c[5]", Domain::Digital);
    let c6 = m.symbols.add_node("c[6]", Domain::Digital);
    let c7 = m.symbols.add_node("c[7]", Domain::Digital);
    let result = m.symbols.add_node("result", Domain::Digital);
    let n1 = m.symbols.add_var("n1", Type::Integer);
    let n2 = m.symbols.add_var("n2", Type::Integer);
    for node in [b4, b5, b6, b7, c4, c5, c6, c7] {
        m.ports.push(Port { node, direction: Direction::In });
    }
    m.ports.push(Port { node: result, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![b4, b5, b6, b7, c4, c5, c6, c7],
        outputs: vec![result],
        regs: vec![n1, n2],
        reg_inits: vec![],
        stmts: vec![
            assign("n1", part_select("b", 7, 4)),
            assign("n2", part_select("c", 4, 7)),
            assign(
                "result",
                Expr::Binary { span: SP, op: VBinOp::Eq, lhs: Box::new(ident("n1")), rhs: Box::new(ident("n2")) },
            ),
        ],
    });
    m
}

#[test]
fn descending_and_ascending_part_selects_of_the_same_bits_agree() {
    let module = part_select_direction_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("dual part-select read must compile"));
    let (b4, b5, b6, b7, c4, c5, c6, c7, result) = (
        DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3),
        DigitalNet(4), DigitalNet(5), DigitalNet(6), DigitalNet(7),
        DigitalNet(8),
    );
    let mut instance = DigitalInstance::new(
        kernel,
        0,
        vec![b4, b5, b6, b7, c4, c5, c6, c7],
        vec![result],
        vec![],
    )
    .expect("instance");
    let mut bench = Bench::new(9);
    bench.init(&mut instance);

    // b[7:4] declared order = [b7,b6,b5,b4] = [1,0,1,1].
    // c[4:7] declared order = [c4,c5,c6,c7] — set to the SAME sequence
    // position-for-position: c4=1, c5=0, c6=1, c7=1.
    bench.set(b7, LogicValue::One);
    bench.set(b6, LogicValue::Zero);
    bench.set(b5, LogicValue::One);
    bench.set(b4, LogicValue::One);
    bench.set(c4, LogicValue::One);
    bench.set(c5, LogicValue::Zero);
    bench.set(c6, LogicValue::One);
    bench.set(c7, LogicValue::One);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[result.0], LogicValue::One, "matching declared-order sequences must pack to the same value");

    // Break the agreement: c6 now disagrees with b5's position.
    bench.set(c6, LogicValue::Zero);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[result.0], LogicValue::Zero, "a disagreeing bit must be detected, not silently ignored");
}

/// Done-when #3: an out-of-range part-select fails loud — `b` only has
/// elements `b[4]..b[7]`; `b[9:8]` names `b[9]`/`b[8]`, neither of which
/// any vector element ever registered.
#[test]
fn out_of_range_part_select_fails_loud() {
    let mut m = LoweredBody::new("Dut");
    let b4 = m.symbols.add_node("b[4]", Domain::Digital);
    let result = m.symbols.add_node("result", Domain::Digital);
    let n = m.symbols.add_var("n", Type::Integer);
    m.ports.push(Port { node: b4, direction: Direction::In });
    m.ports.push(Port { node: result, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![b4],
        outputs: vec![result],
        regs: vec![n],
        reg_inits: vec![],
        stmts: vec![assign("n", part_select("b", 9, 8))],
    });
    let err = match DigitalKernel::compile(&m) {
        Ok(_) => panic!("an out-of-range part-select must fail loud, not silently resolve"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("b[9]") || msg.contains("unresolved identifier"),
        "error must name the missing element, got: {msg}"
    );
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
