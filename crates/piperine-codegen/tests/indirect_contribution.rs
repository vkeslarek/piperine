//! LRM-COVERS: 5.6.7
//!
//! LRMW2-41 (`lrm-conformance-wave2`, T46): `branch_lvalue : indirect_expression
//! == analog_expression;` (§5.6.7) — grammar + lowering. Root
//! `tests/indirect_contribution_opamp_e2e.rs` covers the AC15 simulate
//! proof (the LRM's own §5.6.7 ideal-opamp example, in a feedback
//! circuit). This file covers the flatten-stage scope limits disclosed at
//! the fix site (`flatten/analog.rs::add_indirect_contrib`): a flow-nature
//! (`I()`) target and a conditional indirect contribution both fail loud
//! by typed kind rather than silently mis-lowering.

use piperine_codegen::{CircuitCompiler, CodegenError};
use piperine_lang::parse_and_elaborate;

fn build(src: &str, top: &str) -> Result<(), CodegenError> {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit(top).map(|_| ())
}

/// §5.6.7's own opamp example must at least lower to a compiled circuit —
/// the full simulate proof lives in the root e2e test.
#[test]
fn opamp_indirect_form_compiles() {
    let src = "module opamp (inout electrical out, pin, nin);
        analog V(out):V(pin,nin) == 0;
    endmodule
    module top ();
        electrical gnd, vin, vout;
        opamp op1 (vout, vin, vout);
    endmodule";
    build(src, "top").expect("the LRM's own opamp form compiles");
}

/// Scope cut (disclosed at the fix site): a flow-nature (`I()`) indirect
/// target has no lowering yet — fails loud by typed kind, never a silent
/// mis-lowering.
#[test]
fn flow_nature_indirect_target_fails_loud_by_typed_kind() {
    let src = "module dut (inout electrical out, inout electrical ctrl);
        analog I(out):V(ctrl) == 0;
    endmodule
    module top ();
        electrical gnd, o, c;
        dut d1 (o, c);
    endmodule";
    let err = build(src, "top").expect_err("a flow-nature indirect target must fail to build");
    // `CircuitCompiler::build_circuit` wraps the flattener's own
    // `CodegenError::Unsupported` in `Invalid` (instance-path context) —
    // still a typed `CodegenError` variant, checked by kind + message.
    assert!(
        matches!(err, CodegenError::Invalid(ref msg) if msg.contains("flow-nature") && msg.contains("5.6.7")),
        "expected a typed Invalid error citing §5.6.7's flow-nature scope cut, got: {err:?}"
    );
}

/// Scope cut (disclosed at the fix site): a conditional indirect
/// contribution is rejected outright (§5.6.7 permits it only when the
/// condition is a compile-time constant, not yet distinguished here).
#[test]
fn conditional_indirect_contribution_fails_loud_by_typed_kind() {
    let src = "module dut (inout electrical out, pin, nin, ctrl);
        parameter real thresh = 0;
        analog begin
            if (V(ctrl) > thresh)
                V(out):V(pin,nin) == 0;
        end
    endmodule
    module top ();
        electrical gnd, o, p, n, c;
        dut d1 (o, p, n, c);
    endmodule";
    let err = build(src, "top").expect_err("a conditional indirect contribution must fail to build");
    assert!(
        matches!(err, CodegenError::Invalid(ref msg) if msg.contains("conditional") && msg.contains("5.6.7")),
        "expected a typed Invalid error citing §5.6.7's conditional restriction, got: {err:?}"
    );
}
