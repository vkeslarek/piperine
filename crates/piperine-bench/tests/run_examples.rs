//! Runs every `bench` entry point in `examples/*.phdl` (the showcase gallery
//! also exercised by `piperine-lang`'s `run_examples_test.rs`, which only
//! checks that they compile). This is the "run tudo" end-to-end gate: every
//! example must both compile *and* pass its bench.

use std::fs;
use std::path::PathBuf;

use piperine_bench::{BenchOutcome, BenchRunner};

#[test]
fn all_example_benches_pass() {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf();
    let examples_dir = workspace_root.join("examples");
    assert!(examples_dir.exists(), "examples/ directory not found at {:?}", examples_dir);

    let mut phdl_files: Vec<PathBuf> = fs::read_dir(&examples_dir)
        .expect("read examples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("phdl"))
        .collect();
    phdl_files.sort();
    assert!(!phdl_files.is_empty(), "no .phdl files found in examples/");

    let mut ran = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for path in &phdl_files {
        let body = fs::read_to_string(path).unwrap();
        let design = match piperine_lang::parse_and_elaborate(&body, &piperine_lang::SourceMap::dummy()) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("{}: elaboration failed: {e:?}", path.display()));
                continue;
            }
        };
        let report = BenchRunner::new(&design).run_all();
        for result in &report.results {
            ran += 1;
            match &result.outcome {
                BenchOutcome::Passed => {}
                BenchOutcome::Failed(msg) => {
                    failures.push(format!("{}: {}::{} FAILED — {msg}", path.display(), result.module, result.entry))
                }
                BenchOutcome::Error(msg) => {
                    failures.push(format!("{}: {}::{} ERROR — {msg}", path.display(), result.module, result.entry))
                }
            }
        }
    }

    assert!(ran > 0, "no bench entry points found across any example");
    assert!(failures.is_empty(), "{} bench failure(s):\n{}", failures.len(), failures.join("\n"));
}
