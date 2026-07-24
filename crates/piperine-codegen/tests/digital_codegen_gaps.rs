//! P3b PB-05/06/07: three digital-codegen `CodegenError::Unsupported` fail
//! points that analog codegen already lowers — user-`fn` inlining, enum-
//! pattern `match`, and real↔4-state (`Quad`) coercion.
//!
//! PB-05/PB-06 are PHDL-source-driven (through `parse_and_elaborate` →
//! `lower_bodies` → `DigitalKernel`) so each exercises the real pipeline.
//! PB-07 uses hand-built `LoweredBody` fixtures instead (matching
//! `digital_jit.rs`'s existing style): a *behavior-local-only* digital
//! `var` never gets a codegen `VarId` (only a module-level `var` does — a
//! separate, pre-existing gap, not part of this task), and PHDL's
//! overload-resolution typecheck for `Real::from`/`Quad::from` only infers
//! an identifier argument's type from `behavior.var_types` (behavior-local
//! declarations), never a module-level var — so a PHDL source string can't
//! cleanly drive a *dynamic* (non-literal) value through both directions
//! without tripping one limitation or the other. Neither limitation is a
//! PB-07 target (PB-07 is specifically the `emit/builder.rs::coerce`
//! conversion, not PHDL's type-inference surface for casts) — hand-building
//! the `LoweredBody` calls `coerce` directly, the same way every other
//! `digital_jit.rs` test isolates the emission layer from elaboration.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::{lower_bodies, LoweredBody};
use piperine_codegen::DigitalKernel;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::DigitalEvent;
use piperine_solver::prelude::{DigitalNet, LogicValue};

/// Elaborate + lower `src`, compile module `name`'s digital body.
fn compile_digital(src: &str, name: &str) -> std::sync::Arc<DigitalKernel> {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"));
    let bodies = lower_bodies(&design).expect("lowering failed");
    let body = bodies.get(name).unwrap_or_else(|| panic!("module `{name}` not found"));
    std::sync::Arc::new(DigitalKernel::compile(body).expect("digital compile failed"))
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

// ── PB-05: user-`fn` inlining in a digital body ─────────────────────────────

/// A user `fn` called from a digital body must inline and compute the right
/// value — not `CodegenError::Unsupported`. `nand2(a, b) = !(a & b)`,
/// checked against its truth table (AC1: "no `CodegenError::Unsupported`" +
/// a computed, spec-checkable value).
#[test]
fn digital_fn_call_inlines_and_computes_nand() {
    // Fn param names deliberately differ from the calling module's own port
    // names (`x`/`z` vs `a`/`b`) — a shared, pre-existing resolver quirk
    // (`resolve_stmts` resolves every fn body as if analog, so a param name
    // that collides with the *calling module's* own node name resolves to
    // that node instead of the local param binding; this is orthogonal to
    // PB-05's digital-inlining gap — the same collision affects analog fn
    // calls too — and out of this task's scope).
    let src = "\
discipline Bit { storage Boolean; }
fn nand2(x: Bit, z: Bit) -> Bit { return !(x && z); }
mod Nand(input a: Bit, input b: Bit, output y: Bit) { }
digital Nand { y <- nand2(a, b); }
";
    let kernel = compile_digital(src, "Nand");
    let (a, b, y) = (DigitalNet(0), DigitalNet(1), DigitalNet(2));
    let mut instance = DigitalInstance::new(kernel, 0, vec![a, b], vec![y], vec![]).expect("instance");
    let mut bench = Bench::new(3);
    bench.init(&mut instance);

    for (av, bv, expected) in [
        (LogicValue::Zero, LogicValue::Zero, LogicValue::One),
        (LogicValue::Zero, LogicValue::One, LogicValue::One),
        (LogicValue::One, LogicValue::Zero, LogicValue::One),
        (LogicValue::One, LogicValue::One, LogicValue::Zero),
    ] {
        bench.set(a, av);
        bench.set(b, bv);
        bench.step(0.0, &mut instance);
        assert_eq!(bench.nets[y.0], expected, "nand2({av:?}, {bv:?})");
    }
}

// ── PB-06: enum-pattern `match` in a digital body ───────────────────────────

/// A digital `match` on an enum-typed var resolves each variant to its
/// discriminant — the same variant the const evaluator would match at
/// elaboration time (AC2). One module per variant (module-level `var` init
/// is an elaboration constant), asserting the branch taken for each of the
/// three declared variants.
#[test]
fn digital_enum_pattern_match_selects_the_right_arm() {
    for (variant, expected) in [("Idle", LogicValue::Zero), ("Run", LogicValue::One), ("Done", LogicValue::One)] {
        let src = format!(
            "\
enum State {{ Idle, Run, Done }}
discipline Bit {{ storage Boolean; }}
mod M(output y: Bit) {{ var s: State = {variant}; var out: Bit = 0; }}
digital M {{
    match s {{
        Idle => {{ out = 0; }}
        Run => {{ out = 1; }}
        Done => {{ out = 1; }}
    }}
    y <- out;
}}
"
        );
        let kernel = compile_digital(&src, "M");
        let y = DigitalNet(0);
        let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![y], vec![]).expect("instance");
        let mut bench = Bench::new(1);
        bench.init(&mut instance);
        bench.step(0.0, &mut instance);
        assert_eq!(bench.nets[y.0], expected, "match arm for variant `{variant}`");
    }
}

// ── PB-07: real↔4-state (`Quad`) coercion in a digital body ─────────────────

/// `y = a + 0.0` (a `Quad` net binary-added to a `Real` literal, forcing
/// `coerce_pair` to promote the net into `Real`) then `z <- r` (storing a
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
    use piperine_lang::parse::ast::{BinaryOp, BindOp, Expr, Literal, Stmt};

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
        stmts: vec![
            Stmt::Bind {
                dest: Expr::Ident("r".into()),
                op: BindOp::Assign,
                src: Expr::Binary(
                    Box::new(Expr::Ident("a".into())),
                    BinaryOp::Add,
                    Box::new(Expr::Literal(Literal::Real(0.0))),
                ),
            },
            Stmt::Bind { dest: Expr::Ident("z".into()), op: BindOp::Force, src: Expr::Ident("r".into()) },
        ],
    });
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&m).expect("PB-07: coerce must compile, not Unsupported"));
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
/// `SPEC_DEVIATION` at the fix site in `emit/builder.rs::coerce`).
#[test]
fn digital_real_to_quad_coercion_does_not_truncate_fractional_nonzero() {
    use piperine_codegen::resolve::{DigitalBody, Domain, Type};
    use piperine_lang::parse::ast::{BindOp, Expr, Literal, Stmt};

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
            stmts: vec![
                Stmt::Bind {
                    dest: Expr::Ident("r".into()),
                    op: BindOp::Assign,
                    src: Expr::Literal(Literal::Real(level)),
                },
                Stmt::Bind { dest: Expr::Ident("z".into()), op: BindOp::Force, src: Expr::Ident("r".into()) },
            ],
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
