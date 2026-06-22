use std::fs;
use std::path::{Path, PathBuf};

use piperine_parser::parse_file;

/// A fixture is a *standalone* compilation unit only if it declares a module
/// (or a top-level discipline/nature). Many `.va` files in these libraries are
/// include fragments (macro/variable/analog-body snippets) meant to be pulled
/// into a parent model with `` `include ``; they cannot be parsed alone.
fn is_standalone(content: &str) -> bool {
    content.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("module ")
            || t.starts_with("macromodule ")
            || t.starts_with("discipline ")
            || t.starts_with("nature ")
    })
}

/// Resolve the original on-disk location of a fixture so that its `` `include ``
/// directives (which reference sibling files lost in the flattened fixture
/// tree) resolve. Looks under `$CVAF_MODEL_ROOTS` (colon-separated) or the
/// default upstream checkout locations. Returns `None` when no source tree is
/// available, in which case the fixture is parsed in place.
fn original_of(base: &str) -> Option<PathBuf> {
    let roots = std::env::var("CVAF_MODEL_ROOTS").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{home}/Git/VA-Models:{home}/Git/Verilog-A-photonic-model-library")
    });
    for root in roots.split(':').filter(|s| !s.is_empty()) {
        let out = std::process::Command::new("find")
            .arg(root)
            .arg("-name")
            .arg(base)
            .output()
            .ok()?;
        if let Some(line) = String::from_utf8_lossy(&out.stdout).lines().next() {
            if !line.is_empty() {
                return Some(PathBuf::from(line));
            }
        }
    }
    None
}

fn test_models_in_dir(dir: &Path) {
    if !dir.exists() {
        return;
    }
    let mut failures = Vec::new();
    let mut ok = 0usize;
    let mut skipped = 0usize;
    for entry in fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("va") {
            continue;
        }
        let content = fs::read_to_string(&path).expect("read file");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !is_standalone(&content) {
            skipped += 1;
            continue;
        }
        // Prefer the original location (with its include siblings); fall back to
        // the in-tree fixture for self-contained models.
        let target = original_of(&name).unwrap_or_else(|| path.clone());
        match parse_file(&target) {
            Ok(_) => {
                ok += 1;
                println!("✅ {name}");
            }
            Err(err) => {
                let first = err.lines().next().unwrap_or("").to_string();
                println!("❌ {name}: {first}");
                failures.push(format!("{name}: {first}"));
            }
        }
    }
    println!("\n{} ok, {} skipped (fragments), {} failed", ok, skipped, failures.len());
    if !failures.is_empty() {
        panic!("failed to parse {} models:\n{:#?}", failures.len(), failures);
    }
}

#[test]
fn test_va_models() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/VA-Models");
    test_models_in_dir(&dir);
}

#[test]
fn test_veriloga_lib() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verilogaLib");
    test_models_in_dir(&dir);
}

#[test]
fn test_photonic_model_library() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/Verilog-A-photonic-model-library");
    test_models_in_dir(&dir);
}
