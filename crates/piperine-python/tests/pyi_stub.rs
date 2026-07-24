//! T28 — Complete `.pyi` stubs + docstrings (HOST-26). Spec-derived from
//! tasks.md's "Done when": every public class/fn has a stub with typed
//! kwargs + docstring; import smoke passes; autocomplete-visible fields
//! verified.
//!
//! The stub (`python/piperine/_piperine.pyi`) is hand-written (a compiled
//! `.so` carries no type info of its own), so this test is the mechanical
//! check that guards against drift: every class/function/method/property
//! the stub declares must actually exist at runtime on the native
//! `_piperine` module — a stub entry with no matching runtime attribute is
//! worse than no stub (it lies to the IDE).

use std::path::PathBuf;

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

fn stub_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/python/piperine/_piperine.pyi"))
}

/// The `.pyi` parses as valid Python syntax (a stub is a Python source
/// file with `...` bodies) — `piperine test`/tooling can load it without
/// a syntax error.
#[test]
fn stub_file_is_syntactically_valid_python() {
    let path = stub_path();
    let src = std::fs::read_to_string(&path).expect("read _piperine.pyi");
    let script = format!(
        r#"
import ast
with open({path:?}) as f:
    src = f.read()
ast.parse(src)
print("stub parses OK")
"#,
        path = path.to_str().expect("utf8 path"),
    );
    let _ = &src; // keep the read (fail loud above if missing) even though the script re-reads
    let script_path = write_temp("piperine_t28_syntax.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "stub syntax script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// `py.typed` (PEP 561) marker exists — required for type checkers to
/// treat the installed package as typed at all.
#[test]
fn py_typed_marker_exists() {
    let marker = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/python/piperine/py.typed"));
    assert!(marker.is_file(), "py.typed marker must exist at {marker:?}");
}

/// Every class the stub declares (top-level `class Foo:`) exists as a real
/// attribute on the native `_piperine` module at runtime, and every
/// method/property/getter name declared inside that class body exists on
/// the real runtime class — the "autocomplete-visible fields verified"
/// Done-when criterion, checked mechanically instead of by eyeballing an
/// IDE.
#[test]
fn stub_classes_and_members_exist_at_runtime() {
    let path = stub_path();
    let script = format!(
        r#"
import ast
import _piperine

with open({path:?}) as f:
    tree = ast.parse(f.read())

failures = []
checked_classes = 0
checked_members = 0

for node in tree.body:
    if isinstance(node, ast.FunctionDef):
        # top-level function (e.g. load/load_str)
        if not hasattr(_piperine, node.name):
            failures.append(f"module function {{node.name}} has no runtime _piperine.{{node.name}}")
        continue
    if not isinstance(node, ast.ClassDef):
        continue
    checked_classes += 1
    if not hasattr(_piperine, node.name):
        failures.append(f"class {{node.name}} has no runtime _piperine.{{node.name}}")
        continue
    runtime_cls = getattr(_piperine, node.name)
    for member in node.body:
        name = None
        if isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef)):
            name = member.name
        elif isinstance(member, ast.AnnAssign) and isinstance(member.target, ast.Name):
            name = member.target.id
        if name is None or name.startswith("__"):
            continue
        checked_members += 1
        if not hasattr(runtime_cls, name):
            failures.append(f"{{node.name}}.{{name}} declared in stub has no runtime attribute")

assert checked_classes >= 25, f"expected at least 25 classes in the stub, found {{checked_classes}}"
assert checked_members >= 80, f"expected at least 80 members across all classes, found {{checked_members}}"
assert not failures, "stub drift:\n" + "\n".join(failures)
print(f"stub OK: {{checked_classes}} classes, {{checked_members}} members, all present at runtime")
"#,
        path = path.to_str().expect("utf8 path"),
    );
    let script_path = write_temp("piperine_t28_drift.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "stub drift-check script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}
