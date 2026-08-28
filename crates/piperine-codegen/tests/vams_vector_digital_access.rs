//! LRM-COVERS: 4.2.1, 4.2.1.3, A.2.2.1
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

fn nonblocking(dest: &str, value: Expr) -> Stmt {
    Stmt::NonBlockingAssign { span: SP, target: ident(dest), value }
}

/// A based literal `size'bdigits` (binary only — enough for T11's own
/// fixtures).
fn based_bin(size: u32, digits: &str) -> Expr {
    Expr::Number {
        span: SP,
        value: NumberLit::Based(BasedNumber {
            size: Some(size),
            signed: false,
            base: NumberBase::Binary,
            digits: digits.to_string(),
        }),
    }
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
        m.ports.push(Port::scalar(n, Direction::In));
    }
    m.ports.push(Port::scalar(o, Direction::Out));
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
        m.ports.push(Port::scalar(node, Direction::In));
    }
    m.ports.push(Port::scalar(result, Direction::Out));
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
        m.ports.push(Port::scalar(node, Direction::In));
    }
    m.ports.push(Port::scalar(result, Direction::Out));
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
    m.ports.push(Port::scalar(b4, Direction::In));
    m.ports.push(Port::scalar(result, Direction::Out));
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

// ── T11: whole-vector read/write (LRMW3-03, §4.2.1.3) ──────────────────────

/// `module Dut(output q[0..3]); q <= 4'b0101;` — `q` is a whole 4-bit
/// scalarized vector target, expanded by `expand_whole_vector_assign`
/// (`emit/stmt.rs`) into four per-bit `NonBlockingAssign`s.
fn literal_write_module() -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let q: Vec<_> = (0..4).map(|i| m.symbols.add_node(format!("q[{i}]"), Domain::Digital)).collect();
    for &n in &q {
        m.ports.push(Port::scalar(n, Direction::Out));
    }
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: q,
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![nonblocking("q", based_bin(4, "0101"))],
    });
    m
}

/// Done-when #1: `q <= 4'b0101;` sets the four bits per IEEE 1364 bit
/// order — digit string `"0101"` read right-to-left is lsb-first
/// (`q[0]=1, q[1]=0, q[2]=1, q[3]=0`), i.e. `q` reads as the number 5.
#[test]
fn whole_vector_nonblocking_write_sets_bits_in_1364_order() {
    let module = literal_write_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("whole-vector literal write must compile"));
    let (q0, q1, q2, q3) = (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3));
    let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![q0, q1, q2, q3], vec![]).expect("instance");
    let mut bench = Bench::new(4);
    bench.init(&mut instance);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[q0.0], LogicValue::One, "q[0] (lsb, rightmost digit) must be 1");
    assert_eq!(bench.nets[q1.0], LogicValue::Zero, "q[1] must be 0");
    assert_eq!(bench.nets[q2.0], LogicValue::One, "q[2] must be 1");
    assert_eq!(bench.nets[q3.0], LogicValue::Zero, "q[3] (msb, leftmost digit) must be 0");
}

/// `module Dut(output a[0..1], output b[0..1]); a <= b; b <= a;` — a
/// non-blocking whole-vector swap.
fn whole_vector_swap_module() -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let a: Vec<_> = (0..2).map(|i| m.symbols.add_node(format!("a[{i}]"), Domain::Digital)).collect();
    let b: Vec<_> = (0..2).map(|i| m.symbols.add_node(format!("b[{i}]"), Domain::Digital)).collect();
    for &n in a.iter().chain(&b) {
        m.ports.push(Port::scalar(n, Direction::Out));
    }
    let mut outputs = a.clone();
    outputs.extend(&b);
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs,
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![nonblocking("a", ident("b")), nonblocking("b", ident("a"))],
    });
    m
}

/// Done-when #2: `a <= b; b <= a;` swaps correctly — region semantics
/// (NBA reads live, writes stage) must survive scalarization: each
/// expanded per-bit statement is its own genuine `NonBlockingAssign`, so
/// `b`'s expansion still reads `a`'s *pre-step* value, not the value `a`'s
/// own (already-executed, but only staged) expansion just computed.
#[test]
fn whole_vector_nonblocking_swap_exchanges_values() {
    let module = whole_vector_swap_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("whole-vector swap must compile"));
    let (a0, a1, b0, b1) = (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3));
    let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![a0, a1, b0, b1], vec![]).expect("instance");
    let mut bench = Bench::new(4);
    bench.init(&mut instance);
    // a = 2'b01 (a0=1,a1=0), b = 2'b10 (b0=0,b1=1).
    bench.set(a0, LogicValue::One);
    bench.set(a1, LogicValue::Zero);
    bench.set(b0, LogicValue::Zero);
    bench.set(b1, LogicValue::One);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[a0.0], LogicValue::Zero, "a[0] <- old b[0]");
    assert_eq!(bench.nets[a1.0], LogicValue::One, "a[1] <- old b[1]");
    assert_eq!(bench.nets[b0.0], LogicValue::One, "b[0] <- old a[0]");
    assert_eq!(bench.nets[b1.0], LogicValue::Zero, "b[1] <- old a[1]");
}

