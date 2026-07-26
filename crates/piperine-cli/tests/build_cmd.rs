//! `piperine build` (PB-01..04): actually elaborates + compiles the target
//! design instead of the old stub that printed and stopped.

use std::path::Path;
use std::process::{Command, Output};

/// A scratch project: `Piperine.toml` marker + the given `.phdl` files
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
discipline Electrical { potential v: Real; flow i: Real; }
mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog Resistor { I(p, n) <+ V(p, n) / r; }
mod Board() {
    wire a: Electrical; wire gnd: Electrical;
    r1: Resistor(.p = a, .n = gnd) { .r = 1e3 };
}
";

/// PB-01 AC1: a valid zero-port design builds successfully (elaborates +
/// runs the full codegen pipeline), exit 0, per-module success reported.
#[test]
fn valid_zero_port_design_builds_and_exits_zero() {
    let dir = scratch_project(&[("main.phdl", VALID_ZERO_PORT_DESIGN)]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(out.status.success(), "expected exit 0, got: {text}");
    assert!(text.contains("built `Board`"), "expected a per-module success line, got: {text}");
}

/// PB-01 AC2: elaboration failure (parse error) exits non-zero and prints
/// the elaboration error — never silently succeeds.
#[test]
fn elaboration_failure_exits_nonzero_with_error() {
    let dir = scratch_project(&[("main.phdl", "mod Broken() { this is not phdl")]);
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
discipline Electrical { potential v: Real; flow i: Real; }
mod Broken(inout p: Electrical, inout n: Electrical) { }
analog Broken { I(p, n) <+ no_such_fn(V(p, n)); }
mod Board() {
    wire a: Electrical; wire gnd: Electrical;
    b1: Broken(.p = a, .n = gnd) { };
}
";
    let dir = scratch_project(&[("main.phdl", design)]);
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
discipline Electrical { potential v: Real; flow i: Real; }
mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog Resistor { I(p, n) <+ V(p, n) / r; }
";
    let dir = scratch_project(&[("main.phdl", design)]);
    let out = piperine_build(dir.path(), &[]);
    let text = combined(&out);
    assert!(out.status.success(), "a library-only project must exit 0, got: {text}");
    assert!(text.contains("nothing to build"), "expected a nothing-to-build note, got: {text}");
}

/// Edge case: an explicit `file` argument overrides the default `src/main.phdl`
/// discovery (matches `check`'s existing single-file override behavior).
#[test]
fn explicit_file_argument_overrides_default_discovery() {
    let dir = scratch_project(&[
        ("main.phdl", "mod Unused() { }"),
        ("other.phdl", VALID_ZERO_PORT_DESIGN),
    ]);
    let other_path = dir.path().join("src").join("other.phdl");
    let out = piperine_build(dir.path(), &[other_path.to_str().unwrap()]);
    let text = combined(&out);
    assert!(out.status.success(), "expected exit 0 building the explicit file, got: {text}");
    assert!(text.contains("built `Board`"), "expected `Board` (from other.phdl) built, got: {text}");
}
