use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiperineLock {
    pub package: Vec<LockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    #[default]
    Dependency,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub name: String,
    pub source: String,
    pub hash: String,
    /// Discriminates PHDL library deps from plugin artifacts. Defaults to
    /// `Dependency` so pre-plugin lockfiles parse unchanged.
    #[serde(default, skip_serializing_if = "is_dependency")]
    pub kind: EntryKind,
    /// Plugin-only: sha256 of the loaded artifact. A change forces TOFU
    /// re-approval (SPEC Part VI §5.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Plugin-only: the manifest-declared ABI at trust time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
    /// Plugin-only: RFC3339 timestamp of the TOFU approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_at: Option<String>,
}

fn is_dependency(kind: &EntryKind) -> bool {
    *kind == EntryKind::Dependency
}

impl LockEntry {
    /// A plain dependency entry (the pre-plugin shape).
    pub fn dependency(name: String, source: String, hash: String) -> Self {
        Self { name, source, hash, kind: EntryKind::Dependency, content_hash: None, abi: None, trusted_at: None }
    }
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
            .find(|p| p.name == name && p.kind == EntryKind::Dependency)
            .map(|p| p.hash.clone())
    }

    /// The trusted plugin entry for `name`, if one was recorded.
    pub fn plugin_entry(&self, name: &str) -> Option<&LockEntry> {
        self.package.iter().find(|p| p.name == name && p.kind == EntryKind::Plugin)
    }

    /// Record (or replace) a plugin trust decision.
    pub fn record_plugin(&mut self, entry: LockEntry) {
        self.package.retain(|p| !(p.kind == EntryKind::Plugin && p.name == entry.name));
        self.package.push(entry);
    }
}
