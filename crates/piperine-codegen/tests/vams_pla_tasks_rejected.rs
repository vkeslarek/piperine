//! LRM-COVERS: 9.8
//! LRMW3-20 (`lrm-conformance-wave3`, T14): the §9.8 PLA-modeling task
//! family (`$async$and$array`/`$plane`, `$async$nand$array`/`$plane`,
//! `$async$or$array`/`$plane`, `$async$nor$array`/`$plane`, and the
//! `$sync$…` twins — 16 names total) is recognized by name
//! (`piperine-lang/src/eval/tasks.rs::SystemTaskKind::PlaArray`) but
//! deliberately not lowered — each name fails loud citing §9.8, with the
//! reason recorded in this task's commit body and in
//! `docs/spec/part_iii_conformance.md` (spec.md's P3 AC2 explicitly allows
//! either a real implementation or this documented loud rejection).
//!
//! A real implementation would need a new JIT trampoline for runtime
//! plane-loading file I/O (the same infrastructure gap T12 found and left
//! open for `$readmemh`/`$readmemb`) plus a combinational-array evaluation
//! model that has no existing analog anywhere in this codebase — building
//! both from scratch is out of proportion for this task.

use piperine_codegen::DigitalKernel;
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "
discipline Bit;
    domain discrete;
enddiscipline
";

/// Elaborates a Dut module invoking `task_name` as a statement, then
/// attempts to compile its digital body. Returns the codegen error's
/// message (elaboration itself must succeed — the name is *recognized*,
/// only lowering is refused).
fn compile_error_for(task_name: &str) -> String {
    let src = format!(
        "{DISCIPLINE}
module Dut(input Bit a, input Bit b, output Bit y);
    always @(a or b) begin
        {task_name}(\"plane.txt\", y, a, b);
    end
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("`{task_name}` must still elaborate (it is a *recognized* name): {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    match DigitalKernel::compile(&bodies["Dut"]) {
        Ok(_) => panic!("`{task_name}` must fail loud at codegen, not silently compile"),
        Err(e) => e.to_string(),
    }
}

/// Every one of the 16 PLA-modeling task names fails loud citing §9.8 —
/// not a generic "unknown system task" message (they *are* recognized;
/// only lowering is refused), and each error names the actual task that
/// was rejected.
#[test]
fn every_pla_task_name_fails_loud_citing_section_9_8() {
    const NAMES: &[&str] = &[
        "$async$and$array", "$async$and$plane",
        "$async$nand$array", "$async$nand$plane",
        "$async$or$array", "$async$or$plane",
        "$async$nor$array", "$async$nor$plane",
        "$sync$and$array", "$sync$and$plane",
        "$sync$nand$array", "$sync$nand$plane",
        "$sync$or$array", "$sync$or$plane",
        "$sync$nor$array", "$sync$nor$plane",
    ];
    for name in NAMES {
        let msg = compile_error_for(name);
        assert!(msg.contains("9.8"), "`{name}`'s error must cite LRM §9.8, got: {msg}");
        let bare = name.trim_start_matches('$');
        assert!(
            msg.contains(bare),
            "`{name}`'s error must name the actual rejected task, got: {msg}"
        );
    }
}

/// Discriminating check: an *unrelated* unknown system task name (never
/// registered at all) still gets the ordinary "unknown task" treatment,
/// not the §9.8 message — proving the PLA rejection is name-specific, not
/// a blanket catch-all that would also (wrongly) swallow a real typo.
#[test]
fn an_unrelated_unknown_task_does_not_get_the_pla_message() {
    let src = format!(
        "{DISCIPLINE}
module Dut(input Bit a, output Bit y);
    always @(a) begin
        $totally_made_up_task(a, y);
    end
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborates (task-name validity is a codegen-time check)");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let err = match DigitalKernel::compile(&bodies["Dut"]) {
        Ok(_) => panic!("a genuinely unknown system task must fail loud at codegen"),
        Err(e) => e.to_string(),
    };
    assert!(!err.contains("9.8"), "an unrelated unknown task must not carry the §9.8 PLA message, got: {err}");
}
