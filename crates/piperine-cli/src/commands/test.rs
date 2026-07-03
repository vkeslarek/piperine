use std::path::PathBuf;

use piperine_bench::{BenchOutcome, BenchRunner};

/// Discover every `.phdl` file the same way `check` does: an explicit
/// `file`, or every file under `<project>/src`.
fn discover_files(file: Option<String>) -> Vec<PathBuf> {
    if let Some(f) = file {
        return vec![PathBuf::from(f)];
    }
    let Some(root) = piperine_project::get_current_project_root() else {
        eprintln!("Error: No Piperine.toml found in current or parent directories. Please provide a file.");
        std::process::exit(1);
    };
    let src_dir = root.join("src");
    let mut paths = Vec::new();
    if src_dir.exists() {
        let mut stack = vec![src_dir];
        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("phdl") {
                        paths.push(p);
                    }
                }
            }
        }
    }
    if paths.is_empty() {
        eprintln!("Error: No .phdl files found in src/ directory.");
        std::process::exit(1);
    }
    paths
}

fn source_map(project_root: &std::path::Path) -> piperine_lang::SourceMap {
    let mut source_map = piperine_lang::SourceMap::new(project_root.to_path_buf());
    let headers_dir = project_root.join("headers");
    if headers_dir.exists() {
        source_map = source_map.with_prelude(headers_dir.join("prelude.phdl"));
        source_map.add_namespace("piperine", headers_dir.clone());
        source_map.add_namespace("spice", headers_dir.join("ngspice"));
    }
    source_map
}

pub fn execute(file: Option<String>) {
    let project_root = piperine_project::get_current_project_root().unwrap_or_else(|| std::env::current_dir().unwrap());
    let source_map = source_map(&project_root);

    let mut had_failure = false;
    let mut ran_any = false;
    for path in discover_files(file) {
        let body = match std::fs::read_to_string(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading {}: {e}", path.display());
                had_failure = true;
                continue;
            }
        };
        let design = match piperine_lang::parse_and_elaborate(&body, &source_map) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error elaborating {}: {e:?}", path.display());
                had_failure = true;
                continue;
            }
        };
        let report = BenchRunner::new(&design).run_all();
        for result in &report.results {
            ran_any = true;
            match &result.outcome {
                BenchOutcome::Passed => println!("ok   {}::{}::{}", path.display(), result.module, result.entry),
                BenchOutcome::Failed(msg) => {
                    println!("FAIL {}::{}::{} — {msg}", path.display(), result.module, result.entry);
                    had_failure = true;
                }
                BenchOutcome::Error(msg) => {
                    println!("ERR  {}::{}::{} — {msg}", path.display(), result.module, result.entry);
                    had_failure = true;
                }
            }
        }
    }

    if !ran_any {
        println!("No bench entry points found.");
    }
    if had_failure {
        std::process::exit(1);
    }
}
