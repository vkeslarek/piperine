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

/// Evaluate the compiled kernel's residual at a fixed state (branch voltage
/// `v`, limexp state slots already seeded/updated as `state` prescribes) and
/// return `rhs[0]`, the diode branch's own contribution. Used by both the
/// analytic-Jacobian read and the finite-difference probe below so both see
/// exactly the same residual function.
fn eval_residual_at(kernel: &AnalogKernel, is: f64, vt: f64, state: &[f64], v: f64) -> f64 {
    let params = [is, vt];
    let volts = [v, 0.0];
    let mut rhs = [0.0_f64; 2];
    let sim = SimCtx::default();
    kernel.eval_residual(&volts, &params, state, &[], &sim, &mut rhs);
    rhs[0]
}

/// Regression for the surviving mutant found by the wave2 Verifier: reverting
/// `resolve/diff.rs`'s `name == "limexp"` dispatch guard (so `differentiate`
/// falls through to a hardcoded `0.0` for `limexp`, exactly as it did before
/// T19's fix) left the Newton-convergence test above green, because the
/// specific residual equation under test (`g_prime = -1/r - jac[0]`) never
/// blew up even with `jac[0] == 0` — a *different*, unrelated fixture
/// (`tests/host_limiting.rs`) was the only thing catching the regression.
///
/// This test instead asserts the analytic Jacobian value directly against a
/// central finite-difference estimate of the compiled residual — a zeroed
/// `d(limexp)/dx` fails this check immediately regardless of which residual
/// equation happens to be under test elsewhere.
#[test]
fn limexp_jacobian_matches_finite_difference() {
    let (is, vt) = (1.0e-14, 0.02585);
    let kernel = compile_diode("limexp");
    let params = [is, vt];

    let mut state = vec![0.0_f64; kernel.num_state_slots()];
    let lx_base = kernel.limexp_base();
    let lx_n = kernel.num_limexp();
    for slot in &mut state[lx_base..lx_base + lx_n] {
        *slot = -1.0;
    }

    // Probe at a handful of branch voltages that keep limexp() away from its
    // asymptotic clamp (V(p,n)/vt near 80) so the finite-difference estimate
    // is well-conditioned.
    for &v in &[0.0_f64, 0.2, 0.4, 0.6] {
        // Advance limexp's iteration-to-iteration state to this operating
        // point first, exactly as the undamped Newton walk above does, so
        // the analytic Jacobian and the FD probe agree on which iterate's
        // state they're differentiating around.
        let volts = [v, 0.0];
        let sim = SimCtx::default();
        if lx_n > 0 {
            let mut ylim = vec![0.0_f64; lx_n];
            kernel.eval_limexp_update(&volts, &params, &state, &[], &sim, &mut ylim);
            state[lx_base..lx_base + lx_n].copy_from_slice(&ylim);
        }

        let mut jac = [0.0_f64; 4];
        kernel.eval_jacobian(&volts, &params, &state, &[], &sim, &mut jac);
        let analytic = jac[0];

        // `h` scales with the operating point so the finite-difference probe
        // stays well-conditioned across the huge dynamic range of an
        // exponential diode current (Is*exp(V/vt) spans ~1e-14 .. ~1e-4 V
        // over the probed voltages) — a fixed absolute `h` would either lose
        // precision to cancellation at small V or blur the derivative at
        // large V.
        let h = 1.0e-6 * (1.0 + v.abs());
        let f_plus = eval_residual_at(&kernel, is, vt, &state, v + h);
        let f_minus = eval_residual_at(&kernel, is, vt, &state, v - h);
        let fd = (f_plus - f_minus) / (2.0 * h);

        // Relative tolerance only: the diode current (and hence its
        // Jacobian) spans many orders of magnitude across the probed
        // voltages, so a fixed absolute floor would either be too loose to
        // catch a zeroed-out Jacobian at small V or too tight at large V.
        let tol = 1.0e-3 * fd.abs();
        assert!(
            (analytic - fd).abs() < tol,
            "at V={v}: analytic jac[0]={analytic} vs finite-difference={fd} \
             (diff={}, tol={tol}) — a zeroed limexp Jacobian would show \
             analytic ~= 0.0 here",
            (analytic - fd).abs()
        );
    }
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
