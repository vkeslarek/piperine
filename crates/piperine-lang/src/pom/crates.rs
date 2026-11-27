//! The crate graph: the packages of a build as plain data, which the front end is built from
//! (`Design::load_crates`, `DesignBuilder::crates`). Nothing here reads a manifest or a file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::ids::{CrateId, DeclId, FileId};
use crate::msl::lexer::{Lexer, Token};

/// One crate of a graph: its name, its root file and its named dependencies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrateNode {
    pub name: String,
    /// The crate root file.
    pub root: PathBuf,
    pub dependencies: Vec<DependencyData>,
}

/// A crate as the design stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrateData {
    pub name: String,
    /// The file the crate is read from.
    pub root_file: FileId,
    /// The root `Namespace` declaration: the root file's.
    pub root: DeclId,
    /// As declared; `std` is implicit and not listed.
    pub dependencies: Vec<DependencyData>,
}

/// An edge of the graph: the name a crate gives a dependency, and the crate it names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyData {
    pub name: String,
    pub krate: CrateId,
}

/// Why a crate or a dependency was refused. A refused call leaves the graph unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// Not an identifier, a keyword, or `std`.
    InvalidName { name: String },
    /// The crate already has a dependency of this name.
    DuplicateDependency { krate: String, name: String },
    /// The id does not belong to the graph.
    UnknownCrate { id: CrateId },
    /// The edge would close a cycle; `names` is the cycle, first crate repeated last.
    Cycle { names: Vec<String> },
}

/// The crates of a build, in insertion order; the first is the root crate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrateGraph {
    crates: Vec<CrateNode>,
}

impl CrateGraph {
    pub fn new() -> CrateGraph {
        CrateGraph::default()
    }

    /// Add a crate; the first one added is the root crate.
    pub fn add_crate(&mut self, name: &str, root: impl Into<PathBuf>) -> Result<CrateId, GraphError> {
        Self::check_name(name)?;
        self.crates.push(CrateNode { name: name.to_string(), root: root.into(), dependencies: Vec::new() });
        Ok(CrateId(self.crates.len() as u32 - 1))
    }

    /// Give `from` a dependency called `name` on `to`.
    pub fn add_dependency(&mut self, from: CrateId, name: &str, to: CrateId) -> Result<(), GraphError> {
        for id in [from, to] {
            if id.index() >= self.crates.len() {
                return Err(GraphError::UnknownCrate { id });
            }
        }
        Self::check_name(name)?;
        if self.crates[from.index()].dependencies.iter().any(|d| d.name == name) {
            return Err(GraphError::DuplicateDependency { krate: self.crates[from.index()].name.clone(), name: name.to_string() });
        }
        if let Some(path) = self.path(to, from) {
            let mut names: Vec<String> = path.iter().map(|id| self.crates[id.index()].name.clone()).collect();
            names.push(self.crates[to.index()].name.clone());
            return Err(GraphError::Cycle { names });
        }
        self.crates[from.index()].dependencies.push(DependencyData { name: name.to_string(), krate: to });
        Ok(())
    }

    pub fn crates(&self) -> &[CrateNode] {
        &self.crates
    }

    /// A dependency path from `start` to `goal` (both included; `[start]` when they are equal),
    /// found by a breadth-first walk over the stored edges.
    fn path(&self, start: CrateId, goal: CrateId) -> Option<Vec<CrateId>> {
        let mut came_from: Vec<Option<CrateId>> = vec![None; self.crates.len()];
        let mut seen = vec![false; self.crates.len()];
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start.index()] = true;
        while let Some(at) = queue.pop_front() {
            if at == goal {
                let mut path = vec![at];
                while let Some(prev) = came_from[path[path.len() - 1].index()] {
                    path.push(prev);
                }
                path.reverse();
                return Some(path);
            }
            for dep in &self.crates[at.index()].dependencies {
                if !seen[dep.krate.index()] {
                    seen[dep.krate.index()] = true;
                    came_from[dep.krate.index()] = Some(at);
                    queue.push_back(dep.krate);
                }
            }
        }
        None
    }

    /// Is `name` a crate or dependency name: one identifier, no keyword, not `std`.
    pub fn is_crate_name(name: &str) -> bool {
        if name == "std" {
            return false;
        }
        let (tokens, diagnostics) = Lexer::lex(name);
        diagnostics.is_empty() && matches!(tokens.as_slice(), [(Token::Ident(text), _)] if text == name)
    }

    /// The name of the crate whose root file is `path`: the file stem, with every character that is
    /// no ASCII letter, digit or `_` replaced by `_` and a `_` before a leading digit. A stem that is
    /// then a keyword, `std` or `_` gets a `_` appended, and is returned as the second value so that the
    /// design can report it (`E0171`).
    pub(super) fn crate_name_of(path: &Path) -> (String, Option<String>) {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let mut name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
        if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
            name.insert(0, '_');
        }
        // A file with no stem at all (the empty design's synthetic root) is `_` by construction and
        // reports nothing; a stem that is itself `_` is reserved like any other.
        if Self::is_crate_name(&name) || stem.is_empty() {
            return (name, None);
        }
        let reserved = name.clone();
        name.push('_');
        (name, Some(reserved))
    }

    /// `name` when it is a crate name, else the error that says so.
    fn check_name(name: &str) -> Result<(), GraphError> {
        if Self::is_crate_name(name) { Ok(()) } else { Err(GraphError::InvalidName { name: name.to_string() }) }
    }
}
