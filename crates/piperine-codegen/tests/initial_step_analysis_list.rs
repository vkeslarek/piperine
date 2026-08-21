//! LRM-COVERS: 5.10.2
//!
//! LRM §5.10.2 "Global events", Table 5-1: `initial_step`'s optional
//! analysis-name-list argument restricts which analyses it fires for —
//! `initial_step("dc")` fires at a DC operating point but NOT during a
//! transient run; `initial_step("dc", "tran")` fires in both. The list
//! argument used to parse and get silently dropped (SPEC_DEVIATION.md
//! finding 1.5's second half, re-verified reproducing at HEAD: both forms
//! behaved identically to the unqualified `initial_step`, always firing
//! regardless of the list). Fixed by routing the QUALIFIED form through a
//! new per-analysis `Element::start_analysis` hook (the codegen device's
//! own doc comment on `AnalogInstance::fire_initial_events` explains why
//! the bare/qualified forms fire at different times).

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const SRC: &str = "
module Dut (inout electrical p, inout electrical n);
    real dc_only, dc_and_tran;
    analog begin
        @(initial_step(\"dc\"))
            dc_only = dc_only + 1.0;
        @(initial_step(\"dc\", \"tran\"))
            dc_and_tran = dc_and_tran + 1.0;
        I(p, n) <+ V(p, n) / 1.0e6;
    end
endmodule
module Top ();
    electrical a, gnd;
    Dut d1 (a, gnd);
endmodule
";

fn build() -> piperine_solver::prelude::CircuitInstance {
    let elab = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies: HashMap<String, LoweredBody> = piperine_codegen::resolve::lower_bodies(&elab).expect("lowers");
    CircuitCompiler::new(&elab, &bodies).build_circuit("Top").expect("circuit compiles")
}

fn opvars(circuit: &piperine_solver::prelude::CircuitInstance) -> HashMap<String, f64> {
    circuit.all_devices()[0].read_opvars().into_iter().collect()
}

/// `initial_step("dc")` and `initial_step("dc","tran")` both fire on a
/// plain DC operating point (LRM Table 5-1's DCOP column: `1` for both).
#[test]
fn qualified_initial_step_fires_on_a_dc_operating_point() {
    let mut circuit = build();
    circuit.dc(Context::default()).expect("dc solver starts").solve().expect("DC operating point solves");
    let vars = opvars(&circuit);
    assert!(
        (vars["dc_only"] - 1.0).abs() < 1e-12,
        "initial_step(\"dc\") must fire once on a dc run, got {}",
        vars["dc_only"]
    );
    assert!(
        (vars["dc_and_tran"] - 1.0).abs() < 1e-12,
        "initial_step(\"dc\",\"tran\") must fire once on a dc run, got {}",
        vars["dc_and_tran"]
    );
}

/// `initial_step("dc")` does NOT fire during a transient-only run (LRM
/// Table 5-1's TRAN column for `initial_step("dc")` is `0`);
/// `initial_step("dc","tran")` DOES (column is `1` at its first point) —
/// the exact AC from T12's Done-when.
#[test]
fn qualified_initial_step_dc_only_form_does_not_fire_in_a_transient_run() {
    let mut circuit = build();
    let mut tran = circuit
        .transient(TransientAnalysisOptions::new(1.0, 0.1), Context::default())
        .expect("transient starts");
    tran.solve().expect("transient run completes");
    drop(tran);

    let vars = opvars(&circuit);
    assert!(
        (vars["dc_only"] - 0.0).abs() < 1e-12,
        "initial_step(\"dc\") must NOT fire during a transient-only run \
         (discriminating: the pre-fix bug always fired it), got {}",
        vars["dc_only"]
    );
    assert!(
        (vars["dc_and_tran"] - 1.0).abs() < 1e-12,
        "initial_step(\"dc\",\"tran\") must fire once at the start of the transient run, got {}",
        vars["dc_and_tran"]
    );
}
