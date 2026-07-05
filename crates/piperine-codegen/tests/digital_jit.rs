//! Digital JIT tests: hand-built IR digital bodies compiled to native
//! kernels and driven through the event-driven device wrapper.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::ir::*;
use piperine_codegen::DigitalKernel;
use piperine_codegen::ir::DigitalEvent as IrEvent;
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};

/// `inverter`: `y = ~a` (combinational).
fn inverter() -> LoweredBody {
    let mut m = LoweredBody::new("inverter");
    let a = m.symbols.add_node("a", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    m.ports.push(Port { node: a, direction: Direction::In });
    m.ports.push(Port { node: y, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![a],
        outputs: vec![y],
        regs: vec![],
        stmts: vec![IrStmt::Assign {
            lval: Lval::Net(y),
            expr: IrExpr::Unary(UnOp::Not, Box::new(IrExpr::Net(a))),
        }],
    });
    m
}

/// `dff`: `q` follows `d` on `posedge clk`, reset value 0.
fn dff() -> LoweredBody {
    let mut m = LoweredBody::new("dff");
    let clk = m.symbols.add_node("clk", Domain::Digital);
    let d = m.symbols.add_node("d", Domain::Digital);
    let q = m.symbols.add_node("q", Domain::Digital);
    let r = m.symbols.add_var("r", Type::Quad);
    m.ports.push(Port { node: clk, direction: Direction::In });
    m.ports.push(Port { node: d, direction: Direction::In });
    m.ports.push(Port { node: q, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![clk, d],
        outputs: vec![q],
        regs: vec![r],
        stmts: vec![
            IrStmt::VarDecl { var: r, init: Some(IrExpr::Quad(0)) },
            IrStmt::ClockedBlock {
                event: IrEvent::Posedge(IrExpr::Net(clk)),
                body: vec![IrStmt::Assign { lval: Lval::Var(r), expr: IrExpr::Net(d) }],
            },
            IrStmt::Assign { lval: Lval::Net(q), expr: IrExpr::Var(r) },
        ],
    });
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
    m.ports.push(Port { node: clk, direction: Direction::In });
    m.ports.push(Port { node: d, direction: Direction::In });
    m.ports.push(Port { node: q, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![clk, d],
        outputs: vec![q],
        regs: vec![r1, r2],
        stmts: vec![
            IrStmt::VarDecl { var: r1, init: Some(IrExpr::Quad(0)) },
            IrStmt::VarDecl { var: r2, init: Some(IrExpr::Quad(0)) },
            IrStmt::ClockedBlock {
                event: IrEvent::Posedge(IrExpr::Net(clk)),
                body: vec![
                    IrStmt::Assign { lval: Lval::Var(r2), expr: IrExpr::Var(r1) },
                    IrStmt::Assign { lval: Lval::Var(r1), expr: IrExpr::Net(d) },
                ],
            },
            IrStmt::Assign { lval: Lval::Net(q), expr: IrExpr::Var(r2) },
        ],
    });
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
    // sel ? (match) — y = match a { 0 => 1, 1 => 0, _ => X } as an
    // explicit Match statement over quad values.
    let mut m = LoweredBody::new("mux_match");
    let a = m.symbols.add_node("a", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    m.ports.push(Port { node: a, direction: Direction::In });
    m.ports.push(Port { node: y, direction: Direction::Out });
    m.digital = Some(DigitalBody {
        inputs: vec![a],
        outputs: vec![y],
        regs: vec![],
        stmts: vec![IrStmt::Match {
            scrutinee: IrExpr::Net(a),
            arms: vec![
                (
                    Pattern::BitPattern(vec![Trit::Zero]),
                    vec![IrStmt::Assign { lval: Lval::Net(y), expr: IrExpr::Quad(1) }],
                ),
                (
                    Pattern::BitPattern(vec![Trit::One]),
                    vec![IrStmt::Assign { lval: Lval::Net(y), expr: IrExpr::Quad(0) }],
                ),
            ],
            default: vec![IrStmt::Assign { lval: Lval::Net(y), expr: IrExpr::Quad(2) }],
        }],
    });
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
