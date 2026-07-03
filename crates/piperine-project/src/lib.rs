use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiperineToml {
    pub project: Project,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub edition: String,
}

impl PiperineToml {
    pub fn new(name: &str) -> Self {
        Self {
            project: Project {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                authors: vec![],
                edition: "2024".to_string(),
            },
        }
    }

    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse TOML: {}", e))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let content = self.to_string_pretty().map_err(|e| format!("Failed to serialize TOML: {}", e))?;
        fs::write(path, content).map_err(|e| format!("Failed to write file: {}", e))
    }
}

/// Finds the root directory of the current project by looking for Piperine.toml.
/// Starts from `current_dir` and traverses up the directory tree.
pub fn find_project_root(current_dir: &Path) -> Option<PathBuf> {
    let mut dir = current_dir.to_path_buf();
    loop {
        if dir.join("Piperine.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Helper to get the project root starting from the current working directory.
pub fn get_current_project_root() -> Option<PathBuf> {
    env::current_dir().ok().and_then(|d| find_project_root(&d))
}
