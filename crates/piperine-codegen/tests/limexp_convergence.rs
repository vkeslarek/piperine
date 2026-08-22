//! LRM §4.5.13: `limexp()` limits its output iteration-to-iteration — the
//! property that lets Newton's method stay finite on a stiff exponential
//! junction where plain `exp()` overflows (SPEC_DEVIATION.md 2.3 — pre-fix,
//! `limexp` was a bare `exp(min(x, 80))`, no iteration-to-iteration
//! limiting at all).
//!
//! Drives the compiled diode kernel's own `eval_residual`/`eval_jacobian`
//! through an **undamped** Newton walk on the classic resistor-loaded-diode
//! DC equation `g(V) = (Vin − V)/R − I_diode(V) = 0`, starting from a
//! deliberately bad initial guess. This isolates `limexp`'s own
//! contribution (no `$limit`, no solver-level homotopy/damping to
//! obscure it) directly at the codegen layer.
//!
//! LRM-COVERS: 4.5.13

use piperine_codegen::SimCtx;
use piperine_codegen::AnalogKernel;
use piperine_lang::parse_and_elaborate;
use std::sync::Arc;

const DIODE_SRC_TEMPLATE: &str = "
module Diode ( inout electrical p, inout electrical n );
    parameter real Is = 1.0e-14;
    parameter real vt = 0.02585;
    analog I(p, n) <+ Is * ({{FN}}(V(p, n) / vt) - 1.0);
endmodule
";

fn compile_diode(fn_name: &str) -> Arc<AnalogKernel> {
    let src = DIODE_SRC_TEMPLATE.replace("{{FN}}", fn_name);
    let prog = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("{fn_name} diode elaborates: {e}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed");
    let module = bodies.get("Diode").expect("Diode module lowered");
    let compiled = piperine_codegen::CompiledModule::compile(module).expect("compiles");
    compiled.analog().expect("analog body").clone()
}

/// Undamped Newton on `g(V) = (vin - V)/r - I_diode(V) = 0` — the
/// resistor-loaded-diode DC equation — starting from `v0`. Returns the walk
/// (one entry per iteration); a non-finite entry marks the point of
/// overflow. Advances the kernel's own `limexp` state (`eval_limexp_update`)
/// each iteration exactly as the device layer does, so `limexp`'s
/// iteration-to-iteration limiting is exercised for real, not bypassed.
fn undamped_newton_walk(kernel: &AnalogKernel, is: f64, vt: f64, vin: f64, r: f64, v0: f64, iters: usize) -> Vec<f64> {
    let params = [is, vt];
    let mut state = vec![0.0_f64; kernel.num_state_slots()];
    // Match `AnalogInstance::new`'s own limexp seeding (the -1.0 sentinel).
    let lx_base = kernel.limexp_base();
    let lx_n = kernel.num_limexp();
    for slot in &mut state[lx_base..lx_base + lx_n] {
        *slot = -1.0;
    }
    let mut v = v0;
    let mut walk = Vec::with_capacity(iters);
    for _ in 0..iters {
        if !v.is_finite() {
            walk.push(v);
            break;
        }
        let volts = [v, 0.0];
        let mut rhs = [0.0_f64; 2];
        let mut jac = [0.0_f64; 4];
        let sim = SimCtx::default();
        kernel.eval_residual(&volts, &params, &state, &[], &sim, &mut rhs);
        kernel.eval_jacobian(&volts, &params, &state, &[], &sim, &mut jac);

        let nlx = kernel.num_limexp();
        if nlx > 0 {
            let base = kernel.limexp_base();
            let mut ylim = vec![0.0_f64; nlx];
            kernel.eval_limexp_update(&volts, &params, &state, &[], &sim, &mut ylim);
            state[base..base + nlx].copy_from_slice(&ylim);
        }

        let g = (vin - v) / r - rhs[0];
        let g_prime = -1.0 / r - jac[0];
        v -= g / g_prime;
        walk.push(v);
    }
    walk
}

/// The discriminating property (spec AC13): the identical resistor-loaded
/// diode DC equation, walked by the same undamped Newton iteration from the
/// same bad initial guess, overflows to a non-finite voltage with plain
/// `exp()` but stays finite throughout with `limexp()`.
#[test]
fn limexp_keeps_undamped_newton_finite_where_plain_exp_overflows() {
    let (is, vt, vin, r, v0, iters) = (1.0e-14, 0.02585, 10.0, 1000.0, 25.0, 30);

    let exp_kernel = compile_diode("exp");
    let exp_walk = undamped_newton_walk(&exp_kernel, is, vt, vin, r, v0, iters);
    assert!(
        exp_walk.iter().any(|v| !v.is_finite()),
        "undamped Newton on a bare exp() diode from a bad guess must overflow \
         to a non-finite voltage: {exp_walk:?}"
    );

    let limexp_kernel = compile_diode("limexp");
    let limexp_walk = undamped_newton_walk(&limexp_kernel, is, vt, vin, r, v0, iters);
    assert!(
        limexp_walk.iter().all(|v| v.is_finite()),
        "undamped Newton on a limexp() diode must stay finite throughout: {limexp_walk:?}"
    );
}
