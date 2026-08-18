//! Variable initializers (Annex A.2.2.1, `variable_type ::= variable_id
//! = constant_expression`): a `var` declared with `= expr` powers on at
//! that value. Digital registers seed from `reg_inits` (the kernel's
//! `RegInit` expressions evaluated against the instance's params); the
//! analog persistent-var bank seeds from the kernel's `var_inits` at
//! construction. Before this, registers powered on X/0 and the analog
//! bank always started at 0.0 — the LRM's initializer was silently
//! dropped (§4.2 of the session handoff, resolved here).

use piperine_codegen::SimCtx;
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::LogicValue;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const LOGIC: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline
"#;

/// DVC shape: a counter whose register powers on at 4, published
/// combinationally to `q` — the t = 0 digital state carries the init.
#[test]
fn a_digital_register_powers_on_at_its_initializer() {
    let src = format!(
        "{LOGIC}
    module counter (input logic clk, output logic q);
        integer count = 4;
        always @(posedge clk) count <= count + 1;
        always @(*) q = (count > 3);
    endmodule

    module top (input logic clk, output logic q);
        counter c1 (clk, q);
    endmodule
    "
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = piperine_codegen::CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");

    // No clock edges scheduled: only the power-on evaluation runs, so the
    // t = 0 digital state is exactly the register initializer's effect.
    let result = circuit
        .transient(TransientAnalysisOptions::new(1e-6, 1e-7), Context::default())
        .unwrap()
        .solve()
        .expect("tran completes");

    // q = (count > 3) with count = 4 at power-on. Without the initializer
    // the register powers on at 0/X and q would be 0 (or X) — the init is
    // what makes the assertion fire.
    let q_idx = *info.digital_nets.get("q").expect("net `q`");
    let first = result.get(0).expect("t=0 step");
    assert_eq!(first.digital(q_idx), Some(LogicValue::One), "q = (count > 3) with count = 4 at power-on");
}

/// The analog persistent-var bank seeds from the initializer, evaluated
/// against the instance's own params — proven through a margin that reads
/// the var at t = 0: `x` powers on at `hi` (2.0), so `require r : x >=
/// 1.5` holds; an unseeded bank (0.0) would violate it.
#[test]
fn an_analog_var_bank_seeds_from_its_initializer_per_instance() {
    let src = r#"`include "disciplines.vams"
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 2.0;
    real x = hi;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require seeded : x >= 1.5;
    end
endmodule
"#;
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    let design = parse_and_elaborate(src, &map).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");

    // At the kernel level: eval the margin with a zeroed vars bank
    // *after* seeding through the same path the instance constructor
    // uses — direct proof the init expression evaluates per-instance
    // (against the params the caller passes).
    let kernel = AnalogKernel::compile(&bodies["Dut"]).expect("compiles");
    assert_eq!(kernel.num_margins(), 1);
    let (slot, expr) = kernel.var_inits().first().expect("one var init").clone();
    let seeded = piperine_codegen::resolve::pom_eval_const(&expr, &|name| {
        (name == "hi").then_some(2.0)
    })
    .expect("const-evaluates");
    assert_eq!(seeded, 2.0, "the init reads the param");
    let mut vars = vec![0.0; kernel.num_vars()];
    vars[slot] = seeded;
    let state = vec![0.0; kernel.num_state_slots()];
    let volts = [0.0, 0.0];
    let m = kernel.eval_margin(0, &volts, &[2.0], &state, &vars, &SimCtx::default());
    assert!(m >= 0.0, "margin with the seeded bank: {m}");
    let unseeded = kernel.eval_margin(0, &volts, &[2.0], &state, &vec![0.0; kernel.num_vars()], &SimCtx::default());
    assert!(unseeded < 0.0, "the same margin without seeding violates: {unseeded} — the init is load-bearing");
}
