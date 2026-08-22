//! LRM-COVERS: 5.9, 5.9.3
//! LRMW2-20 (`lrm-conformance-wave2`, T32): analog `for`/`while`/`repeat`
//! with elaboration-constant bounds unroll into repeated statements
//! (`flatten/analog.rs::unroll_loop`, index substituted for `for` via
//! `subst_stmt`/`subst_expr`) instead of failing loud unconditionally —
//! SPEC_DEVIATION.md §2.7. Fixtures adapt §5.9.3's own worked example
//! (`for (k = 1; k <= width; k = k + 1) tmp = tmp + V(dt[k]);`): that
//! example indexes an *array* net (`electrical [1:width] dt;`) and a
//! `parameter`-typed bound (`width`) — array analog nets are a separate,
//! pre-existing gap (not this task's `Where`, see the `urc codegen gaps`
//! roadmap item), and a `parameter` bound is never a genuine elaboration
//! constant from this compiled-once-per-module-type kernel's own
//! perspective (a `parameter` differs per instance; codegen has no
//! per-instance environment at flatten time). This keeps the LRM's own
//! for-loop-summing-a-contribution shape and its literal genvar
//! mechanics, with a literal loop bound instead of the array/parameter
//! combination.
//!
//! Uses the same `AnalogKernel::eval_residual` technique as
//! `analog_multiple_blocks.rs` — a residual read is a genuine
//! simulate-level observation, not a shape-only inspection.
//!
//! The digital half ("digital loops follow the same rule", `emit/stmt.rs`
//! reusing the very same `flatten::analog::unroll_loop`) is proven at the
//! bottom of this file with hand-built `LoweredBody`s compiled through
//! `DigitalKernel`, the same style `digital_jit.rs`/`case_equality.rs` use.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

/// Compile the two-terminal module `Dut` whose full analog behavior
/// section is `analog_src` (verbatim).
fn kernel(analog_src: &str) -> Result<AnalogKernel, String> {
    let src = format!(
        "module Dut(inout electrical a, inout electrical b);
    integer k;
    real tmp;
    {analog_src}
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaborate failed: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).map_err(|e| e.to_string())
}

