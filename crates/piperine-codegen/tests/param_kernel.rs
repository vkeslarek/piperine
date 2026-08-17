//! The `∂F/∂p` kernel (dv-gradients T2, DVG-02): ONE parameter-indexed JIT
//! function per module, regardless of parameter count — the design that keeps
//! a ~60-parameter model card off the Cranelift function-count wall `.disto`
//! hits on MOS2/MOS3 (`TryFromIntError`). The function takes the parameter
//! index as its 7th argument and dispatches internally.
//!
//! Lives in its own test binary so [`AnalogKernel::compile_count`] deltas are
//! not polluted by concurrent tests in the same process.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;


/// Compile the two-terminal module `Dut` (params `params`, body `body`).
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
    kernel.param_names().iter().position(|n| n == name).unwrap_or_else(|| {
        panic!("no param `{name}` among {:?}", kernel.param_names())
    })
}

// ── one function, parameter-indexed ──────────────────────────────────────────

#[test]
fn one_param_indexed_function_regardless_of_parameter_count() {
    // Two parameters; the kernel is ONE function that, indexed by the param,
    // yields the right derivative for each — proving it is parameter-indexed
    // (one function), not parameter-specialized (two).
    let kernel = kernel(
        "parameter real g = 2e-3; parameter real k = 0.5;",
        "I(p, n) <+ g * V(p, n) + k;",
    );
    assert!(kernel.has_param_jacobian(), "a body with Real params emits the kernel");
    // The count is asserted numerically — not inspected by eye. Two differentiable
    // parameters, but exactly one function.
    assert_eq!(
        kernel.num_param_jacobian_functions(),
        1,
        "one ∂F/∂p function regardless of parameter count (got the .disto wall otherwise)"
    );

    let g = idx_of(&kernel, "g");
    let k = idx_of(&kernel, "k");
    let sim = SimCtx::default();
    let volts = [0.5, 0.0];
    let params = [2e-3, 0.5];
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];

    // d/dg (g·V + k) = V(p,n) = 0.5; d/dk = 1.0 — same function, different index.
    let mut out_g = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(g, &volts, &params, &state, &vars, &sim, &mut out_g);
    assert!((out_g[0] - 0.5).abs() < 1e-12, "d/dg = V = 0.5, got {}", out_g[0]);

    let mut out_k = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(k, &volts, &params, &state, &vars, &sim, &mut out_k);
    assert!((out_k[0] - 1.0).abs() < 1e-12, "d/dk = 1.0, got {}", out_k[0]);
}

// ── the `.disto` wall: a ~60-parameter model card compiles into ONE function ─

#[test]
fn a_mos1_sized_model_card_compiles_into_one_function() {
    // A Mos1 model card carries ~60 parameters. A per-parameter kernel design
    // (one function per parameter) walks into `.disto`'s wall
    // (`TryFromIntError` on MOS2/MOS3). The parameter-indexed design compiles
    // all 60 as dispatch blocks inside a single function.
    let mut params = String::new();
    for i in 0..60 {
        params.push_str(&format!("parameter real p{i} = 1.0; "));
    }
    // The body uses a handful; the rest still get zero-derivative dispatch
    // blocks (one function, sixty blocks).
    let body = "I(p, n) <+ (p0 + p1 * V(p, n) + p2 * V(p, n) * V(p, n));";

    let kernel = kernel(&params, body);
    assert_eq!(
        kernel.num_param_jacobian_functions(),
        1,
        "a 60-parameter model card compiles exactly one ∂F/∂p function"
    );
    // 60 Real parameters → 60 dispatch targets, all in one function.
    assert_eq!(
        kernel.param_jacobian_indices().len(),
        60,
        "all 60 Real params are dispatch targets"
    );

    // Smoke value check on the indexed dispatch: d/d(p1) of (p0 + p1·V + p2·V²)
    // is V(p,n).
    let p1 = idx_of(&kernel, "p1");
    let sim = SimCtx::default();
    let volts = [0.3, 0.0];
    let params: Vec<f64> = (0..60).map(|i| 1.0 + i as f64 * 0.01).collect();
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];
    let mut out = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(p1, &volts, &params, &state, &vars, &sim, &mut out);
    assert!((out[0] - 0.3).abs() < 1e-12, "d/d(p1) = V = 0.3, got {}", out[0]);
}

