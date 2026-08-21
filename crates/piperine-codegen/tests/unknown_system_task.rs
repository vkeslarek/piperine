//! T13/LRMW2-09: a system task neither the analog nor the digital lowering
//! arm recognizes is a loud `CodegenError::Unsupported` naming the task and
//! LRM §9.2 — parity between the two arms, through the shared
//! `CodegenError::unknown_task` helper (`crates/piperine-codegen/src/error.rs`).
//! Before this fix `flatten/analog.rs`'s `_ => {}` silently dropped the
//! statement instead of erroring.
//!
//! LRM-COVERS: 9.2

use piperine_codegen::resolve::lower_bodies;
use piperine_codegen::{CodegenError, CompiledModule};
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "";

fn compile_err(src: &str, name: &str) -> CodegenError {
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elab");
    let bodies = lower_bodies(&prog).expect("lowering failed");
    match CompiledModule::compile(bodies.get(name).expect("module present")) {
        Err(e) => e,
        Ok(_) => panic!("device `{name}` must not compile — the task is unrecognized"),
    }
}

/// The task's own example: a typo'd `$dispaly` in an analog body — the
/// misspelling is not a real system task, so it must fail loud naming the
/// task and §9.2, not silently vanish.
#[test]
fn unknown_task_in_analog_body_errors_naming_task_and_clause() {
    let src = format!(
        "{DISCIPLINE}
module Probe ( inout electrical p, inout electrical n );
    analog begin
        I(p, n) <+ V(p, n);
        $dispaly(\"x\");
    end
endmodule
"
    );
    let err = compile_err(&src, "Probe");
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("dispaly"), "error must name the task: {msg}");
            assert!(msg.contains("9.2"), "error must cite LRM §9.2: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other}"),
    }
}

/// A syntactically well-formed but genuinely unimplemented task
/// (`$save`, LRM Table 9-1 — checkpointing, never lowered by this codegen)
/// must fail with the same error *kind* and message shape as the typo case
/// — "known-but-unimplemented" and "unknown spelling" both funnel through
/// the one `unknown_task` helper.
#[test]
fn known_but_unimplemented_task_in_analog_body_errors_same_shape() {
    let src = format!(
        "{DISCIPLINE}
module Probe ( inout electrical p, inout electrical n );
    analog begin
        I(p, n) <+ V(p, n);
        $save(\"chkpt\");
    end
endmodule
"
    );
    let err = compile_err(&src, "Probe");
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("save"), "error must name the task: {msg}");
            assert!(msg.contains("9.2"), "error must cite LRM §9.2: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other}"),
    }
}

/// Digital-arm parity (design's "one helper, two call sites"): the exact
/// same unrecognized task, in a digital `always` block, must produce the
/// same error *kind* (`CodegenError::Unsupported`) with the same message
/// shape — proving both arms route through `CodegenError::unknown_task`.
#[test]
fn unknown_task_in_digital_body_errors_with_same_kind_as_analog() {
    let src = "
module Probe ();
    reg out;
    always @(*) begin
        $dispaly(\"x\");
        out = 1;
    end
endmodule
";
    let err = compile_err(src, "Probe");
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("dispaly"), "error must name the task: {msg}");
            assert!(msg.contains("9.2"), "error must cite LRM §9.2: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported, got {other}"),
    }
}
