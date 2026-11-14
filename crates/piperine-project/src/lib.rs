//! Packages: the manifest, the dependencies it names and the lockfile that pins them.

use std::path::{Path, PathBuf};

use piperine_lang::pom::Design;

pub mod error;
pub mod git;
pub mod lockfile;
pub mod manifest;
pub mod resolver;

pub use error::PackageError;
pub use lockfile::{LockedPackage, Lockfile};
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

/// Resolve the package at `dir` and read every crate of its graph, `std` last. A package error
/// returns no design; a language error is a diagnostic of the design that is returned.
pub fn load_project(dir: &Path) -> Result<Design, Vec<PackageError>> {
    let packages = resolve(dir, Options::default())?;
    Ok(Design::load_crates(packages.graph))
}