// ── emitted once at compile time; a `set` does not re-emit ───────────────────

#[test]
fn param_jacobian_is_emitted_once_and_not_reemitted_by_a_set_loop() {
    // Compiling the module (which includes the ∂F/∂p kernel) is one compile;
    // subsequent parameter writes on an instance never re-emit (MD-18). The
    // kernel lives on the per-module AnalogKernel, Arc-shared across instances
    // — a `set` mutates instance param storage, not the compiled code.
    let src = format!(
        "module R(inout electrical p, inout electrical n);
    parameter real g = 1e-3;
    analog I(p, n) <+ g * V(p, n);
endmodule
module Top();
    electrical gnd, a;
    R r (.p(a), .n(gnd));
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");

    let before = AnalogKernel::compile_count();
    let mut compiler = piperine_codegen::CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");
    let one_compile = AnalogKernel::compile_count() - before;
    assert!(one_compile >= 1, "the module compiled at least once");

    // A `set` loop on the built circuit: restamps only, never recompiles. The
    // ∂F/∂p function is already emitted; the count must not move.
    for _ in 0..5 {
        let _ = circuit.set_element_param("r", "g", piperine_solver::abi::Value::Real(2e-3));
    }
    assert_eq!(
        AnalogKernel::compile_count() - before,
        one_compile,
        "a set loop re-emits nothing (the ∂F/∂p kernel was emitted once at compile time)"
    );
}

// ── cross-check ∂F/∂p against finite differences of the residual ─────────────
//
// The spec's oracle for DC gradients is central finite difference. The kernel's
// per-contribution derivative, stamped ± at (plus, minus), must reproduce the
// FD sensitivity of the residual vector w.r.t. the parameter — for a nonlinear
// device, not only a linear one.

#[test]
fn param_jacobian_matches_finite_difference_of_the_residual() {
    let kernel = kernel(
        "parameter real is = 1e-14;",
        "I(p, n) <+ is * (exp(V(p, n) / $vt) - 1.0);",
    );
    let is = idx_of(&kernel, "is");
    let sim = SimCtx::at_temperature(300.0);
    let vt = 300.0 * SimCtx::K_B_OVER_Q;
    let volts = [0.65, 0.0];
    let params = [1e-14];
    let state = vec![0.0; kernel.num_state_slots()];
    let vars = vec![0.0; kernel.num_vars()];

    // Analytic ∂F/∂is: d/d(is) of is·(exp(V/vt) − 1) = (exp(V/vt) − 1), per
    // contribution; stamped + at plus, − at minus.
    let mut out = vec![0.0; kernel.param_jacobian_contribs().len()];
    kernel.eval_param_jacobian(is, &volts, &params, &state, &vars, &sim, &mut out);
    let did = out[0];
    let expected = (volts[0] / vt).exp() - 1.0;
    assert!((did - expected).abs() < 1e-9 * expected.abs(), "analytic d(I)/d(is) = {expected}, got {did}");

    // Finite-difference the residual at plus/minus and confirm ±did.
    let res = |is_val: f64| -> [f64; 2] {
        let p = [is_val];
        let mut r = [0.0; 2];
        kernel.eval_residual(&volts, &p, &state, &vars, &sim, &mut r);
        r
    };
    let h = 1e-9;
    let rp = res(params[0] + h);
    let rm = res(params[0] - h);
    let fd_plus = (rp[0] - rm[0]) / (2.0 * h);
    let fd_minus = (rp[1] - rm[1]) / (2.0 * h);
    assert!((fd_plus - did).abs() < 1e-6 * did.abs(), "d(res[plus])/d(is) = +{did}, fd = {fd_plus}");
    assert!((fd_minus + did).abs() < 1e-6 * did.abs(), "d(res[minus])/d(is) = −{did}, fd = {fd_minus}");
}
