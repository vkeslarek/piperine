use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashMap;

pub mod git;
pub mod lockfile;
pub mod resolver;
pub mod source_map;

pub use source_map::project_source_map;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiperineToml {
    pub project: Project,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencySource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencySource {
    Git(GitDependency),
    Path(PathDependency),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathDependency {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitDependency {
    pub git: String,
    pub version: Option<String>,
    pub branch: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GitRequirement {
    Version(String),
    Branch(String),
    Rev(String),
    Latest,
}

impl GitDependency {
    pub fn requirement(&self) -> GitRequirement {
        if let Some(ref v) = self.version {
            GitRequirement::Version(v.clone())
        } else if let Some(ref b) = self.branch {
            GitRequirement::Branch(b.clone())
        } else if let Some(ref r) = self.rev {
            GitRequirement::Rev(r.clone())
        } else {
            GitRequirement::Latest
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub version: String,
    pub authors: Vec<String>,
    pub edition: String,
}

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Failed to serialize TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
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
            dependencies: HashMap::new(),
        }
    }

    pub fn to_string_pretty(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let content = fs::read_to_string(path)?;
        let project = toml::from_str(&content)?;
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let content = self.to_string_pretty()?;
        fs::write(path, content)?;
        Ok(())
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
