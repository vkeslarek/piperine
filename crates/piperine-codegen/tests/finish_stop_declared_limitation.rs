//! LRM-COVERS: 9.7.1, 9.7.2
//!
//! lrm-conformance-wave3 (Phase 6, T37): §9.7.1 `$finish` and §9.7.2
//! `$stop` — simulation-control termination, including the Table 9-25
//! diagnostic-level argument. **Declared limitation** (the choice
//! recorded in this task's commit body, per T37's "implement or declare
//! a limitation" instruction): neither task is implemented anywhere in
//! this codebase — no mechanism exists to end a running transient/DC
//! solve early from inside a device body. Before this task the DIGITAL
//! context silently no-op'd (`emit/stmt.rs`'s `"finish" | "stop" =>
//! Ok(())`) — a genuine CLAUDE.md fail-loud violation, since a design
//! calling `$finish` got neither an error nor termination. This task
//! makes the digital context fail loud too, matching the analog
//! context's pre-existing honest rejection
//! (`flatten/analog.rs`'s `"finish" | "stop" =>
//! Err(CodegenError::unsupported(...))`).
//!
//! Implementing the real feature needs a "stop early" signal threaded
//! from a device body through the transient/DC loop up to the host
//! `Session` — solver-level plumbing well beyond a coverage-backfill
//! task's scope.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;

fn digital_fails_loud(task: &str, arg: &str) {
    let src = format!(
        "discipline logic;\ndomain discrete;\nenddiscipline\n\
         module dut (input logic clk);\nalways @(posedge clk) ${task}({arg});\nendmodule\n\
         module top ();\nlogic clk_net;\ndut d1 (clk_net);\nendmodule\n"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    match compiler.build_circuit_mapped("top") {
        Ok(_) => panic!("${task} in the digital context must fail loud, not silently no-op"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(msg.contains(task), "error must name the task, got: {msg}");
        }
    }
}

fn analog_fails_loud(task: &str, arg: &str) {
    let src = format!(
        "module dut (inout electrical a);\nanalog begin\n${task}({arg});\nend\nendmodule\n\
         module top ();\nelectrical a;\ndut d1 (a);\nendmodule\n"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    if compiler.build_circuit_mapped("top").is_ok() {
        panic!("${task} in the analog context must fail loud");
    }
}

#[test]
fn dollar_finish_fails_loud_in_both_contexts() {
    for level in ["", "0", "1", "2"] {
        digital_fails_loud("finish", level);
        analog_fails_loud("finish", level);
    }
}

#[test]
fn dollar_stop_fails_loud_in_both_contexts() {
    for level in ["", "0", "1", "2"] {
        digital_fails_loud("stop", level);
        analog_fails_loud("stop", level);
    }
}
