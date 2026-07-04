use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiperineLock {
    pub package: Vec<LockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub name: String,
    pub source: String,
    pub hash: String,
}

#[derive(Error, Debug)]
pub enum LockError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Failed to serialize TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl PiperineLock {
    pub fn new() -> Self {
        Self {
            package: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Option<Self>, LockError> {
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let lock: PiperineLock = toml::from_str(&content)?;
        Ok(Some(lock))
    }

    pub fn save(&mut self, path: &Path) -> Result<(), LockError> {
        // Sort entries for deterministic output
        self.package.sort_by(|a, b| a.name.cmp(&b.name));

        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_hash(&self, name: &str) -> Option<String> {
        self.package
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.hash.clone())
    }
}
