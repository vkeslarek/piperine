//! `AnalogKernel` numerics: what the JIT emits for one module — charge and
//! charge-Jacobian for a reactive contribution, the `SimCtx` ambient reads
//! (`$vt`, `$simparam`), guard folding, function inlining, and the runtime
//! state a shaping operator claims.
//!
//! Restored from the never-compiled `analog_jit.rs` (P6/CLN-06). The original
//! hand-built `LoweredBody` values through `IrExpr`/`IrStmt`, which no longer
//! exist; every test here drives the same assertions from PHDL source through
//! `lower_bodies`, the idiom `limiters.rs` already uses. The original's
//! resistor residual/Jacobian and its four solve-level cases are not restored —
//! they duplicate `codegen_ir.rs` and `from_ir.rs` (see the P6 audit).

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// Compile the two-terminal module `Dut` whose body is `body`, with `params`
/// declared verbatim in the module header.
fn kernel(params: &str, body: &str) -> Result<AnalogKernel, piperine_codegen::CodegenError> {
    let src = format!(
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{ {params} }}
analog Dut {{ {body} }}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    AnalogKernel::compile(&bodies["Dut"])
}

/// Residual of a compiled two-terminal kernel at `volts`, with `params`.
fn residual(kernel: &AnalogKernel, volts: [f64; 2], params: &[f64], sim: &SimCtx) -> [f64; 2] {
    let vars = vec![0.0; kernel.num_vars()];
    let state = vec![0.0; kernel.num_state_slots()];
    let mut out = [0.0; 2];
    kernel.eval_residual(&volts, params, &state, &vars, sim, &mut out);
    out
}

// ─── Reactive contributions ───────────────────────────────────────────────────

#[test]
fn a_capacitor_emits_charge_and_charge_jacobian_but_no_resistive_current() {
    let kernel = kernel("param c: Real = 1e-6;", "I(p, n) <+ ddt(c * V(p, n));")
        .expect("compile capacitor");
    assert!(kernel.has_reactive(), "a ddt contribution is reactive");

    let params = [1e-6];
    let sim = SimCtx::default();
    let volts = [3.0, 1.0];
    let vars = vec![0.0; kernel.num_vars()];
    let state = vec![0.0; kernel.num_state_slots()];

    // Q = C·V; the resistive residual of a pure capacitor is zero.
    assert_eq!(residual(&kernel, volts, &params, &sim), [0.0; 2], "no resistive current");

    let mut charge = [0.0; 2];
    kernel.eval_charge(&volts, &params, &state, &vars, &sim, &mut charge);
    let expected = 1e-6 * 2.0;
    assert!((charge[0] - expected).abs() < 1e-18, "Q(p) = {} vs {expected}", charge[0]);
    assert!((charge[1] + expected).abs() < 1e-18, "Q(n) = -Q(p): {}", charge[1]);

    let mut charge_jacobian = [0.0; 4];
    kernel.eval_charge_jacobian(&volts, &params, &state, &vars, &sim, &mut charge_jacobian);
    assert!((charge_jacobian[0] - 1e-6).abs() < 1e-18, "dQ(p)/dV(p) = C");
    assert!((charge_jacobian[3] - 1e-6).abs() < 1e-18, "dQ(n)/dV(n) = C");
}

// ─── Ambient reads from SimCtx ────────────────────────────────────────────────

#[test]
fn a_diode_reads_its_thermal_voltage_from_the_sim_context() {
    let kernel = kernel("param is: Real = 1e-14;", "I(p, n) <+ is * (exp(V(p, n) / $vt) - 1.0);")
        .expect("compile diode");

    let params = [1e-14];
    let sim = SimCtx::at_temperature(300.0);
    let vt = 300.0 * SimCtx::K_B_OVER_Q;
    let expected = 1e-14 * ((0.6 / vt).exp() - 1.0);

    let res = residual(&kernel, [0.6, 0.0], &params, &sim);
    assert!(
        (res[0] - expected).abs() < expected.abs() * 1e-12,
        "diode current {} vs {expected}",
        res[0]
    );

    // dI/dV = is/vt · exp(V/vt) — the symbolic derivative, at the same temperature.
    let vars = vec![0.0; kernel.num_vars()];
    let state = vec![0.0; kernel.num_state_slots()];
    let mut jacobian = [0.0; 4];
    kernel.eval_jacobian(&[0.6, 0.0], &params, &state, &vars, &sim, &mut jacobian);
    let g = 1e-14 / vt * (0.6 / vt).exp();
    assert!((jacobian[0] - g).abs() < g * 1e-12, "dI/dV = {} vs {g}", jacobian[0]);
}

#[test]
fn simparam_reads_the_sim_context_temperature() {
    // `$simparam("temp", default)` must return the context's temperature, not
    // its default, when the context supplies one.
    let kernel = kernel("", "I(p, n) <+ $simparam(\"temperature\", 1.0) * V(p, n);")
        .expect("compile $simparam reader");

    let hot = residual(&kernel, [1.0, 0.0], &[], &SimCtx::at_temperature(400.0));
    let cold = residual(&kernel, [1.0, 0.0], &[], &SimCtx::at_temperature(300.0));
    assert!((hot[0] - 400.0).abs() < 1e-9, "reads the context temperature: {}", hot[0]);
    assert!((cold[0] - 300.0).abs() < 1e-9, "tracks the context: {}", cold[0]);
}

// ─── Control flow and functions ───────────────────────────────────────────────

#[test]
fn a_guarded_contribution_conducts_only_above_its_threshold() {
    let kernel = kernel(
        "param r: Real = 100.0;",
        "if (V(p, n) > 1.0) { I(p, n) <+ V(p, n) / r; }",
    )
    .expect("compile clipper");

    let params = [100.0];
    let sim = SimCtx::default();
    let on = residual(&kernel, [2.0, 0.0], &params, &sim);
    assert!((on[0] - 0.02).abs() < 1e-15, "above threshold conducts: {}", on[0]);
    assert_eq!(residual(&kernel, [0.5, 0.0], &params, &sim), [0.0; 2], "below threshold is off");
}

#[test]
fn a_user_function_is_inlined_into_the_contribution() {
    let src = format!(
        "{DISCIPLINE}
fn double(x: Real) -> Real {{ return x * 2.0; }}
mod Dut(inout p: Electrical, inout n: Electrical) {{}}
analog Dut {{ I(p, n) <+ double(V(p, n)); }}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let kernel = AnalogKernel::compile(&bodies["Dut"]).expect("compile doubler");

    let res = residual(&kernel, [1.5, 0.0], &[], &SimCtx::default());
    assert!((res[0] - 3.0).abs() < 1e-15, "double(1.5) = 3.0, got {}", res[0]);
}

// ─── Runtime operators ────────────────────────────────────────────────────────

/// The original suite asserted that `transition` **fails loud** as an operator
/// with no companion model. That is no longer true: it compiles and runs
/// (`device/analog/operators.rs::transition_at`), so the assertion is inverted
/// to the behaviour that actually holds — it compiles and claims a runtime
/// state slot. (`CLAUDE.md`'s known-gaps list still calls `transition` a gap;
/// corrected in T24. `laplace_*`/`zi_*` cannot reach codegen at all — they
/// have no `extern operator` declaration, so MD-24 stops them at elaboration.)
#[test]
fn transition_compiles_and_claims_a_runtime_state_slot() {
    let kernel = kernel("", "I(p, n) <+ transition(V(p, n), 0.0, 1e-6, 1e-6);")
        .expect("transition has a companion model");
    assert!(kernel.num_state_slots() > 0, "the shaping operator owns runtime state");
}
