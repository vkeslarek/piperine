//! SC-08 — `table()` loud-error paths at lowering: non-monotonic
//! breakpoints, length mismatch, and non-constant arrays are `LowerError`s,
//! never a silent 0.0.

fn lower_err(body: &str) -> String {
    let src = format!(
        "discipline Electrical {{ potential v : Real; flow i : Real; }}\n\
         mod T (inout p : Electrical, inout n : Electrical) {{ }}\n\
         analog T {{ I(p, n) <+ {body}; }}\n"
    );
    let design =
        piperine_lang::parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
            .expect("elaborates");
    let err = piperine_codegen::resolve::lower_bodies(&design).expect_err("must fail loud");
    format!("{err}")
}

#[test]
fn non_monotonic_breakpoints_fail_loud() {
    let msg = lower_err("table(V(p, n), [0.0, 2.0, 1.0], [0.0, 1.0, 2.0])");
    assert!(msg.contains("strictly increasing"), "names the rule: {msg}");
}

#[test]
fn length_mismatch_fails_loud() {
    let msg = lower_err("table(V(p, n), [0.0, 1.0, 2.0], [0.0, 1.0])");
    assert!(msg.contains("same length"), "names the rule: {msg}");
}

#[test]
fn arity_and_mode_fail_loud() {
    let msg = lower_err("table(V(p, n), [0.0, 1.0])");
    assert!(msg.contains("3 arguments"), "names the arity: {msg}");
    let msg = lower_err(
        "table(V(p, n), [0.0, 1.0], [0.0, 1.0], \"cubic\")",
    );
    assert!(msg.contains("linear"), "names the supported mode: {msg}");
}
