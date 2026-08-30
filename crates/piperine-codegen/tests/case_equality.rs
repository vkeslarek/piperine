//! `===`/`!==` are four-state case equality, distinct from `==`/`!=` (LRM
//! §4.2.6, T24/LRMW2-18) — end to end through the digital JIT: unlike weak
//! equality (which poisons to X whenever either operand is X/Z), case
//! equality never returns X and treats X/Z as ordinary comparable bits.
//!
//! LRM-COVERS: 4.2.6

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_lang::ast::{BinOp, Expr, Stmt};
use piperine_lang::error::PreprocessedSpan;

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::*;
use piperine_codegen::DigitalKernel;
use piperine_solver::abi::DigitalEvent;
use piperine_solver::prelude::{DigitalNet, LogicValue};

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

fn assign(dest: &str, value: Expr) -> Stmt {
    Stmt::Assign { span: SP, target: ident(dest), value }
}

/// `y = a <op> b;` — a single combinational statement comparing two quad
/// inputs with the given operator.
fn compare_module(name: &str, op: BinOp) -> LoweredBody {
    let mut m = LoweredBody::new(name);
    let a = m.symbols.add_node("a", Domain::Digital);
    let b = m.symbols.add_node("b", Domain::Digital);
    let y = m.symbols.add_node("y", Domain::Digital);
    m.ports.push(Port::scalar(a, Direction::In));
    m.ports.push(Port::scalar(b, Direction::In));
    m.ports.push(Port::scalar(y, Direction::Out));
    m.digital = Some(DigitalBody {
        inputs: vec![a, b],
        outputs: vec![y],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![assign(
            "y",
            Expr::Binary { span: SP, op, lhs: Box::new(ident("a")), rhs: Box::new(ident("b")) },
        )],
    output_drive_strength: Default::default() });
    m
}

/// Drives one `DigitalInstance` directly, same style as `digital_jit.rs`.
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

/// Spec's own worked example: `1'bx === 1'bx` is true; `1'bx === 1'b1` is
/// false (never X) — case equality never poisons.
#[test]
fn case_equality_treats_x_as_an_ordinary_comparable_bit() {
    let module = compare_module("case_eq", BinOp::CaseEq);
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile case_eq"));
    let (a, b, y) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance = DigitalInstance::new(kernel, 0, vec![a, b], vec![y], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);

    bench.set(a, LogicValue::X);
    bench.set(b, LogicValue::X);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::One, "1'bx === 1'bx is true (spec AC, LRM §4.2.6)");

    bench.set(a, LogicValue::X);
    bench.set(b, LogicValue::One);
    bench.step(1.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::Zero, "1'bx === 1'b1 is false, not X");
}

/// The contrast case the spec AC requires: `1'bx == 1'bx` is falsy (X),
/// discriminating ordinary weak equality from case equality.
#[test]
fn ordinary_equality_is_poisoned_to_x_by_an_unknown_operand() {
    let module = compare_module("weak_eq", BinOp::Eq);
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile weak_eq"));
    let (a, b, y) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance = DigitalInstance::new(kernel, 0, vec![a, b], vec![y], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);

    bench.set(a, LogicValue::X);
    bench.set(b, LogicValue::X);
    bench.step(0.0, &mut instance);
    assert_eq!(
        bench.nets[y.0],
        LogicValue::X,
        "1'bx == 1'bx is X (weak equality poisons), not 1 — discriminates from `===`"
    );
}

/// `!==` mirrors `===`: never X, and distinguishes `Z` from `X` (an exact
/// bit-pattern comparison, unlike `==`/`!=`'s normalize-Z-to-X weak
/// equality).
#[test]
fn case_inequality_mirrors_case_equality() {
    let module = compare_module("case_ne", BinOp::CaseNe);
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile case_ne"));
    let (a, b, y) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance = DigitalInstance::new(kernel, 0, vec![a, b], vec![y], vec![]).expect("instance");

    let mut bench = Bench::new(3);
    bench.init(&mut instance);

    bench.set(a, LogicValue::X);
    bench.set(b, LogicValue::X);
    bench.step(0.0, &mut instance);
    assert_eq!(bench.nets[y.0], LogicValue::Zero, "1'bx !== 1'bx is false");

    bench.set(a, LogicValue::Z);
    bench.set(b, LogicValue::X);
    bench.step(1.0, &mut instance);
    assert_eq!(
        bench.nets[y.0],
        LogicValue::One,
        "1'bz !== 1'bx is true — Z and X are distinct bit patterns under case (in)equality"
    );
}
