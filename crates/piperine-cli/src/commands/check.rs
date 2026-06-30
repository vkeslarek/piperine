use std::path::{Path, PathBuf};

use piperine_ams::Document;

/// Detect the file format by extension.
///
/// `.va`/`.vams` → Verilog-A/AMS, parsed with `piperine_ams::Document::parse_file`.
/// `.phdl`/`.ppr` → PHDL, parsed with `piperine_lang::parse_and_elaborate`.
/// All other extensions fall back to Verilog-A.
fn detect_format(path: &Path) -> FileFormat {
    match path.extension().and_then(|s| s.to_str()) {
        Some("phdl") | Some("ppr") => FileFormat::Ppr,
        _                        => FileFormat::Ams,
    }
}

enum FileFormat { Ams, Ppr }

/// Returns the list of model names captured by `path`, or exits with a
/// message on parse failure.  Used by both the CLI and the test suite.
pub fn check_file(path: &Path) -> Result<CheckSummary, String> {
    println!("Checking file: {}", path.display());
    match detect_format(path) {
        FileFormat::Ams => {
            let doc = Document::parse_file(path)
                .map_err(|e| format!("parse failed: {e}"))?;
            let module_names: Vec<String> = doc.modules.iter().map(|m| m.name.clone()).collect();
            println!("  AMS modules: {}", doc.modules.len());
            for m in &doc.modules {
                println!(
                    "    - {} (ports: {}, parameters: {}, analog_blocks: {}, instances: {})",
                    m.name, m.ports.len(), m.parameters.len(),
                    m.analog_blocks.len(), m.instances.len()
                );
            }
            Ok(CheckSummary::Ams { module_names })
        }
        FileFormat::Ppr => {
            let body = std::fs::read_to_string(path)
                .map_err(|e| format!("read failed: {e}"))?;
            let elab = piperine_lang::parse_and_elaborate(&body)
                .map_err(|e| format!("parse/elab failed: {e}"))?;
            let module_names: Vec<String> =
                elab.modules.keys().cloned().collect();
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
    Ams  { module_names: Vec<String> },
    Ppr  { module_names: Vec<String> },
}

pub fn execute(file: Option<String>) {
    let path = if let Some(f) = file {
        PathBuf::from(f)
    } else {
        if let Some(root) = piperine_project::get_current_project_root() {
            let toml_path = root.join("Piperine.toml");
            match piperine_project::PiperineToml::load(&toml_path) {
                Ok(toml) => {
                    println!("Loaded project: {} v{}", toml.project.name, toml.project.version);
                    // Default to src/main.vams (legacy) or src/main.phdl.
                    let ppr = root.join("src").join("main.phdl");
                    if ppr.exists() { ppr } else { root.join("src").join("main.vams") }
                }
                Err(e) => {
                    eprintln!("Error loading Piperine.toml: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("Error: No Piperine.toml found in current or parent directories. Please provide a file.");
            std::process::exit(1);
        }
    };

    if let Err(e) = check_file(&path) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
