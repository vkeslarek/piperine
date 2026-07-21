//! System-task migration completeness — declared-language-surface T21
//! (DLS-19 completion).
//!
//! T20 collapsed `elab/resolve.rs`'s hardcoded `valid_diagnostics` array
//! into a `CallableRegistry` lookup against `headers/tasks.phdl`'s
//! `extern task` declarations. This is a dedicated regression fixture
//! proving the collapse didn't silently drop any of the original
//! `valid_diagnostics` entries — each one must still resolve as a
//! `BehaviorStmt::Diagnostic` call post-migration.
//!
//! Pre-migration `valid_diagnostics` (verified directly in the pre-T20
//! `elab/resolve.rs` source, 11 entries — tasks.md's "9" figure in its
//! Done-when text undercounts the real array; this fixture tests the
//! actual original 11, the correctness-relevant number):
//! `write`, `strobe`, `display`, `info`, `warning`, `error`, `fatal`,
//! `bound_step`, `finish`, `stop`, `discontinuity`.

use piperine_lang::{parse_and_elaborate, SourceMap};

/// Builds a minimal digital body issuing the given diagnostic call as a
/// statement, and asserts it elaborates without the
/// "Unrecognized diagnostic call" error T20's registry lookup would raise
/// for a name that isn't declared.
fn assert_diagnostic_resolves(sys: &str, call: &str) {
    let src = format!(
        "
        mod Top() {{}}
        digital Top {{
            {call};
        }}
        "
    );
    let result = parse_and_elaborate(&src, &SourceMap::dummy());
    assert!(
        result.is_ok(),
        "diagnostic `${sys}` must still resolve post-T20 migration, got: {:?}",
        result.err()
    );
}

#[test]
fn write_resolves() {
    assert_diagnostic_resolves("write", "$write()");
}

#[test]
fn strobe_resolves() {
    assert_diagnostic_resolves("strobe", "$strobe()");
}

#[test]
fn display_resolves() {
    assert_diagnostic_resolves("display", "$display()");
}

#[test]
fn info_resolves() {
    assert_diagnostic_resolves("info", "$info()");
}

#[test]
fn warning_resolves() {
    assert_diagnostic_resolves("warning", "$warning()");
}

#[test]
fn error_resolves() {
    assert_diagnostic_resolves("error", "$error()");
}

#[test]
fn fatal_resolves() {
    assert_diagnostic_resolves("fatal", "$fatal()");
}

#[test]
fn bound_step_resolves() {
    assert_diagnostic_resolves("bound_step", "$bound_step(1.0e-9)");
}

#[test]
fn finish_resolves() {
    assert_diagnostic_resolves("finish", "$finish()");
}

#[test]
fn stop_resolves() {
    assert_diagnostic_resolves("stop", "$stop()");
}

#[test]
fn discontinuity_resolves() {
    assert_diagnostic_resolves("discontinuity", "$discontinuity()");
}

/// The negative control: a name that was never in `valid_diagnostics` (and
/// has no `extern task` declaration) must still fail loud — proves the
/// registry lookup is a real check, not an accidental always-pass.
#[test]
fn undeclared_diagnostic_name_still_fails_loud() {
    let src = "
        mod Top() {}
        digital Top {
            $totally_not_a_real_diagnostic();
        }
    ";
    let err = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("an undeclared diagnostic name must fail loud");
    assert!(
        err.to_string().contains("totally_not_a_real_diagnostic"),
        "error should name the unrecognized diagnostic: {err}"
    );
}