/// `module Dut(output wide[0..3]); wide <= 2'b11;` (2-bit literal into a
/// 4-bit target: zero-extend) and a second module `wide2 <= narrow[0..1]`
/// where `narrow` only has 2 elements feeding a 4-wide target — both
/// exercise the zero-extend half of the width-mismatch rule; a 6-bit
/// literal into the same 4-bit target exercises the truncate half.
fn width_mismatch_module(literal: Expr) -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let wide: Vec<_> = (0..4).map(|i| m.symbols.add_node(format!("wide[{i}]"), Domain::Digital)).collect();
    for &n in &wide {
        m.ports.push(Port::scalar(n, Direction::Out));
    }
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: wide,
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![nonblocking("wide", literal)],
    });
    m
}

/// Done-when #3 (zero-extend half): a 2-bit literal `2'b11` into a 4-bit
/// target zero-extends the high two bits — `wide = 0b0011`, not `X`/
/// garbage in `wide[2]`/`wide[3]`.
#[test]
fn width_mismatch_zero_extends_a_narrower_literal() {
    let module = width_mismatch_module(based_bin(2, "11"));
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("narrower literal must zero-extend"));
    let (w0, w1, w2, w3) = (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3));
    let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![w0, w1, w2, w3], vec![]).expect("instance");
    let mut bench = Bench::new(4);
    bench.init(&mut instance);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[w0.0], LogicValue::One, "wide[0] = 1 (literal bit 0)");
    assert_eq!(bench.nets[w1.0], LogicValue::One, "wide[1] = 1 (literal bit 1)");
    assert_eq!(bench.nets[w2.0], LogicValue::Zero, "wide[2] must zero-extend, not X/garbage");
    assert_eq!(bench.nets[w3.0], LogicValue::Zero, "wide[3] must zero-extend, not X/garbage");
}

/// Done-when #3 (truncate half): a 6-bit literal `6'b110101` into a 4-bit
/// target discards its top two (excess) bits — only the low four bits
/// (`0101`) land.
#[test]
fn width_mismatch_truncates_a_wider_literal() {
    let module = width_mismatch_module(based_bin(6, "110101"));
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("wider literal must truncate"));
    let (w0, w1, w2, w3) = (DigitalNet(0), DigitalNet(1), DigitalNet(2), DigitalNet(3));
    let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![w0, w1, w2, w3], vec![]).expect("instance");
    let mut bench = Bench::new(4);
    bench.init(&mut instance);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[w0.0], LogicValue::One, "wide[0] = low nibble bit 0 (1)");
    assert_eq!(bench.nets[w1.0], LogicValue::Zero, "wide[1] = low nibble bit 1 (0)");
    assert_eq!(bench.nets[w2.0], LogicValue::One, "wide[2] = low nibble bit 2 (1)");
    assert_eq!(bench.nets[w3.0], LogicValue::Zero, "wide[3] = low nibble bit 3 (0), the excess `11` above is discarded");
}

// ── T12: memory read/write through a constant index (LRMW3-04, §A.2.2.1) ──

fn index_of_expr(base: &str, i: i64) -> Expr {
    index_of(base, i)
}

/// `reg [1:0] mem [0:1];` scalarized as `mem[0][0]`, `mem[0][1]`,
/// `mem[1][0]`, `mem[1][1]` (T5's nested naming) — an 8-node module:
/// `mem[1] <= 2'b10;` writes word 1 through a constant index, then `d <=
/// mem[1];` and `e <= mem[0];` read word 1 and word 0 back into two
/// 2-bit output vectors.
fn memory_module() -> LoweredBody {
    let mut m = LoweredBody::new("Dut");
    let mem00 = m.symbols.add_node("mem[0][0]", Domain::Digital);
    let mem01 = m.symbols.add_node("mem[0][1]", Domain::Digital);
    let mem10 = m.symbols.add_node("mem[1][0]", Domain::Digital);
    let mem11 = m.symbols.add_node("mem[1][1]", Domain::Digital);
    let d0 = m.symbols.add_node("d[0]", Domain::Digital);
    let d1 = m.symbols.add_node("d[1]", Domain::Digital);
    let e0 = m.symbols.add_node("e[0]", Domain::Digital);
    let e1 = m.symbols.add_node("e[1]", Domain::Digital);
    for n in [mem00, mem01, mem10, mem11] {
        m.ports.push(Port::scalar(n, Direction::Out));
    }
    for n in [d0, d1, e0, e1] {
        m.ports.push(Port::scalar(n, Direction::Out));
    }
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: vec![mem00, mem01, mem10, mem11, d0, d1, e0, e1],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![
            nonblocking("mem", based_bin(2, "10")).with_index_target("mem", 1),
            nonblocking("d", index_of_expr("mem", 1)),
            nonblocking("e", index_of_expr("mem", 0)),
        ],
    });
    m
}

