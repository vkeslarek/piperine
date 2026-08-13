//! `piperine build` (PB-01..04): actually elaborates + compiles the target
//! design instead of the old stub that printed and stopped.

use std::path::Path;
use std::process::{Command, Output};

/// A scratch project: `Piperine.toml` marker + the given `.vams` files
/// (relative path under `src/` → content).
fn scratch_project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("Piperine.toml"), "[project]\nname = \"scratch\"\n").unwrap();
    for (rel, content) in files {
        let path = dir.path().join("src").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

fn piperine_build(dir: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_piperine"))
        .arg("build")
        .args(extra)
        .current_dir(dir)
        .output()
        .expect("spawn piperine build")
}

fn combined(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

const VALID_ZERO_PORT_DESIGN: &str = "\
`include \"disciplines.vams\"
module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Board ();
    electrical a, gnd;
    Resistor #(.r(1e3)) r1 (.p(a), .n(gnd));
endmodule
";

/// PB-01 AC1: a valid zero-port design builds successfully (elaborates +
/// runs the full codegen pipeline), exit 0, per-module success reported.
#[test]
fn valid_zero_port_design_builds_and_exits_zero() {
    let dir = scratch_project(&[("main.vams", VALID_ZERO_PORT_DESIGN)]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(out.status.success(), "expected exit 0, got: {text}");
    assert!(text.contains("built `Board`"), "expected a per-module success line, got: {text}");
}

/// PB-01 AC2: elaboration failure (parse error) exits non-zero and prints
/// the elaboration error — never silently succeeds.
#[test]
fn elaboration_failure_exits_nonzero_with_error() {
    let dir = scratch_project(&[("main.vams", "module Broken (); this is not vams")]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(!out.status.success(), "expected non-zero exit, got success. output: {text}");
    assert!(text.contains("Elaboration failed"), "expected an elaboration error, got: {text}");
}

/// PB-01 AC3: a zero-port module whose body fails codegen (an unsupported
/// construct) exits non-zero with the error attributed to that module.
#[test]
fn codegen_failure_exits_nonzero_attributed_to_module() {
    // A call to an unresolvable function name elaborates fine (elaboration
    // doesn't require the callee to exist as a builtin) but fails IR
    // validation at codegen time — a real post-elaboration failure (mirrors
    // `piperine-codegen/tests/silent_bugs.rs::d5_user_fn_missing_still_errors`).
    let design = "\
`include \"disciplines.vams\"
module Broken (inout electrical p, inout electrical n);
    analog begin
        I(p, n) <+ no_such_fn(V(p, n));
    end
endmodule
module Board ();
    electrical a, gnd;
    Broken b1 (.p(a), .n(gnd));
endmodule
";
    let dir = scratch_project(&[("main.vams", design)]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(!out.status.success(), "expected non-zero exit, got success. output: {text}");
    assert!(text.contains("`Board` failed to build"), "expected the error attributed to `Board`, got: {text}");
}

/// PB-01 AC4: a project with no zero-port modules (library-only) is not an
/// error — prints a note and exits 0.
#[test]
fn library_only_project_prints_note_and_exits_zero() {
    let design = "\
`include \"disciplines.vams\"
module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
";
    let dir = scratch_project(&[("main.vams", design)]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(out.status.success(), "a library-only project must exit 0, got: {text}");
    assert!(text.contains("nothing to build"), "expected a nothing-to-build note, got: {text}");
}

/// Edge case: an explicit `file` argument overrides the default `src/main.vams`
/// discovery (matches `check`'s existing single-file override behavior).
#[test]
fn explicit_file_argument_overrides_default_discovery() {
    let dir = scratch_project(&[
        ("main.vams", "module Unused (); endmodule"),
        ("other.vams", VALID_ZERO_PORT_DESIGN),
    ]);
    let other_path = dir.path().join("src").join("other.vams");
    let out = piperine_build(dir.path(), &[other_path.to_str().unwrap()]);
    let text = combined(&out);
    assert!(out.status.success(), "expected exit 0 building the explicit file, got: {text}");
    assert!(text.contains("built `Board`"), "expected `Board` (from other.vams) built, got: {text}");
}
