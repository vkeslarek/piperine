//! LRM-COVERS: 5.10.2
//!
//! LRM §5.10.2 "Global events": `@(final_step)` fires once, at the end of
//! an analysis. `flatten/analog.rs`'s `add_event` used to push a no-op
//! `Info`-severity diagnostic and `return Ok(())` for `EventSource::FinalStep`
//! — the body's statements never even reached the flattener, so the
//! trigger compiled to nothing and the body never ran (SPEC_DEVIATION.md
//! finding 1.5, re-verified reproducing at HEAD before this fix). This
//! test proves it fires — exactly once — through the whole pipeline: a
//! module `var` updated only inside `@(final_step)`, read back through
//! `read_opvars()` after a real transient solve.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

fn build(src: &str, top: &str) -> piperine_solver::prelude::CircuitInstance {
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies: HashMap<String, LoweredBody> = piperine_codegen::resolve::lower_bodies(&elab).expect("lowers");
    CircuitCompiler::new(&elab, &bodies).build_circuit(top).expect("circuit compiles")
}

/// A trivial resistor branch keeps the circuit well-posed (a converging DC
/// point to start the transient from); `fired_count` is touched ONLY
/// inside `@(final_step)`.
const SRC: &str = "
module Dut (inout electrical p, inout electrical n);
    real fired_count;
    analog begin
        I(p, n) <+ V(p, n) / 1.0e6;
        @(final_step)
            fired_count = fired_count + 1.0;
    end
endmodule
module Top ();
    electrical a, gnd;
    Dut d1 (a, gnd);
endmodule
";

/// AC (T12 Done-when): a `final_step` body's side effect is observable
/// exactly once per analysis — not zero (the pre-fix no-op) and not
/// repeated on every accepted step.
#[test]
fn final_step_fires_exactly_once_per_transient_analysis() {
    let mut circuit = build(SRC, "Top");
    let mut tran = circuit
        .transient(TransientAnalysisOptions::new(1.0, 0.1), Context::default())
        .expect("transient starts");
    tran.solve().expect("transient run completes");
    drop(tran);

    let dev = &circuit.all_devices()[0];
    let opvars: HashMap<String, f64> = dev.read_opvars().into_iter().collect();
    let fired_count = *opvars
        .get("fired_count")
        .unwrap_or_else(|| panic!("missing opvar `fired_count`, got {opvars:?}"));
    assert!(
        (fired_count - 1.0).abs() < 1e-12,
        "final_step's body must run exactly once (not 0 — the pre-fix no-op — \
         and not >1 — once per accepted step): got fired_count = {fired_count}"
    );
}

/// Regression: a DC operating point ALSO triggers `final_step` exactly
/// once (LRM Table 5-1: `final_step` fires at DCOP too, not just at the
/// end of a sweep/transient/AC/noise run) — the `analysis_kind == Dc`
/// gate in `DcSolver::solve` must not accidentally suppress the
/// top-level case it exists to allow.
#[test]
fn final_step_fires_once_on_a_plain_dc_operating_point_too() {
    let mut circuit = build(SRC, "Top");
    circuit.dc(Context::default()).expect("dc solver starts").solve().expect("DC operating point solves");

    let dev = &circuit.all_devices()[0];
    let opvars: HashMap<String, f64> = dev.read_opvars().into_iter().collect();
    let fired_count = *opvars.get("fired_count").expect("opvar `fired_count`");
    assert!(
        (fired_count - 1.0).abs() < 1e-12,
        "final_step must fire once on a plain DC operating point too, got {fired_count}"
    );
}
