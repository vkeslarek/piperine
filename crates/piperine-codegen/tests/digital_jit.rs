//! Digital JIT tests: hand-built POM-Stmt digital bodies compiled to native
//! kernels and driven through the event-driven device wrapper.
//!
//! LRM-COVERS: 8.5.3, 8.5.3.3, 8.5.3.4
//! 8.5.3 ("Scheduling implication of assignments") is a purely
//! organizational heading over 8.5.3.1-8.5.3.7 with no independently
//! testable content of its own; it's carried here alongside two of its
//! genuinely-tested children (blocking/non-blocking assignment scheduling).

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_lang::ast::{BinOp, CaseArm, CaseKind, EventExpr, Expr, Stmt, UnOp};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::{BasedNumber, NumberBase, NumberLit};

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::*;
use piperine_codegen::DigitalKernel;
use piperine_solver::abi::DigitalEvent; use piperine_solver::prelude::{DigitalNet, LogicValue};

// ── VAMS-AST constructors ────────────────────────────────────────────────────
//
// These bodies are hand-built rather than parsed on purpose: the point is to
// exercise `DigitalKernel::compile` and `DigitalInstance` in isolation from
// elaboration (the same style the emission-layer tests use).

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

/// A one-bit based literal — `1'b0`, `1'b1`, `1'bx`.
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

/// `<=` — a non-blocking assignment (T23/LRMW2-14).
fn nonblocking(dest: &str, value: Expr) -> Stmt {
    Stmt::NonBlockingAssign { span: SP, target: ident(dest), value }
}

fn block(stmts: Vec<Stmt>) -> Stmt {
    Stmt::Block { span: SP, name: None, decls: vec![], stmts }
}

/// `always @(posedge sig) body` — the clocked form `compile` splits out.
fn posedge(sig: &str, body: Stmt) -> Stmt {
    Stmt::EventControl {
        span: SP,
        events: vec![EventExpr::Posedge(ident(sig))],
        body: Box::new(body),
    }
}

/// Register power-on value 0 (`reg_inits` takes a constant POM expression).
fn zero_init(var: VarId) -> (VarId, Expr) {
    (var, Expr::Number { span: SP, value: NumberLit::Integer(0) })
}

/// `inverter`: `y = ~a` (combinational).
fn inverter() -> LoweredBody {
    let mut m = LoweredBody::new("inverter");
    let a = m.symbols.add_node("a", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    m.ports.push(Port::scalar(a, Direction::In));
    m.ports.push(Port::scalar(y, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![a],
        outputs: vec![y],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![assign(
            "y",
            Expr::Unary { span: SP, op: UnOp::BitNot, operand: Box::new(ident("a")) },
        )],
    output_drive_strength: Default::default() });
    m
}

/// `dff`: `q` follows `d` on `posedge clk`, reset value 0.
fn dff() -> LoweredBody {
    let mut m = LoweredBody::new("dff");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let d = m.symbols.add_node("d", Domain::Digital);
    let q = m.symbols.add_node("q", Domain::Digital);
    let r = m.symbols.add_var("r", Type::Quad);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.ports.push(Port::scalar(d, Direction::In));
    m.ports.push(Port::scalar(q, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![clk, d],
        outputs: vec![q],
        regs: vec![r],
        reg_inits: vec![zero_init(r)],
        stmts: vec![
            posedge("clk", block(vec![assign("r", ident("d"))])),
            assign("q", ident("r")),
        ],
    output_drive_strength: Default::default() });
    m
}

/// Drives one `DigitalInstance` directly: a tiny testbench holding net
/// values and delivering events immediately.
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

    /// Power-on: apply register inits and the instance's t=0 outputs.
    fn init(&mut self, instance: &mut DigitalInstance) {
        instance.init(&mut self.queue);
        while let Some(Reverse(event)) = self.queue.pop() {
            self.nets[event.net.0] = event.value;
        }
    }

    /// Evaluate the instance at `t` and apply every event it emitted.
    fn step(&mut self, t: f64, instance: &mut DigitalInstance) {
        instance.eval(t, &self.nets, &[], &mut self.queue);
        while let Some(Reverse(event)) = self.queue.pop() {
            self.nets[event.net.0] = event.value;
        }
    }
}

