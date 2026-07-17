//! Example-gallery gate (BRM-11 dual contract): every `examples/*.phdl`
//! elaborates, and every `examples/*.py` runs green in-process through the
//! same embedded-CPython host `piperine run` uses.

use std::fs;
use std::path::PathBuf;

fn examples_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    assert!(dir.exists(), "examples/ directory not found at {}", dir.display());
    dir
}

/// Every file in `examples/` with extension `ext`, sorted for a stable
/// report order. The gallery must never silently shrink to nothing.
fn gallery(ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(examples_dir())
        .expect("read examples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some(ext))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .{ext} files found in examples/");
    files
}

/// Leg 1: every `.phdl` example is a valid circuit module — elaboration is
/// the compile gate (the runnable assertions live in the `.py` twins).
#[test]
fn every_example_phdl_elaborates() {
    let mut failures: Vec<String> = Vec::new();
    for path in gallery("phdl") {
        let body = fs::read_to_string(&path).unwrap();
        if let Err(e) = piperine_lang::parse_and_elaborate(&body, &piperine_lang::SourceMap::dummy()) {
            failures.push(format!("{}: elaboration failed: {e:?}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{} example(s) failed:\n{}", failures.len(), failures.join("\n"));
}

/// Leg 2: every `.py` example runs green through the embedded CPython host.
/// One test function so the scripts execute sequentially — the embedded
/// interpreter is a single shared process (GIL), and parallel scripts would
/// interleave their `__main__` namespaces.
#[test]
fn every_example_python_runs() {
    let mut failures: Vec<String> = Vec::new();
    for path in gallery("py") {
        let script = path.to_str().expect("utf-8 example path");
        if let Err(e) = piperine_python::embed::run_script(script) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{} example(s) failed:\n{}", failures.len(), failures.join("\n"));
}
