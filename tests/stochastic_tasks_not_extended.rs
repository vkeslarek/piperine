//! LRM-COVERS: 9.9
//!
//! lrm-conformance-wave3 (Phase 6, T37): §9.9 "Stochastic analysis system
//! tasks" — "Verilog AMS HDL does not extend the stochastic analysis
//! tasks defined in IEEE Std 1364-2005 Verilog HDL." This is a "no
//! delta" clause: proving it means proving Piperine adds no VAMS-specific
//! support for the 1364-2005 queue/stochastic-analysis family
//! (`$q_initialize`, `$q_add`, `$q_remove`, `$q_full`, `$q_exam`, …) and
//! does not silently pretend to accept one — an unrecognized call fails
//! loud citing §9.2 (the general system-task-name error), the same
//! outcome any *unextended* 1364-2005 task gets, never a silent no-op.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;

#[test]
fn a_1364_2005_stochastic_task_is_not_silently_accepted() {
    let src = "discipline logic;\ndomain discrete;\nenddiscipline\n\
        module dut (input logic clk);\nalways @(posedge clk) $q_initialize(1, 2, 3, 4);\nendmodule\n\
        module top ();\nlogic clk_net;\ndut d1 (clk_net);\nendmodule\n";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    match compiler.build_circuit_mapped("top") {
        Ok(_) => panic!("$q_initialize must not be silently accepted — §9.9 extends nothing"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(
                msg.contains("q_initialize"),
                "the error must name the unrecognized task, got: {msg}"
            );
        }
    }
}