#[test]
fn inverter_computes_quad_not() {
    let module = inverter();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile inverter"));
    let (a, y) = (DigitalNet(0), DigitalNet(1));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![a], vec![y], vec![]).expect("instance");

    let mut bench = Bench::new(2);
    bench.init(&mut instance);
    bench.set(a, LogicValue::Zero);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::One);

    bench.set(a, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::Zero);

    bench.set(a, LogicValue::X);
    bench.step(2.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::X, "X propagates");
}

#[test]
fn dff_captures_on_rising_edge_only() {
    let module = dff();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile dff"));
    let (clk, d, q) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![clk, d], vec![q], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);
    // Reset state: q = 0 (register init).
    bench.set(clk, LogicValue::Zero);
    bench.set(d, LogicValue::One);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[q.0], LogicValue::Zero, "no edge yet");

    // Data changes without a clock edge: q holds.
    bench.set(d, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[q.0], LogicValue::Zero, "level-insensitive");

    // Rising edge captures d = 1.
    bench.set(clk, LogicValue::One);
    bench.step(2.0, &mut instance);
    assert_eq!(bench.nets[q.0], LogicValue::One, "captured on posedge");

    // Falling edge: no capture.
    bench.set(d, LogicValue::Zero);
    bench.set(clk, LogicValue::Zero);
    bench.step(3.0, &mut instance);
    assert_eq!(bench.nets[q.0], LogicValue::One, "negedge ignored");

    // Next rising edge captures d = 0.
    bench.set(clk, LogicValue::One);
    bench.step(4.0, &mut instance);
    assert_eq!(bench.nets[q.0], LogicValue::Zero, "captured new data");
}