fn residual(k: &AnalogKernel, volts: [f64; 2]) -> [f64; 2] {
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = [0.0; 2];
    k.eval_residual(&volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

/// AC (T32 Done-when #1): the LRM's own §5.9.3 for-loop-summing shape —
/// `for (k = 0; k < 4; k = k + 1) tmp = tmp + k;` — sums `0+1+2+3 = 6`,
/// contributed as a constant current, and the kernel actually *compiles
/// and evaluates* (simulates), not merely elaborates.
#[test]
fn for_loop_with_constant_bounds_unrolls_and_sums() {
    let k = kernel(
        "
    analog begin
        tmp = 0.0;
        for (k = 0; k < 4; k = k + 1) begin
            tmp = tmp + k;
        end
        I(a, b) <+ tmp;
    end
    ",
    )
    .expect("constant-bound for loop must compile");
    let out = residual(&k, [1.0, 0.0]);
    assert!(
        (out[0] - 6.0).abs() < 1e-9,
        "0+1+2+3 = 6 must reach the contribution as a constant current, got rhs[a] = {}",
        out[0]
    );
    assert!((out[1] - -6.0).abs() < 1e-9, "rhs[b] must be the negated sum, got {}", out[1]);
}

/// Discriminating (AC14-style: two different bound values give two
/// different results — an ignored/unrolled-wrong loop can't pass both):
/// changing the upper bound from `< 4` to `< 6` changes the summed result
/// from 6 (0+1+2+3) to 15 (0+1+2+3+4+5).
#[test]
fn different_constant_bounds_give_different_sums() {
    let k4 = kernel(
        "
    analog begin
        tmp = 0.0;
        for (k = 0; k < 4; k = k + 1) begin
            tmp = tmp + k;
        end
        I(a, b) <+ tmp;
    end
    ",
    )
    .expect("compiles");
    let k6 = kernel(
        "
    analog begin
        tmp = 0.0;
        for (k = 0; k < 6; k = k + 1) begin
            tmp = tmp + k;
        end
        I(a, b) <+ tmp;
    end
    ",
    )
    .expect("compiles");
    let out4 = residual(&k4, [1.0, 0.0])[0];
    let out6 = residual(&k6, [1.0, 0.0])[0];
    assert!((out4 - 6.0).abs() < 1e-9, "bound<4: 0+1+2+3=6, got {out4}");
    assert!((out6 - 15.0).abs() < 1e-9, "bound<6: 0+1+2+3+4+5=15, got {out6}");
    assert!((out4 - out6).abs() > 1.0, "two different bounds must give two different results");
}

/// §5.9.1: `repeat(N) stmt` — no loop-index binding, body repeats
/// unchanged. `repeat(3) tmp = tmp + 2.0;` accumulates 6.0 (three
/// unrolled `tmp = tmp + 2.0;` copies, blocking-assign so each sees the
/// previous write).
#[test]
fn repeat_with_a_constant_count_unrolls() {
    let k = kernel(
        "
    analog begin
        tmp = 0.0;
        repeat (3) begin
            tmp = tmp + 2.0;
        end
        I(a, b) <+ tmp;
    end
    ",
    )
    .expect("constant repeat count must compile");
    let out = residual(&k, [1.0, 0.0])[0];
    assert!((out - 6.0).abs() < 1e-9, "repeat(3) of `tmp += 2.0` must sum to 6.0, got {out}");
}

/// AC (T32 Done-when #2): a `for` bound referencing a module `parameter`
/// is not an elaboration constant from codegen's own perspective (the
/// compiled-once-per-module-type kernel has no per-instance environment
/// at flatten time) — must fail loud citing §5.9.3, by the typed
/// `CodegenError::Unsupported` kind, never `is_err()`.
#[test]
fn a_non_constant_bound_fails_loud_citing_the_lrm_clause() {
    let src = "module Dut(inout electrical a, inout electrical b);
    parameter integer width = 4;
    integer k;
    real tmp;
    analog begin
        tmp = 0.0;
        for (k = 0; k < width; k = k + 1) begin
            tmp = tmp + k;
        end
        I(a, b) <+ tmp;
    end
endmodule
";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let err = match AnalogKernel::compile(&bodies["Dut"]) {
        Ok(_) => panic!("a parameter-dependent bound must fail loud"),
        Err(e) => e,
    };
    match err {
        piperine_codegen::CodegenError::Unsupported(msg) => {
            assert!(msg.contains("§5.9.3"), "error cites the LRM clause: {msg}");
        }
        other => panic!("expected CodegenErrorKind::Unsupported citing §5.9.3, got {other:?}"),
    }
}

/// AC (T32 Done-when #3): unroll count is bounded — an absurd/
/// non-terminating bound fails loud instead of exploding the JIT.
#[test]
fn an_absurdly_large_bound_fails_loud_instead_of_exploding() {
    let src = "module Dut(inout electrical a, inout electrical b);
    integer k;
    real tmp;
    analog begin
        tmp = 0.0;
        for (k = 0; k < 1000000; k = k + 1) begin
            tmp = tmp + k;
        end
        I(a, b) <+ tmp;
    end
endmodule
";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let err = match AnalogKernel::compile(&bodies["Dut"]) {
        Ok(_) => panic!("an absurd bound must fail loud, not explode the JIT"),
        Err(e) => e,
    };
    match err {
        piperine_codegen::CodegenError::Unsupported(msg) => {
            assert!(msg.contains("§5.9.3"), "error cites the LRM clause: {msg}");
        }
        other => panic!("expected CodegenErrorKind::Unsupported, got {other:?}"),
    }
}

// ── Digital half: "digital loops follow the same rule" ──────────────────────

mod digital {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    use piperine_lang::ast::{Expr, Stmt, UnOp};
    use piperine_lang::error::PreprocessedSpan;
    use piperine_lang::lexer::{BasedNumber, NumberBase, NumberLit};

    use piperine_codegen::device::DigitalInstance;
    use piperine_codegen::resolve::*;
    use piperine_codegen::{CodegenError, DigitalKernel};
    use piperine_solver::abi::DigitalEvent;
    use piperine_solver::prelude::{DigitalNet, LogicValue};

