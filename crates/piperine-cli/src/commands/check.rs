use std::path::{Path, PathBuf};

/// Detect the file format by extension.
///
/// `.phdl`/`.ppr` → PHDL, parsed with `piperine_lang::parse_and_elaborate`.
fn detect_format(path: &Path) -> FileFormat {
    match path.extension().and_then(|s| s.to_str()) {
        Some("phdl") | Some("ppr") => FileFormat::Ppr,
        _ => FileFormat::Ams,
    }
}

enum FileFormat {
    Ams,
    Ppr,
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CheckError {
    #[error("AMS format is no longer supported directly. Please use PHDL.")]
    AmsNotSupported,
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("elaboration failed: {0}")]
    Elab(String),
}

pub fn check_file(
    path: &Path,
    source_map: &piperine_lang::SourceMap,
) -> Result<CheckSummary, CheckError> {
    println!("Checking file: {}", path.display());
    match detect_format(path) {
        FileFormat::Ams => Err(CheckError::AmsNotSupported),
        FileFormat::Ppr => {
            let body = std::fs::read_to_string(path)?;
            let elab = piperine_lang::parse_and_elaborate(&body, source_map)
                .map_err(|e| CheckError::Elab(format!("{:?}", e)))?;
            let module_names: Vec<String> = elab.modules().map(|m| m.name().to_string()).collect();
            println!("  PHDL modules: {}", module_names.len());
            for name in &module_names {
                println!("    - {name}");
            }
            Ok(CheckSummary::Ppr { module_names })
        }
    }
}

#[derive(Debug)]
pub enum CheckSummary {
    Ppr { module_names: Vec<String> },
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
                                    } else if p.is_file()
                                        && p.extension().and_then(|s| s.to_str()) == Some("phdl")
                                    {
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
            eprintln!("Error in file {}: {}", path.display(), e);
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