#[test]
fn pipeline_reads_pre_edge_values() {
    // Two registers in one clocked block: r2 <= r1; r1 <= d. On a single
    // edge r2 must take r1's *old* value (a pipeline, not a wire).
    let mut m = LoweredBody::new("pipe2");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let d = m.symbols.add_node("d", Domain::Digital);
    let q = m.symbols.add_node("q", Domain::Digital);
    let r1 = m.symbols.add_var("r1", Type::Quad);
    let r2 = m.symbols.add_var("r2", Type::Quad);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.ports.push(Port::scalar(d, Direction::In));
    m.ports.push(Port::scalar(q, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![clk, d],
        outputs: vec![q],
        regs: vec![r1, r2],
        reg_inits: vec![zero_init(r1), zero_init(r2)],
        stmts: vec![
            posedge(
                "clk",
                block(vec![assign("r2", ident("r1")), assign("r1", ident("d"))]),
            ),
            assign("q", ident("r2")),
        ],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile pipe2"));
    let (clk_n, d_n, q_n) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![clk_n, d_n], vec![q_n], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);
    bench.set(clk_n, LogicValue::Zero);
    bench.set(d_n, LogicValue::One);
    bench.step(0.0, &mut instance);

    // Edge 1: r1 ← 1, r2 ← old r1 (0). q = 0.
    bench.set(clk_n, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[q_n.0], LogicValue::Zero, "one-stage latency");

    // Edge 2: r2 ← 1. q = 1.
    bench.set(clk_n, LogicValue::Zero);
    bench.step(2.0, &mut instance);
    bench.set(clk_n, LogicValue::One);
    bench.step(3.0, &mut instance);
    assert_eq!(bench.nets[q_n.0], LogicValue::One, "data arrives after two edges");
}

#[test]
fn match_selects_arm_and_default() {
    // `case (a) 1'b0: y = 1'b1; 1'b1: y = 1'b0; default: y = 1'bx; endcase`
    // — VAMS's spelling of the arm-plus-default selection PHDL wrote as a
    // `match` statement over quad values.
    let mut m = LoweredBody::new("mux_match");
    let a = m.symbols.add_node("a", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    m.ports.push(Port::scalar(a, Direction::In));
    m.ports.push(Port::scalar(y, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![a],
        outputs: vec![y],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![Stmt::Case {
            span: SP,
            kind: CaseKind::Case,
            selector: ident("a"),
            arms: vec![
                CaseArm {
                    span: SP,
                    labels: Some(vec![bit("0")]),
                    body: Box::new(assign("y", bit("1"))),
                },
                CaseArm {
                    span: SP,
                    labels: Some(vec![bit("1")]),
                    body: Box::new(assign("y", bit("0"))),
                },
                CaseArm { span: SP, labels: None, body: Box::new(assign("y", bit("x"))) },
            ],
            attrs: vec![],
        }],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile mux_match"));
    let (a_n, y_n) = (DigitalNet(0), DigitalNet(1));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![a_n], vec![y_n], vec![]).expect("instance");

    let mut bench = Bench::new(2);
    bench.init(&mut instance);
    for (input, expected) in [
        (LogicValue::Zero, LogicValue::One),
        (LogicValue::One, LogicValue::Zero),
        (LogicValue::X, LogicValue::X),
    ] {
        bench.set(a_n, input);
        bench.step(0.0, &mut instance);
        assert_eq!(bench.nets[y_n.0], expected, "match({input:?})");
    }
}

// ── T23/LRMW2-14: `=` (blocking, Active region) vs `<=` (non-blocking, Nba
// region) — LRM §8.5.3.3/4 ──────────────────────────────────────────────────

/// `always @(posedge clk) begin qa <= qb; qb <= qa; end` — spec AC9's own
/// worked example: both `<=` right-hand sides read the pre-edge value
/// (neither write is visible to the other statement in the same block), so
/// the pair exchanges.
fn nba_swap() -> LoweredBody {
    let mut m = LoweredBody::new("nba_swap");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let qa = m.symbols.add_node("qa", Domain::Digital);
    let qb = m.symbols.add_node("qb", Domain::Digital);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.ports.push(Port::scalar(qa, Direction::Out));
    m.ports.push(Port::scalar(qb, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![clk],
        outputs: vec![qa, qb],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![posedge(
            "clk",
            block(vec![nonblocking("qa", ident("qb")), nonblocking("qb", ident("qa"))]),
        )],
    output_drive_strength: Default::default() });
    m
}

#[test]
fn nonblocking_swap_in_a_clocked_block_exchanges_values() {
    let module = nba_swap();
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile nba_swap"));
    let (clk, qa, qb) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![clk], vec![qa, qb], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);
    // Outputs *are* the register state here (no separate backing var) —
    // preload them directly, mirroring the real `output reg` idiom's
    // power-on/previous-cycle value.
    bench.set(clk, LogicValue::Zero);
    bench.set(qa, LogicValue::Zero);
    bench.set(qb, LogicValue::One);
    bench.step(0.0, &mut instance);

    bench.set(clk, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[qa.0], LogicValue::One, "qa <- old qb (AC9 swap)");
    assert_eq!(bench.nets[qb.0], LogicValue::Zero, "qb <- old qa (AC9 swap)");
}

/// AC9's contrast case: the `=` (blocking) form of the identical statement
/// pair does **not** swap — `qa = qb;` writes live immediately, so
/// `qb = qa;` reads the just-written value and both end up holding the old
/// `qb` (a clobber, not an exchange).
#[test]
fn blocking_assignment_in_a_clocked_block_does_not_swap() {
    let mut m = LoweredBody::new("blocking_no_swap");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let qa = m.symbols.add_node("qa", Domain::Digital);
    let qb = m.symbols.add_node("qb", Domain::Digital);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.ports.push(Port::scalar(qa, Direction::Out));
    m.ports.push(Port::scalar(qb, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![clk],
        outputs: vec![qa, qb],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![posedge("clk", block(vec![assign("qa", ident("qb")), assign("qb", ident("qa"))]))],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile blocking_no_swap"));
    let (clk, qa, qb) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![clk], vec![qa, qb], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);
    bench.set(clk, LogicValue::Zero);
    bench.set(qa, LogicValue::Zero);
    bench.set(qb, LogicValue::One);
    bench.step(0.0, &mut instance);

    bench.set(clk, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[qa.0], LogicValue::One, "qa = qb (blocking, immediate)");
    assert_eq!(bench.nets[qb.0], LogicValue::One, "qb = qa reads the JUST-WRITTEN qa, not old qa — a clobber");
}

/// Done-when: "combinational `<=`... behave per §8.5.3.3/4" — the swap
/// idiom also holds with no `always`/event control at all (two bare `<=`
/// statements land in `compile`'s comb-only classification): both writes
/// still stage, so the pair exchanges rather than clobbers.
#[test]
fn nonblocking_swap_in_a_combinational_block_exchanges_values() {
    let mut m = LoweredBody::new("nba_swap_comb");
    let ya = m.symbols.add_node("ya", Domain::Digital);
    let yb = m.symbols.add_node("yb", Domain::Digital);
    m.ports.push(Port::scalar(ya, Direction::Out));
    m.ports.push(Port::scalar(yb, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: vec![ya, yb],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![nonblocking("ya", ident("yb")), nonblocking("yb", ident("ya"))],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile nba_swap_comb"));
    let (ya_n, yb_n) = (DigitalNet(0), DigitalNet(1));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![], vec![ya_n, yb_n], vec![]).expect("instance");

    let mut bench = Bench::new(2);
    bench.init(&mut instance);
    bench.set(ya_n, LogicValue::Zero);
    bench.set(yb_n, LogicValue::One);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[ya_n.0], LogicValue::One, "ya <- old yb (comb NBA swap)");
    assert_eq!(bench.nets[yb_n.0], LogicValue::Zero, "yb <- old ya (comb NBA swap)");
}

/// A blocking write inside a clocked block is visible to a *later*
/// non-blocking right-hand side in the same block: `x = a;` (§8.5.3.3)
/// lands live immediately, so `y <= x;` (§8.5.3.4) computes its
/// right-hand side from the just-written `x`, not `x`'s pre-edge
/// (previous-cycle) value — `x`'s reg-init (`0`) is deliberately different
/// from what `x = a;` computes this edge (`1`), so a reader that only sees
/// a blanket pre-call snapshot gets the wrong answer.
#[test]
fn blocking_write_is_visible_to_a_later_nonblocking_read_in_the_same_block() {
    let mut m = LoweredBody::new("mixed_block");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let a = m.symbols.add_node("a", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    let x = m.symbols.add_var("x", Type::Quad);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.ports.push(Port::scalar(a, Direction::In));
    m.ports.push(Port::scalar(y, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![clk, a],
        outputs: vec![y],
        regs: vec![x],
        reg_inits: vec![zero_init(x)],
        stmts: vec![posedge(
            "clk",
            block(vec![assign("x", ident("a")), nonblocking("y", ident("x"))]),
        )],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile mixed_block"));
    let (clk_n, a_n, y_n) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance =
        DigitalInstance::new(kernel, 0, vec![clk_n, a_n], vec![y_n], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);
    bench.set(clk_n, LogicValue::Zero);
    bench.set(a_n, LogicValue::One);
    bench.step(0.0, &mut instance);

    bench.set(clk_n, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(
        bench.nets[y_n.0],
        LogicValue::One,
        "y <= x reads x AFTER `x = a;` ran this edge (1), not x's pre-edge reg-init value (0)"
    );
}

/// Done-when: "the existing clocked idiom `q <= q + 1` ... still passes" —
/// a self-referencing non-blocking increment still increments exactly once
/// per edge.
#[test]
fn nonblocking_self_increment_still_increments_once_per_edge() {
    let mut m = LoweredBody::new("nba_counter");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let cnt = m.symbols.add_var("cnt", Type::Integer);
    m.ports.push(Port::scalar(clk, Direction::In));
    m.digital = Some(DigitalBody {
        inputs: vec![clk],
        outputs: vec![],
        regs: vec![cnt],
        reg_inits: vec![zero_init(cnt)],
        stmts: vec![posedge(
            "clk",
            block(vec![nonblocking(
                "cnt",
                Expr::Binary {
                    span: SP,
                    op: BinOp::Add,
                    lhs: Box::new(ident("cnt")),
                    rhs: Box::new(Expr::Number { span: SP, value: NumberLit::Integer(1) }),
                },
            )]),
        )],
    output_drive_strength: Default::default() });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("compile nba_counter"));
    let clk_n = DigitalNet(0);
    let mut instance = DigitalInstance::new(kernel, 0, vec![clk_n], vec![], vec![]).expect("instance");

    let mut bench = Bench::new(1);
    bench.init(&mut instance);

    let cnt_value = |instance: &DigitalInstance| -> f64 {
        instance
            .export_vars()
            .expect("cnt is 0/1, not X/Z")
            .into_iter()
            .find(|(v, _)| *v == cnt)
            .map(|(_, value)| value)
            .expect("cnt is exported")
    };

    bench.set(clk_n, LogicValue::Zero);
    bench.step(0.0, &mut instance);
    assert_eq!(cnt_value(&instance), 0.0, "power-on reset value");

    for (i, expected) in [1.0, 2.0, 3.0].into_iter().enumerate() {
        let t = (2 * i) as f64;
        bench.set(clk_n, LogicValue::Zero);
        bench.step(t, &mut instance);
        bench.set(clk_n, LogicValue::One);
        bench.step(t + 1.0, &mut instance);
        assert_eq!(cnt_value(&instance), expected, "increments once per posedge");
    }
}
