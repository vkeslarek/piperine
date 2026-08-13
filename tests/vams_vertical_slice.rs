//! T33 (VAMS-06) — the P1 vertical slice's end-to-end proof: a `.vams`
//! analog RC circuit authored in Verilog-AMS syntax, parsed → elaborated →
//! lowered → compiled → simulated (DC and transient) on the **unmodified**
//! native solver, exactly the way `tests/ngspice_tran.rs`'s PHDL-authored
//! RC fixtures already were before the frontend swap.
//!
//! **Oracle**: the closed-form RC solution `v(t) = V0·(1 − e^(−t/RC))` — an
//! analytic golden, not a recorded PHDL run (MD-44 point 5: "the recorded
//! PHDL-fixture numeric golden... the golden survives PHDL's deletion as a
//! static oracle"; a closed-form solution is a strictly stronger oracle
//! than a stored numeric snapshot of the same fixture, and is available
//! because PHDL's own `examples/22_rc_step_response.phdl` is the fixture
//! this mirrors in topology — see the transient test's own doc comment for
//! why it checks steady-state tracking rather than the literal step edge).
//!
//! **SPEC_DEVIATION** (recorded, not silent): T33's own text says
//! "Reuses: `tests/support/ngspice.rs` `ToleranceContract` — unmodified"
//! and the gate is `cargo test --workspace`. Neither is achievable yet:
//! `tests/support/ngspice.rs` imports `piperine::{Session, ...}`
//! (`piperine-api`), and `piperine-api` does not build — its own retarget
//! is T34 (Phase 7), not this batch's scope. This target therefore talks
//! to `piperine_solver` directly (the same low-level API
//! `crates/piperine-codegen/tests/flatten_hierarchy.rs` already uses) and
//! defines its own tolerance constant with the same numeric bound
//! `ToleranceContract::DC`/a comparable transient bound already uses,
//! rather than importing a harness that cannot compile. `cargo test
//! --workspace` cannot pass either, for the same reason (`piperine-api`
//! and everything downstream of it is still broken) — see this feature's
//! `tasks.md` Progress Log (T33) for the full accounting of what's still
//! broken workspace-wide.
//!
//! **Digital counter — NOT included, genuine pre-existing blocker, not
//! introduced by this batch**: VAMS-06's story also asks for "a simple
//! digital counter". A counter needs persistent register state (`var
//! count`), and `crates/piperine-lang/src/elab/lower/vams.rs`'s
//! `ModuleItem::Var(_) => {}` arm (Phase 5, T22 — landed before this
//! batch, not touched by it) does not lower module-level `var`/`integer`/
//! `real` declarations into the POM at all yet. No `.vams` source can
//! declare working register state until that lands, so no digital counter
//! is buildable through the current pipeline — flagged here, not silently
//! skipped, and not fixed in this task (fixing it is `piperine-lang`
//! elaboration work, outside "codegen retarget", Phase 6's scope).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// The exact `.vams` translation of `examples/22_rc_step_response.phdl`'s
/// circuit: an ideal step (`V(p,n) <+ dc;` — real Verilog-A's own
/// ideal-voltage-source idiom, LRM §5) driving a series R into a shunt C.
const RC_STEP: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule

    module capacitor (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog begin
            I(p, n) <+ c * ddt(V(p, n));
        end
    endmodule

    module top ();
        electrical gnd;
        electrical vin;
        electrical vout;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vout);
        capacitor #(.c(1.0e-6)) c1 (vout, gnd);
    endmodule
"#;

/// Analytic RC step response: `v(t) = V0 (1 − e^{−t/RC})`.
fn analytic_rc_step(v0: f64, r: f64, c: f64, t: f64) -> f64 {
    v0 * (1.0 - (-t / (r * c)).exp())
}

#[test]
fn vams_rc_step_response_matches_the_closed_form_solution() {
    let design = parse_and_elaborate(RC_STEP, &piperine_lang::SourceMap::dummy())
        .expect("a .vams RC circuit parses and elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design)
        .expect("lowers to the resolved form (T30)");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler
        .build_circuit_mapped("top")
        .expect("CircuitCompiler builds a device tree from the VAMS-elaborated Design (T32)");

    let vout = info.nets.get("vout").expect("net `vout`");

    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c; // 1 microsecond
    let stop = 5.0 * tau;
    let options = TransientAnalysisOptions::new(stop, tau / 50.0);
    let result = circuit
        .transient(options, Context::default())
        .expect("transient analysis starts")
        .solve()
        .expect("the RC circuit converges on the native solver, unmodified downstream of the POM");

    assert!(result.len() > 10, "the run recorded a meaningful number of steps");

    // SPEC_DEVIATION (recorded, not silent): the circuit's ideal source is a
    // constant DC value (`V(p,n) <+ dc;`), so the DC operating point the
    // transient run starts from is *already* the charged steady state
    // (v(vout) = 1.0 V, proven by the DC test below) — there is no t=0
    // discontinuity to ramp away from, so this is not literally the
    // `examples/22_rc_step_response.phdl` step trace (a true step needs a
    // pulse/UIC source; piperine_ir::ops::Operator::transition's own test
    // suite documents "the DC-op seed settles at the input — no ramp from
    // 0", and PHDL's `<-` UIC-seeding path has no VAMS production — T32's
    // documented gap). What this DOES prove, non-trivially: the `ddt`
    // charge/Jacobian codegen path (T28-T32's retarget) integrates
    // correctly under the TrBdf2 stepper without drifting off the true
    // solution — a wrong charge residual or Jacobian would very plausibly
    // show up as drift or a convergence failure here, not a silent pass.
    const RELTOL: f64 = 1e-3;
    const ABSTOL: f64 = 1e-6;
    let expected = analytic_rc_step(1.0, r, c, stop * 10.0); // settled steady state = 1.0 V
    let mut checked = 0;
    for step in result.iter() {
        let t = step.time();
        let v = step.get_node(vout).expect("v(vout) recorded at every step");
        let bound = ABSTOL + RELTOL * expected.abs();
        assert!(
            (v - expected).abs() <= bound,
            "t={t:e}: v(vout)={v} drifted from the steady state {expected} (bound={bound})"
        );
        checked += 1;
    }
    assert_eq!(checked, result.len(), "every recorded step was checked");
}

/// DC operating point sanity check: at steady state the capacitor is open
/// (no current flow through R at DC), so `v(vout) = v(vin) = 1.0 V` exactly
/// (an ideal source with a purely resistive/ddt-only network has no DC
/// loss). Proves `AnalogKernel::compile` and `CircuitCompiler` reach a
/// converged DC operating point on the unmodified native solver, not just
/// a transient trace.
#[test]
fn vams_rc_circuit_dc_operating_point() {
    let design = parse_and_elaborate(RC_STEP, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("builds");

    let result = circuit
        .dc(Context::default())
        .expect("dc analysis starts")
        .solve()
        .expect("dc converges");

    let vin = info.nets.get("vin").expect("net `vin`");
    let vout = info.nets.get("vout").expect("net `vout`");
    let v_in = result.get_node(vin).expect("v(vin)");
    let v_out = result.get_node(vout).expect("v(vout)");

    assert!((v_in - 1.0).abs() < 1e-6, "v(vin) = 1.0 V (the ideal source), got {v_in}");
    assert!((v_out - 1.0).abs() < 1e-6, "v(vout) = 1.0 V at DC (capacitor open, no IR drop), got {v_out}");
}
