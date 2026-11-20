//! `Manifests`: what the crates of this repository say they depend on, read from their `Cargo.toml`,
//! so a chapter test can hold a sentence about which crate knows which.

use std::path::Path;

/// The manifests of the repository's crates.
pub struct Manifests;

impl Manifests {
    /// The names listed under `[dependencies]` of the crate `crate_name` (a directory of `crates/`). Only
    /// the plain `[dependencies]` table counts: a dev-dependency is a test's, not the crate's.
    pub fn dependencies_of(crate_name: &str) -> Vec<String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("crates").join(crate_name).join("Cargo.toml");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut in_dependencies = false;
        let mut names = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_dependencies = line == "[dependencies]";
            } else if in_dependencies {
                let name = line.split(['=', '.']).next().unwrap_or("").trim();
                if !name.is_empty() && !name.starts_with('#') {
                    names.push(name.to_string());
                }
            }
        }
        names
    }

    /// Whether the crate `crate_name` lists `dependency` under `[dependencies]`.
    pub fn depends_on(crate_name: &str, dependency: &str) -> bool {
        Self::dependencies_of(crate_name).iter().any(|name| name == dependency)
    }
}
