use piperine_bench::BenchRunner;
use std::path::PathBuf;

pub fn execute(entry: Option<String>, file: Option<String>) {
    crate::commands::build::execute(file.clone());

    let (source_map, _project_root) = super::utils::build_source_map();

    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            root.join("src").join("main.phdl")
        } else {
            eprintln!("Error: No Piperine.toml found. Please provide a file.");
            std::process::exit(1);
        }
    };

    let body = match std::fs::read_to_string(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading {}: {}", path.display(), e);
            std::process::exit(1);
        }
    };

    let design = match piperine_lang::parse_and_elaborate(&body, &source_map) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error elaborating {}:\n{:?}", path.display(), e);
            std::process::exit(1);
        }
    };

    let runner = BenchRunner::new(&design);

    if let Some(e) = entry {
        let parts: Vec<&str> = e.split("::").collect();
        if parts.len() != 2 {
            eprintln!(
                "Error: entry point must be in the form `module::fn` (e.g. `my_bench::main`)"
            );
            std::process::exit(1);
        }
        let module = parts[0];
        let func = parts[1];

        println!("Running {}::{}...", module, func);
        match runner.run_entry(module, func) {
            piperine_bench::BenchOutcome::Passed => {
                println!("Success.");
            }
            piperine_bench::BenchOutcome::Failed(msg) => {
                eprintln!("Failed: {}", msg);
                std::process::exit(1);
            }
            piperine_bench::BenchOutcome::Error(msg) => {
                eprintln!("Error: {}", msg);
                std::process::exit(1);
            }
        }
    } else {
        println!("Running all bench entry points in {}...", path.display());
        let report = runner.run_all();
        let mut had_failure = false;
        let mut ran_any = false;
        for result in &report.results {
            ran_any = true;
            match &result.outcome {
                piperine_bench::BenchOutcome::Passed => {
                    println!("ok   {}::{}", result.module, result.entry)
                }
                piperine_bench::BenchOutcome::Failed(msg) => {
                    println!("FAIL {}::{} — {}", result.module, result.entry, msg);
                    had_failure = true;
                }
                piperine_bench::BenchOutcome::Error(msg) => {
                    println!("ERR  {}::{} — {}", result.module, result.entry, msg);
                    had_failure = true;
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
}
