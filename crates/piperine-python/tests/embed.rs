//! P11 — embedded-interpreter test for [`piperine_python::embed::run_script`]
//! (PY-15 / spec AC16/17). Verifies `import piperine` resolves in a fresh
//! embedded interpreter (no pip install) and that script exceptions
//! propagate as `PyErr`.
//!
//! One test function — `run_script` performs the global Python init
//! (`append_to_inittab` + `prepare_freethreaded_python`); running both
//! assertions in a single test keeps the init sequential and avoids a race
//! on the process-global interpreter state.

use piperine_python::embed::run_script;

/// Write `body` to a temp file named `name`; return its path as a `String`.
/// Test-only helper — `expect` is fine here (the path is test-controlled,
/// not a production user-input path).
fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp script");
    path.to_str().expect("non-utf8 temp path").to_string()
}

/// PY-15 / spec AC16+AC17: `piperine run foo.py` registers `_piperine` +
/// the `piperine` facade in a fresh interpreter, so `import piperine`
/// resolves with no pip install (AC16), and a script exception propagates
/// as `Err` carrying the diagnostic (AC17 — fail loud, no silent swallow).
#[test]
fn run_script_imports_piperine_and_propagates_errors() {
    // AC16: `import piperine` resolves and exposes the documented surface.
    // The facade's `import _piperine` succeeds via the inittab registration;
    // the typed re-exports (load, TranConfig) prove the facade materialized.
    let import_path = write_temp(
        "piperine_embed_import_test.py",
        "import piperine\nassert hasattr(piperine, 'load')\nassert hasattr(piperine, 'TranConfig')\n",
    );
    let result = run_script(&import_path);
    assert!(
        result.is_ok(),
        "import piperine must resolve (AC16), got: {:?}",
        result.err()
    );
    let _ = std::fs::remove_file(&import_path);

    // AC17: a Python exception propagates as `Err` — the CLI surfaces it to
    // stderr + non-zero exit (no silent swallow).
    let boom_path = write_temp(
        "piperine_embed_error_test.py",
        "raise RuntimeError('boom-from-embedded-script')\n",
    );
    let err = run_script(&boom_path).expect_err("script exception must propagate (AC17)");
    let msg = format!("{err}");
    assert!(
        msg.contains("boom-from-embedded-script"),
        "error must carry the script's diagnostic, got: {msg}"
    );
    let _ = std::fs::remove_file(&boom_path);
}
