use std::path::PathBuf;

use super::utils::build_source_map;

/// Find every `.phdl` file under `root/src` (mirrors `check::execute`'s
/// project-wide discovery).
fn discover_phdl_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let src_dir = root.join("src");
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
    paths
}

/// `piperine build`: elaborate the target PHDL, then compile every zero-port
/// module (a module with no `inout`/`input`/`output` ports — the only
/// structural signal a self-contained circuit root carries today, matching
/// every example's `mod Board()`/`mod Top()` convention) through the full
/// codegen pipeline (`lower_bodies` + `CircuitCompiler::build_circuit`).
/// A zero-port module is the only thing that can stand as a `CircuitCompiler`
/// root; a project with none is a library — not an error, just nothing to
/// build (PB-04).
pub fn execute(file: Option<String>) {
    let (source_map, project_root) = build_source_map();

    let target_paths = if let Some(f) = file {
        vec![PathBuf::from(f)]
    } else {
        let paths = discover_phdl_files(&project_root);
        if paths.is_empty() {
            eprintln!("Error: No .phdl files found in src/ directory.");
            std::process::exit(1);
        }
        paths
    };

    let mut had_error = false;
    let mut built_any = false;

    for path in &target_paths {
        println!("Building design for: {}", path.display());
        let body = match std::fs::read_to_string(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading {}: {e}", path.display());
                had_error = true;
                continue;
            }
        };

        let design = match piperine_lang::parse_and_elaborate(&body, &source_map) {
            Ok(d) => d,
            Err(report) => {
                eprintln!("Elaboration failed in {}:\n{:?}", path.display(), report);
                had_error = true;
                continue;
            }
        };

        let bodies = match piperine_codegen::resolve::lower_bodies(&design) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Codegen failed in {}: {e}", path.display());
                had_error = true;
                continue;
            }
        };

        let zero_port_modules: Vec<&str> = design
            .modules()
            .filter(|m| m.ports().is_empty())
            .map(|m| m.name())
            .collect();

        if zero_port_modules.is_empty() {
            println!("  (no zero-port module in {} — nothing to build)", path.display());
            continue;
        }

        for module_name in zero_port_modules {
            let mut compiler = piperine_codegen::CircuitCompiler::new(&design, &bodies);
            match compiler.build_circuit(module_name) {
                Ok(_) => {
                    println!("  built `{module_name}`");
                    built_any = true;
                }
                Err(e) => {
                    eprintln!("  `{module_name}` failed to build: {e}");
                    had_error = true;
                }
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
    if !built_any {
        println!("Nothing to build (no zero-port modules found).");
    }
}
