use std::path::{Path, PathBuf};

/// Recognized source extensions: `.vams`/`.va` (Verilog-AMS/Verilog-A,
/// parsed with `piperine_lang::parse_and_elaborate`).
pub(crate) fn is_recognized_source(path: &Path) -> bool {
    matches!(path.extension().and_then(|s| s.to_str()), Some("vams") | Some("va"))
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("unrecognized source extension (expected .vams or .va)")]
    UnrecognizedExtension,
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("elaboration failed: {0}")]
    Elab(String),
    #[error("elaboration failed:\n{0:?}")]
    ElabReport(miette::Report),
}

pub fn check_file(
    path: &Path,
    source_map: &piperine_lang::SourceMap,
) -> Result<CheckSummary, CheckError> {
    println!("Checking file: {}", path.display());
    if !is_recognized_source(path) {
        return Err(CheckError::UnrecognizedExtension);
    }
    let body = std::fs::read_to_string(path)?;
    let elab = piperine_lang::parse_and_elaborate(&body, source_map)
        .map_err(CheckError::ElabReport)?;
    let module_names: Vec<String> = elab.modules().map(|m| m.name().to_string()).collect();
    println!("  modules: {}", module_names.len());
    for name in &module_names {
        println!("    - {name}");
    }
    Ok(CheckSummary { module_names })
}

#[derive(Debug)]
pub struct CheckSummary {
    pub module_names: Vec<String>,
}

pub fn execute(file: Option<String>) {
    let (source_map, _project_root) = super::utils::build_source_map();

    let target_paths = if let Some(f) = file {
        vec![PathBuf::from(f)]
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            let toml_path = root.join("Piperine.toml");
            match piperine_project::PiperineToml::load(&toml_path) {
                Ok(toml) => {
                    println!(
                        "Loaded project: {} v{}",
                        toml.project.name, toml.project.version
                    );
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
                                    } else if p.is_file() && is_recognized_source(&p) {
                                        paths.push(p);
                                    }
                                }
                            }
                        }
                    }
                    if paths.is_empty() {
                        eprintln!("Error: No .vams/.va files found in src/ directory.");
                        std::process::exit(1);
                    }
                    paths
                }
                Err(e) => {
                    eprintln!("Error loading Piperine.toml: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!(
                "Error: No Piperine.toml found in current or parent directories. Please provide a file."
            );
            std::process::exit(1);
        }
    };

    let mut had_error = false;
    for path in target_paths {
        if let Err(e) = check_file(&path, &source_map) {
            match e {
                CheckError::ElabReport(report) => {
                    eprintln!("Error in file {}:\n{:?}", path.display(), report);
                }
                other => {
                    eprintln!("Error in file {}: {}", path.display(), other);
                }
            }
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
