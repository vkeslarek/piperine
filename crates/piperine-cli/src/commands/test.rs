//! `piperine test` — discovers and runs the project's Python testbenches
//! (`*_tb.py`). Each file runs in a fresh subprocess through the same
//! embedded-CPython path as `piperine run` (isolation + a killable timeout);
//! a non-zero script exit (a raised exception) marks the file failed and its
//! traceback is shown.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Per-file wall-clock budget. Default 300 s (design: "generous");
/// `PIPERINE_TEST_TIMEOUT_SECS` overrides (integration tests use a couple of
/// seconds to exercise the kill path).
fn timeout() -> Duration {
    std::env::var("PIPERINE_TEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(300))
}

/// Directories never recursed into: virtualenvs and build artifacts (design),
/// plus VCS internals (huge, never testbenches).
const SKIP_DIRS: [&str; 3] = [".venv", "target", ".git"];

/// Every `*_tb.py` under `root`, recursively, skipping [`SKIP_DIRS`] —
/// sorted for a stable run order. Public for integration tests.
pub fn discover_testbenches(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                if !SKIP_DIRS.contains(&name.to_string_lossy().as_ref()) {
                    stack.push(path);
                }
            } else if path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.ends_with("_tb.py"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// One testbench's outcome: the wall-clock it took and the failure output
/// (traceback / timeout notice) when it did not pass.
enum Outcome {
    Passed,
    Failed(String),
}

/// Run one testbench in a fresh subprocess (`piperine run <file>` — the same
/// embedded-CPython path), capturing output to scratch files so a chatty
/// script cannot deadlock on a full pipe. Killed and failed on timeout.
fn run_one(file: &Path, exe: &Path, budget: Duration) -> Outcome {
    let scratch = std::env::temp_dir().join(format!("piperine-tb-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&scratch);
    let stdout_path = scratch.join("stdout");
    let stderr_path = scratch.join("stderr");
    let (stdout, stderr) = match (
        std::fs::File::create(&stdout_path),
        std::fs::File::create(&stderr_path),
    ) {
        (Ok(o), Ok(e)) => (o, e),
        _ => return Outcome::Failed(format!("could not create scratch files in {}", scratch.display())),
    };
    let spawn = std::process::Command::new(exe)
        .arg("run")
        .arg(file)
        .stdout(stdout)
        .stderr(stderr)
        .spawn();
    let mut child = match spawn {
        Ok(c) => c,
        Err(e) => return Outcome::Failed(format!("could not spawn `{} run`: {e}", exe.display())),
    };
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() > budget => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Outcome::Failed(format!("wait failed: {e}")),
        }
    };
    let output = || -> String {
        let mut s = String::new();
        if let Ok(o) = std::fs::read_to_string(&stdout_path) {
            s.push_str(&o);
        }
        if let Ok(e) = std::fs::read_to_string(&stderr_path) {
            s.push_str(&e);
        }
        s.trim().to_string()
    };
    match status {
        Ok(status) if status.success() => Outcome::Passed,
        Ok(status) => Outcome::Failed(format!("exit {}\n{}", status.code().unwrap_or(-1), output())),
        Err(()) => Outcome::Failed(format!(
            "timeout after {} s — killed\n{}",
            budget.as_secs(),
            output()
        )),
    }
}

pub fn execute(list: bool, file: Option<String>) {
    let root = piperine_project::get_current_project_root()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    let testbenches = match file {
        Some(f) => vec![PathBuf::from(f)],
        None => discover_testbenches(&root),
    };

    if testbenches.is_empty() {
        println!("No Python testbenches (`*_tb.py`) found under {}.", root.display());
        return;
    }
    if list {
        for tb in &testbenches {
            println!("{}", tb.display());
        }
        return;
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("piperine"));
    let budget = timeout();
    let mut failures = 0usize;
    for tb in &testbenches {
        match run_one(tb, &exe, budget) {
            Outcome::Passed => println!("PASS {}", tb.display()),
            Outcome::Failed(report) => {
                failures += 1;
                println!("FAIL {}\n{report}", tb.display());
            }
        }
    }
    println!(
        "{} run, {} passed, {} failed",
        testbenches.len(),
        testbenches.len() - failures,
        failures
    );
    if failures > 0 {
        std::process::exit(1);
    }
}
