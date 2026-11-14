//! Packages: the manifest, the dependencies it names and the lockfile that pins them.

use std::path::{Path, PathBuf};

pub mod error;
pub mod git;
pub mod manifest;
pub mod resolver;

pub use error::PackageError;
pub use manifest::{Dependency, GitRef, Manifest, Package, Requirement, Source, Version};
pub use resolver::{Options, PackageGraph, ResolvedPackage, resolve};

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