    const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

    fn ident(name: &str) -> Expr {
        Expr::Ident { span: SP, name: name.to_string() }
    }

    fn assign(dest: &str, value: Expr) -> Stmt {
        Stmt::Assign { span: SP, target: ident(dest), value }
    }

    /// A one-bit based literal — `1'b0`.
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

    /// `y = 1'b0; repeat(count) y = ~y;` — combinational, no inputs.
    fn repeat_toggle_module(name: &str, count: Expr) -> LoweredBody {
        let mut m = LoweredBody::new(name);
        let y = m.symbols.add_node("y", Domain::Digital);
        m.ports.push(Port { node: y, direction: Direction::Out });
        m.digital = Some(DigitalBody {
            inputs: vec![],
            outputs: vec![y],
            regs: vec![],
            reg_inits: vec![],
            stmts: vec![
                assign("y", bit("0")),
                Stmt::Repeat {
                    span: SP,
                    count,
                    body: Box::new(assign(
                        "y",
                        Expr::Unary { span: SP, op: UnOp::BitNot, operand: Box::new(ident("y")) },
                    )),
                },
            ],
        });
        m
    }

    fn int_lit(v: i64) -> Expr {
        Expr::Number { span: SP, value: NumberLit::Integer(v) }
    }

    /// Compiles `module`, drives one `DigitalInstance` through its initial
    /// comb evaluation (no inputs to change afterward), and returns the
    /// resulting value of the single output net `y`.
    fn eval_y(module: &LoweredBody) -> LogicValue {
        let kernel = std::sync::Arc::new(DigitalKernel::compile(module).expect("compile"));
        let y = DigitalNet(0);
        let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![y], vec![]).expect("instance");
        let mut nets = vec![LogicValue::X; 1];
        let mut queue: BinaryHeap<Reverse<DigitalEvent>> = BinaryHeap::new();
        instance.init(&mut queue);
        while let Some(Reverse(event)) = queue.pop() {
            nets[event.net.0] = event.value;
        }
        nets[y.0]
    }

    /// AC (digital half of T32 Done-when #1/#3): `repeat` with a constant
    /// count unrolls — an *odd* toggle count (3) flips `y` from 0 to 1.
    #[test]
    fn repeat_with_odd_constant_count_unrolls_and_flips() {
        let module = repeat_toggle_module("odd_repeat", int_lit(3));
        assert_eq!(eval_y(&module), LogicValue::One, "3 unrolled `~y` toggles from 0 must land on 1");
    }

    /// Discriminating pair (AC14-style): an *even* count (2) must leave
    /// `y` back at 0 — the two counts must give two different results, so
    /// a broken/ignored unroll can't pass both this and the odd case.
    #[test]
    fn repeat_with_even_constant_count_unrolls_and_settles() {
        let module = repeat_toggle_module("even_repeat", int_lit(2));
        assert_eq!(eval_y(&module), LogicValue::Zero, "2 unrolled `~y` toggles from 0 must land back on 0");
    }

    /// AC (digital half of Done-when #2): a non-constant count (an
    /// undefined identifier — codegen has no environment to resolve it
    /// from) fails loud citing §5.9.3, through the exact same
    /// `unroll_loop` the analog path uses (`emit/stmt.rs` calls
    /// `crate::flatten::analog::unroll_loop` directly) — proving the
    /// digital call site shares the same typed-error behavior, not a
    /// silently-diverged copy.
    #[test]
    fn a_non_constant_digital_repeat_count_fails_loud() {
        let module = repeat_toggle_module("non_const_repeat", ident("undefined_bound"));
        let err = match DigitalKernel::compile(&module) {
            Ok(_) => panic!("a non-constant repeat count must fail loud"),
            Err(e) => e,
        };
        match err {
            CodegenError::Unsupported(msg) => {
                assert!(msg.contains("§5.9.3"), "error cites the LRM clause: {msg}");
            }
            other => panic!("expected CodegenError::Unsupported citing §5.9.3, got {other:?}"),
        }
    }
}
