//! LRM-COVERS: 4.7, 4.7.1, 4.7.2.1, 4.7.3
//!
//! LRMW2-44 (`lrm-conformance-wave2`, T44): `analog function` declarations
//! used to be silently dropped at `elab::lower::vams::elab_module`
//! (`ModuleItem::Function(_) => {}`) — the resolved-form inliner
//! (`resolve/pom/expr.rs::inline_user_fn`/`inline_fn_body`) already existed
//! but `SymbolTable::fn_by_name` never matched anything, so it was dead
//! code. This is the end-to-end proof required by spec.md's AC15 ("a user
//! `analog function` returns a value used in a contribution"): the LRM's
//! own §4.7.1 Example 1 (`maxValue`), simulated (DC operating point), not
//! merely elaborated.
//!
//! Re-verification note (deeper gap than the task's own "just wiring"
//! framing): `inline_fn_body` at HEAD (pre-T44) never actually returned the
//! function's own-name assignment for a flat (non-nested-`begin`) body — it
//! always fell through to a hardcoded `0.0`. Fixed alongside the wiring
//! (mirrors `emit/builder.rs`'s already-correct digital-path twin) — see
//! the T44 commit body.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// LRM §4.7.1 Example 1, verbatim (renamed `n1`/`n2` kept): "This example
/// defines an analog user defined function called `maxValue`, which
/// returns the potential of whichever signal is larger."
fn maxvalue_fixture(va: f64, vb: f64) -> f64 {
    let src = format!(
        r#"
        module dut (inout electrical outp);
            electrical a, b;

            analog function real maxValue;
                input n1, n2;
                real n1, n2;
                begin
                    maxValue = (n1 > n2) ? n1 : n2;
                end
            endfunction

            analog begin
                V(a) <+ {va};
                V(b) <+ {vb};
                V(outp) <+ maxValue(V(a), V(b));
            end
        endmodule

        module top ();
            electrical outp;
            dut d1 (outp);
        endmodule
        "#
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let outp = info.nets.get("outp").expect("net `outp`");
    dc.get_node(outp).expect("V(outp) at the operating point")
}

/// AC15: the function's return value drives `V(outp, gnd) <+ maxValue(...)`
/// — `outp` observably tracks `max(a, b)`, not a constant/dropped value.
#[test]
fn analog_function_return_drives_a_contribution() {
    let out = maxvalue_fixture(1.0, 3.0);
    assert!((out - 3.0).abs() < 1e-9, "max(1.0, 3.0) = 3.0, got {out}");
}

/// Discrimination (spec.md AC15/Wave-2 §standing rule): swapping which
/// argument is larger must change which one wins — an inliner that always
/// picked (or dropped) one argument would pass the first case above but
/// fail this one.
#[test]
fn analog_function_return_tracks_which_argument_is_larger() {
    let out = maxvalue_fixture(5.0, 2.0);
    assert!((out - 5.0).abs() < 1e-9, "max(5.0, 2.0) = 5.0, got {out}");
}
