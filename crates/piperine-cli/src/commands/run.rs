use piperine_bench::BenchRunner;
use std::path::PathBuf;

pub fn execute(entry: Option<String>, file: Option<String>) {
    // Python script path: `piperine run foo.py` embeds CPython and runs the
    // script with `import piperine` available (PY-15 / spec AC16). Detected
    // by `.py` suffix on the positional `entry` arg so no extra flag is
    // needed; the bench flow below is untouched for `.phdl`/`module::fn` use.
    if entry.as_deref().is_some_and(|e| e.ends_with(".py")) {
        let path = entry.expect("checked above");
        if let Err(e) = piperine_python::embed::run_script(&path) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    crate::commands::build::execute(file.clone());

    let (source_map, project_root) = super::utils::build_source_map();

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

    let plugin_host = super::utils::load_plugin_host(&project_root);
    if let Err(e) = plugin_host.fire_after_parse(&body) {
        eprintln!("Plugin error: {e}");
        std::process::exit(1);
    }
    let mut design = match piperine_lang::parse_and_elaborate_seeded(&body, &source_map, |ctx| {
        plugin_host.seed_schemas(ctx);
    }) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error elaborating {}:\n{:?}", path.display(), e);
            std::process::exit(1);
        }
    };
    super::utils::stamp_project_meta(&mut design, &project_root);
    if let Err(e) = plugin_host.fire_after_elaborate(&design) {
        eprintln!("Plugin error: {e}");
        std::process::exit(1);
    }

    let mut runner = BenchRunner::new(&design);
    if !plugin_host.is_empty() {
        runner = runner
            .with_device_provider(plugin_host.clone())
            .with_plugins(plugin_host.clone());
    }

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
