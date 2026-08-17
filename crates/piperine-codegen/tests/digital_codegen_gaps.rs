//! P3b PB-07: the real↔4-state (`Quad`) coercion fail point in digital
//! codegen that analog codegen already lowers.
//!
//! PB-07 uses hand-built `LoweredBody` fixtures (matching `digital_jit.rs`'s
//! style): PB-07 is specifically the `emit/builder.rs::coerce` conversion, not
//! the frontend's type-inference surface for casts — hand-building the
//! `LoweredBody` calls `coerce` directly, the same way every other
//! `digital_jit.rs` test isolates the emission layer from elaboration.
//!
//! PB-05 (user-`fn` inlining in a digital body) and PB-06 (enum-pattern
//! `match`) were the other two fail points this target covered. Both were
//! driven from PHDL source and neither has a Verilog-AMS retarget today:
//! VAMS `function`/`analog function` declarations are not lowered into the
//! POM at all (`resolve/pom/body.rs` Pass 1.5, MD-45 — `fn_by_name` is a
//! permanently empty landing spot), and PHDL's `enum` layer was deleted
//! outright with no LRM counterpart. They are removed rather than rewritten
//! against a frontend that cannot produce them.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::DigitalKernel;
use piperine_lang::ast::{BinOp, Expr, Stmt};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::NumberLit;
use piperine_solver::abi::DigitalEvent;
use piperine_solver::prelude::{DigitalNet, LogicValue};

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

fn real(value: f64) -> Expr {
    Expr::Number { span: SP, value: NumberLit::Real(value) }
}

fn assign(dest: &str, value: Expr) -> Stmt {
    Stmt::Assign { span: SP, target: ident(dest), value }
}

/// Minimal single-instance combinational bench: set inputs, step once, read
/// outputs — no clock, no event queue draining beyond init.
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

// ── PB-07: real↔4-state (`Quad`) coercion in a digital body ─────────────────

/// `r = a + 0.0` (a `Quad` net binary-added to a `Real` literal, forcing
/// `coerce_pair` to promote the net into `Real`) then `z = r` (storing a
/// `Real` var into a `Quad` output net, forcing `store_net`'s `coerce(_,
/// DigTy::Quad)`) — both directions of PB-07's fix, in one pass, driven
/// entirely by `coerce`/`coerce_pair`, not `CodegenError::Unsupported`
/// (AC3). `a`'s three possible net values (0/1/X) each produce a
/// spec-checkable `z`: `Quad -> Real` collapses X to `0` (existing 2-state
/// projection reused unchanged, see PB-07 in `emit/builder.rs::coerce`), so
/// `a=0` and `a=X` both round-trip to `z=0`, `a=1` round-trips to `z=1`.
#[test]
fn digital_quad_real_coercion_both_directions() {
    use piperine_codegen::resolve::{DigitalBody, Domain, Type};

    let mut m = LoweredBody::new("coerce_both");
    let a = m.symbols.add_node("a", Domain::Digital);
    let z = m.symbols.add_node("z", Domain::Digital);
    let _r = m.symbols.add_var("r", Type::Real);
    m.ports.push(piperine_codegen::resolve::Port {
        node: a,
        direction: piperine_codegen::resolve::Direction::In,
    });
    m.ports.push(piperine_codegen::resolve::Port {
        node: z,
        direction: piperine_codegen::resolve::Direction::Out,
    });
    m.digital = Some(DigitalBody {
        inputs: vec![a],
        outputs: vec![z],
        regs: vec![],
        reg_inits: vec![],
        stmts: vec![
            assign(
                "r",
                Expr::Binary {
                    span: SP,
                    op: BinOp::Add,
                    lhs: Box::new(ident("a")),
                    rhs: Box::new(real(0.0)),
                },
            ),
            assign("z", ident("r")),
        ],
    });
    let kernel = std::sync::Arc::new(
        DigitalKernel::compile(&m).expect("PB-07: coerce must compile, not Unsupported"),
    );
    let (a_n, z_n) = (DigitalNet(0), DigitalNet(1));
    let mut instance = DigitalInstance::new(kernel, 0, vec![a_n], vec![z_n], vec![]).expect("instance");
    let mut bench = Bench::new(2);
    bench.init(&mut instance);

    for (input, expected, label) in [
        (LogicValue::One, LogicValue::One, "1 -> Real(1.0) -> Quad(1)"),
        (LogicValue::Zero, LogicValue::Zero, "0 -> Real(0.0) -> Quad(0)"),
        (LogicValue::X, LogicValue::Zero, "X collapses to 0 -> Real(0.0) -> Quad(0)"),
    ] {
        bench.set(a_n, input);
        bench.step(0.0, &mut instance);
        assert_eq!(bench.nets[z_n.0], expected, "{label}");
    }
}

/// `Real -> Quad` truthiness on a fractional value: `0.5` must map to `1`
/// (nonzero), proving the coercion checks the real value directly and does
/// not truncate toward zero first (the literal `Real -> Int -> Quad` route
/// the spec suggested would wrongly truncate `0.5` to Int `0`; see the
/// `Implementation note` at the fix site in `emit/builder.rs::coerce`).
#[test]
fn digital_real_to_quad_coercion_does_not_truncate_fractional_nonzero() {
    use piperine_codegen::resolve::{DigitalBody, Domain, Type};

    for (level, expected) in [(0.5_f64, LogicValue::One), (0.0_f64, LogicValue::Zero), (-0.25_f64, LogicValue::One)] {
        let mut m = LoweredBody::new("frac_coerce");
        let z = m.symbols.add_node("z", Domain::Digital);
        let _r = m.symbols.add_var("r", Type::Real);
        m.ports.push(piperine_codegen::resolve::Port {
            node: z,
            direction: piperine_codegen::resolve::Direction::Out,
        });
        m.digital = Some(DigitalBody {
            inputs: vec![],
            outputs: vec![z],
            regs: vec![],
            reg_inits: vec![],
            stmts: vec![assign("r", real(level)), assign("z", ident("r"))],
        });
        let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("PB-07: coerce must compile"));
        let z_n = DigitalNet(0);
        let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![z_n], vec![]).expect("instance");
        let mut bench = Bench::new(1);
        bench.init(&mut instance);
        bench.step(0.0, &mut instance);
        assert_eq!(bench.nets[z_n.0], expected, "level={level} -> Quad");
    }
}