/// `Stmt` builder helper: rewrites a `nonblocking(base, value)`'s target
/// from `Ident(base)` to `Index(base, i)` — used only to keep
/// `memory_module`'s call sites reading as `mem[1] <= …` rather than a
/// bespoke constructor per statement.
trait WithIndexTarget {
    fn with_index_target(self, base: &str, i: i64) -> Stmt;
}
impl WithIndexTarget for Stmt {
    fn with_index_target(self, base: &str, i: i64) -> Stmt {
        match self {
            Stmt::NonBlockingAssign { span, value, .. } => {
                Stmt::NonBlockingAssign { span, target: index_of(base, i), value }
            }
            Stmt::Assign { span, value, .. } => Stmt::Assign { span, target: index_of(base, i), value },
            other => other,
        }
    }
}

/// Done-when #1: a word written at a constant index reads back at that
/// index (`mem[1] <= 2'b10` then `d <= mem[1]` gives `d = 2'b10`) and
/// leaves the *other* word untouched — `mem[0]` was never written, so
/// `e <= mem[0]` must read back `X` in both bits, asserted per word (not
/// just that *some* memory read happened).
#[test]
fn memory_word_write_and_read_through_constant_index() {
    let module = memory_module();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("memory word read/write must compile"));
    let nets: Vec<DigitalNet> = (0..8).map(DigitalNet).collect();
    let (d0, d1, e0, e1) = (nets[4], nets[5], nets[6], nets[7]);
    let mut instance = DigitalInstance::new(kernel, 0, vec![], nets.clone(), vec![]).expect("instance");
    let mut bench = Bench::new(8);
    bench.init(&mut instance);
    bench.step(0.0, &mut instance);

    // 2'b10: digit '0' (rightmost) -> bit0=0, digit '1' (leftmost) -> bit1=1.
    assert_eq!(bench.nets[d0.0], LogicValue::Zero, "d[0] must be word 1's bit 0 (written 2'b10 -> bit0=0)");
    assert_eq!(bench.nets[d1.0], LogicValue::One, "d[1] must be word 1's bit 1 (written 2'b10 -> bit1=1)");
    assert_eq!(bench.nets[e0.0], LogicValue::X, "e[0] (word 0, never written) must read X, not a silent 0");
    assert_eq!(bench.nets[e1.0], LogicValue::X, "e[1] (word 0, never written) must read X, not a silent 0");
}

/// Done-when #2: a non-constant memory index fails loud — `k` is a
/// runtime `integer` variable, never an elaboration constant. `d <=
/// mem[k];` reads through the non-constant index directly (the read side
/// of `rhs_bits_for_vector_assign`'s `Expr::Index` arm).
#[test]
fn non_constant_memory_index_fails_loud() {
    let mut m = LoweredBody::new("Dut");
    let mem00 = m.symbols.add_node("mem[0][0]", Domain::Digital);
    let mem10 = m.symbols.add_node("mem[1][0]", Domain::Digital);
    let d0 = m.symbols.add_node("d[0]", Domain::Digital);
    let k = m.symbols.add_var("k", Type::Integer);
    m.ports.push(Port::scalar(mem00, Direction::Out));
    m.ports.push(Port::scalar(mem10, Direction::Out));
    m.ports.push(Port::scalar(d0, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: vec![mem00, mem10, d0],
        regs: vec![k],
        reg_inits: vec![(k, int(0))],
        stmts: vec![Stmt::NonBlockingAssign {
            span: SP,
            target: ident("d"),
            value: Expr::Index { span: SP, base: Box::new(ident("mem")), index: Box::new(ident("k")) },
        }],
    });
    let err = match DigitalKernel::compile(&m) {
        Ok(_) => panic!("a non-constant memory index must fail loud, never silently resolve"),
        Err(e) => e,
    };
    let msg = err.to_string();
    assert!(msg.contains("5.5.2"), "error must cite LRM §5.5.2 (constant-index rule), got: {msg}");
}
