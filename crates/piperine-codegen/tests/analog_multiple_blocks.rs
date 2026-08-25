//! LRM-COVERS: 6.9.1, 6.9, 5.6.1.2, 5.6.1.3
//!
//! T69 (`lrm-conformance-wave2`) re-reading, no new tests: §6.9
//! "Elaboration" is the containing narrative heading for §6.9.1 (already
//! this file's own subject) with no independent rule of its own.
//! `two_analog_blocks_sum_their_contributions` is also §5.6.1.3's own
//! "Value retention" rule ("multiple contributions to the same ... branch
//! will be additive") and §5.6.1.2's own three-step "Evaluation" (RHS
//! evaluated, added to any previously-retained value, then assigned at
//! cycle end) — the same additive-accumulation mechanism both clauses
//! describe, exercised here across two concatenated analog blocks.
//! LRM §6.9.1 "Concatenation of analog blocks": a module with more than one
//! `analog` block has them combined into a single block, in source order —
//! "the analog blocks shall execute in the order that they are specified in
//! the module." `resolve/pom/body.rs`'s per-behavior loop used to assign
//! `bodies[i].analog = Some(AnalogBody { .. })` unconditionally, so only the
//! *last* `analog` block of a module survived lowering and every earlier one
//! vanished with no diagnostic — SPEC_DEVIATION.md finding 1.1, re-verified
//! at HEAD before this fix (the digital arm right above it already appends;
//! the analog arm did not).
//!
//! These tests drive the fixed lowering through `AnalogKernel::eval_residual`
//! (the same technique `analog_kernel.rs`/`analog_device_numerics.rs` use) —
//! a residual read is a genuine simulate-level observation, not a
//! shape-only inspection of the lowered `AnalogBody`.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

/// Compile the two-terminal module `Dut` whose full analog behavior section
/// is `analog_src` (verbatim — callers supply their own `analog begin … end`
/// blocks so more than one can be declared).
fn kernel(analog_src: &str) -> AnalogKernel {
    let src = format!(
        "module Dut(inout electrical a, inout electrical b);
    real x;
    {analog_src}
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaborate failed: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).expect("kernel compiles")
}

/// Residual of a compiled two-terminal kernel at `volts`.
fn residual(k: &AnalogKernel, volts: [f64; 2]) -> [f64; 2] {
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = [0.0; 2];
    k.eval_residual(&volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

/// AC4: two `analog` blocks, each contributing a constant current to the
/// same branch, sum at the operating point — the fixed lowering must not
/// drop either one.
#[test]
fn two_analog_blocks_sum_their_contributions() {
    let k = kernel(
        "
    analog begin
        I(a, b) <+ 1.0e-3;
    end
    analog begin
        I(a, b) <+ 2.0e-3;
    end
    ",
    );
    let out = residual(&k, [1.0, 0.0]);
    assert!(
        (out[0] - 3.0e-3).abs() < 1e-12,
        "rhs[a] must be the SUM of both blocks' contributions (1e-3 + 2e-3 = 3e-3), got {}",
        out[0]
    );
    assert!(
        (out[1] - -3.0e-3).abs() < 1e-12,
        "rhs[b] must be the negated sum, got {}",
        out[1]
    );
}

/// AC4: swapping the order of two *independent* constant-current blocks
/// changes nothing at the operating point.
#[test]
fn swapping_analog_block_order_does_not_change_the_result() {
    let forward = kernel(
        "
    analog begin
        I(a, b) <+ 1.0e-3;
    end
    analog begin
        I(a, b) <+ 2.0e-3;
    end
    ",
    );
    let reversed = kernel(
        "
    analog begin
        I(a, b) <+ 2.0e-3;
    end
    analog begin
        I(a, b) <+ 1.0e-3;
    end
    ",
    );
    let out_fwd = residual(&forward, [1.0, 0.0]);
    let out_rev = residual(&reversed, [1.0, 0.0]);
    assert!(
        (out_fwd[0] - out_rev[0]).abs() < 1e-12,
        "block order must not affect the summed residual: forward={}, reversed={}",
        out_fwd[0],
        out_rev[0]
    );
    assert!((out_fwd[0] - 3.0e-3).abs() < 1e-12, "sum itself must still be 3e-3, got {}", out_fwd[0]);
}

/// Statement order across concatenated blocks (LRM §6.9.1's "execute in the
/// order specified"): a module-level variable set in block 1 must be visible
/// when block 2 reads it. If concatenation dropped block order (or dropped
/// block 1 altogether, as the pre-fix bug did), `x` would read back as its
/// zero default instead of 5.0.
#[test]
fn variable_set_in_first_block_is_visible_in_second_block() {
    let k = kernel(
        "
    analog begin
        x = 5.0;
    end
    analog begin
        I(a, b) <+ x * 1.0e-3;
    end
    ",
    );
    let out = residual(&k, [1.0, 0.0]);
    assert!(
        (out[0] - 5.0e-3).abs() < 1e-12,
        "block 2 must see x=5.0 set by block 1 (rhs[a] = 5e-3); a dropped or \
         misordered block 1 would read x=0.0 instead, got rhs[a] = {}",
        out[0]
    );
}

/// Regression: a module with only ONE `analog` block still compiles and
/// behaves as before (the `None` arm of the merge match).
#[test]
fn single_analog_block_regression() {
    let k = kernel(
        "
    analog begin
        I(a, b) <+ 1.0e-3;
    end
    ",
    );
    let out = residual(&k, [1.0, 0.0]);
    assert!((out[0] - 1.0e-3).abs() < 1e-12, "single block unaffected by the merge fix, got {}", out[0]);
}
