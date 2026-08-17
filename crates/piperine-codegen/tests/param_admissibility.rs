//! `∂F/∂p` admissibility and compile-once (dv-gradients T3, DVG-03): only
//! `Real`-typed restamp-class parameters are analytically differentiable;
//! rebuild-class (presence-queried) and discrete parameters emit no
//! derivative, a circuit with none emits no kernel at all (zero cost), and the
//! compile count is stable across a `set` loop (MD-18).
//!
//! The loud refusal itself is the driver's job (T6) — it reads
//! [`AnalogKernel::param_jacobian_indices`] and refuses any index absent from
//! it. This test pins the codegen half: which params that slice contains, and
//! that an excluded index emits nothing.
//!
//! Own binary so [`AnalogKernel::compile_count`] deltas are not polluted by
//! concurrent tests in the same process.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;


fn kernel(params: &str, body: &str) -> AnalogKernel {
    let src = format!(
        "module Dut(inout electrical p, inout electrical n);
    {params}
    analog begin {body} end
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"]).expect("compile")
}

fn idx_of(kernel: &AnalogKernel, name: &str) -> usize {
    kernel.param_names().iter().position(|n| n == name).unwrap()
}

// ── discrete parameters are not differentiable ───────────────────────────────

#[test]
fn a_discrete_parameter_emits_no_derivative() {
    // `n` is Integer; `g` is Real. Only `g` is analytic; `n` finds no block.
    let kernel = kernel(
        "parameter real g = 1e-3; parameter integer nk = 3;",
        "I(p, n) <+ g * V(p, n);",
    );
    let g = idx_of(&kernel, "g");
    let n = idx_of(&kernel, "nk");
    assert!(kernel.has_param_jacobian(), "g is differentiable → kernel emitted");
    let admissible: Vec<usize> = kernel.param_jacobian_indices().to_vec();
    assert!(admissible.contains(&g), "g (Real) is admissible");
    assert!(
        !admissible.contains(&n),
        "n (Integer) is not admissible — same rule as `.sens`"
    );

    // A request for `n` emits nothing: the default dispatch block writes zero.
    let sim = piperine_codegen::SimCtx::default();
    let volts = [0.5, 0.0];
    let params = [1e-3, 3.0];
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];
    let mut out = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(n, &volts, &params, &state, &vars, &sim, &mut out);
    assert!(out.iter().all(|&v| v == 0.0), "discrete param emits no derivative (all zero)");
}

// ── rebuild-class parameters are not differentiable ──────────────────────────

#[test]
fn a_rebuild_class_parameter_emits_no_derivative() {
    // `p` is an optional Real (`Real?` + `get_or` → presence-queried), hence
    // rebuild-class for an instance that leaves it absent; the module-level
    // kernel excludes it. `g` is plain Real restamp-class, so the kernel still
    // emits (for `g`), just not `p`.
    let kernel = kernel(
        "parameter real popt = 0.0; parameter real g = 1e-3;",
        "I(p, n) <+ ($param_given(\"popt\") ? popt : 0.0) * V(p, n) + g * V(p, n);",
    );
    let p = idx_of(&kernel, "popt");
    let g = idx_of(&kernel, "g");
    assert!(kernel.has_param_jacobian(), "g is differentiable → kernel emitted");
    let admissible: Vec<usize> = kernel.param_jacobian_indices().to_vec();
    assert!(admissible.contains(&g), "g (Real, restamp) is admissible");
    assert!(
        !admissible.contains(&p),
        "p (presence-queried → rebuild-class) is not admissible — the driver fails loud on a request"
    );

    // A request for `p` emits nothing.
    let sim = piperine_codegen::SimCtx::default();
    let volts = [0.5, 0.0];
    let params = [0.0, 1e-3];
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];
    let mut out = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(p, &volts, &params, &state, &vars, &sim, &mut out);
    assert!(out.iter().all(|&v| v == 0.0), "rebuild-class param emits no derivative (all zero)");
}

// ── zero differentiable parameters → no kernel (zero cost) ───────────────────

#[test]
fn a_circuit_with_no_differentiable_params_emits_no_kernel() {
    // No parameters at all: a plain resistor. Nothing to differentiate.
    let resistor = kernel("", "I(p, n) <+ V(p, n) / 1e3;");
    assert!(
        !resistor.has_param_jacobian(),
        "no params → no ∂F/∂p kernel compiled (zero cost)"
    );
    assert_eq!(resistor.num_param_jacobian_functions(), 0);

    // Only a discrete param: still nothing analytic.
    let discrete_only = kernel("parameter integer nk = 3;", "I(p, n) <+ nk * V(p, n);");
    assert!(
        !discrete_only.has_param_jacobian(),
        "only an Integer param → no ∂F/∂p kernel"
    );

    // Only a presence-queried (rebuild-class) Real param: excluded → no kernel.
    let rebuild_only = kernel(
        "parameter real popt = 0.0;",
        "I(p, n) <+ ($param_given(\"popt\") ? popt : 0.0) * V(p, n);",
    );
    assert!(
        !rebuild_only.has_param_jacobian(),
        "only a rebuild-class param → no ∂F/∂p kernel"
    );
}

// ── compile-once: a set loop on a sensitivity-enabled circuit recompiles nothing ─

#[test]
fn compile_count_is_unchanged_across_a_set_loop_on_a_sensitivity_enabled_circuit() {
    // A circuit with several admissible parameters (the ∂F/∂p kernel is
    // emitted). Setting a restamp-class parameter restamps only — the kernel
    // is shared per module and never recompiled (MD-18).
    let src = format!(
        "module G(inout electrical p, inout electrical n);
    parameter real g = 1e-3;
    parameter real r = 1e3;
    analog I(p, n) <+ g * V(p, n) + V(p, n) / r;
endmodule
module Top();
    electrical gnd, a;
    G x (.p(a), .n(gnd));
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");

    let before = AnalogKernel::compile_count();
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");
    let compiled = AnalogKernel::compile_count() - before;
    assert!(compiled >= 1, "the sensitivity-enabled module compiled at least once");

    for v in [2e-3, 3e-3, 1e-3, 5e-3] {
        let _ = circuit.set_element_param("x", "g", piperine_solver::abi::Value::Real(v));
        let _ = circuit.set_element_param("x", "r", piperine_solver::abi::Value::Real(2e3));
    }
    assert_eq!(
        AnalogKernel::compile_count() - before,
        compiled,
        "a set loop on restamp params recompiles nothing — ∂F/∂p was emitted once at compile time"
    );
}
