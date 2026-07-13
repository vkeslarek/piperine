use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::git::{GitError, sync_and_checkout};
use crate::{DependencySource, GitRequirement, PiperineToml};

use crate::lockfile::{LockEntry, LockError, PiperineLock};

#[derive(Error, Debug)]
pub enum ResolverError {
    #[error(
        "Conflict detected for package '{package}'. Cannot satisfy requirement {new_req:?} because {existing_req:?} was already requested."
    )]
    Conflict {
        package: String,
        existing_req: GitRequirement,
        new_req: GitRequirement,
    },
    #[error("Git error for package '{package}': {source}")]
    Git {
        package: String,
        #[source]
        source: GitError,
    },
    #[error("Failed to load Piperine.toml from {path}: {source}")]
    ManifestLoad {
        path: PathBuf,
        #[source]
        source: crate::ProjectError,
    },
    #[error("Failed to process Piperine.lock: {0}")]
    LockfileError(#[from] LockError),
    #[error(
        "Bad `subdir` for package '{package}': {reason} (subdir = {subdir:?})"
    )]
    Subdir {
        package: String,
        subdir: String,
        reason: String,
    },
}

/// Resolve a git dependency's `subdir` inside its checkout: relative,
/// no `..`, and the directory must exist — anything else fails loud.
fn apply_subdir(
    package: &str,
    checkout: &Path,
    subdir: Option<&str>,
) -> Result<PathBuf, ResolverError> {
    let Some(subdir) = subdir else { return Ok(checkout.to_path_buf()) };
    let bad = |reason: &str| ResolverError::Subdir {
        package: package.to_string(),
        subdir: subdir.to_string(),
        reason: reason.to_string(),
    };
    let rel = Path::new(subdir);
    if rel.is_absolute() {
        return Err(bad("must be a relative path inside the repository"));
    }
    if rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(bad("must not contain `..`"));
    }
    let dir = checkout.join(rel);
    if !dir.is_dir() {
        return Err(bad("no such directory in the checked-out repository"));
    }
    Ok(dir)
}

/// A fully resolved dependency map, mapping package names to their local on-disk paths.
pub type ResolvedMap = HashMap<String, PathBuf>;

pub struct Resolver {
    project_root: PathBuf,
    deps_dir: PathBuf,
    resolved_reqs: HashMap<String, GitRequirement>,
    resolved_paths: ResolvedMap,
    visited: HashSet<String>,
    lockfile: PiperineLock,
    update_mode: bool,
}

impl Resolver {
    pub fn new(project_root: &Path, update_mode: bool) -> Self {
        let deps_dir = project_root.join("target").join("deps");
        Self {
            project_root: project_root.to_path_buf(),
            deps_dir,
            resolved_reqs: HashMap::new(),
            resolved_paths: HashMap::new(),
            visited: HashSet::new(),
            lockfile: PiperineLock::new(),
            update_mode,
        }
    }

    /// Resolve the `[plugins]` sources of `root_manifest` into local paths.
    /// Path sources resolve relative to the project root; git sources sync
    /// into `target/plugins/<name>/`. Plugins have no transitive PHDL
    /// dependencies — no recursive walk (SPEC Part VI §5).
    pub fn resolve_plugins(
        &mut self,
        root_manifest: &PiperineToml,
    ) -> Result<ResolvedMap, ResolverError> {
        let plugins_dir = self.project_root.join("target").join("plugins");
        if !root_manifest.plugins.is_empty() {
            std::fs::create_dir_all(&plugins_dir).ok();
        }
        let mut resolved = ResolvedMap::new();
        for (name, source) in &root_manifest.plugins {
            match source {
                DependencySource::Path(path_dep) => {
                    let path = PathBuf::from(&path_dep.path);
                    let path = if path.is_absolute() { path } else { self.project_root.join(path) };
                    resolved.insert(name.clone(), path);
                }
                DependencySource::Git(git_dep) => {
                    let req = git_dep.requirement();
                    let target_dir = plugins_dir.join(name);
                    let target_str = match req {
                        GitRequirement::Version(ref v) => format!("release/v{}", v),
                        GitRequirement::Branch(ref b) => b.clone(),
                        GitRequirement::Rev(ref r) => r.clone(),
                        GitRequirement::Latest => "latest".to_string(),
                    };
                    sync_and_checkout(&git_dep.git, &target_dir, &target_str).map_err(|e| {
                        ResolverError::Git { package: name.clone(), source: e }
                    })?;
                    let dir = apply_subdir(name, &target_dir, git_dep.subdir.as_deref())?;
                    resolved.insert(name.clone(), dir);
                }
            }
        }
        Ok(resolved)
    }

    pub fn resolve(&mut self, root_manifest: &PiperineToml) -> Result<ResolvedMap, ResolverError> {
        if !self.deps_dir.exists() {
            std::fs::create_dir_all(&self.deps_dir).ok();
        }

        let lock_path = self.project_root.join("Piperine.lock");
        if !self.update_mode
            && let Some(existing_lock) = PiperineLock::load(&lock_path)? {
                self.lockfile = existing_lock;
            }

        self.resolve_deps(&root_manifest.dependencies)?;

        self.lockfile.save(&lock_path)?;
        Ok(self.resolved_paths.clone())
    }

    fn resolve_deps(
        &mut self,
        deps: &HashMap<String, DependencySource>,
    ) -> Result<(), ResolverError> {
        // Clone keys to avoid borrowing issues during recursive resolution
        let packages: Vec<(String, DependencySource)> =
            deps.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        for (pkg_name, source) in packages {
            if self.visited.contains(&pkg_name) {
                // Already resolved or in process, skip to avoid infinite loop
                continue;
            }

            match source {
                DependencySource::Path(path_dep) => {
                    // For path dependencies, we assume they are always compatible.
                    let path = PathBuf::from(path_dep.path);
                    self.resolved_paths.insert(pkg_name.clone(), path.clone());
                    self.visited.insert(pkg_name.clone());

                    // Recursively resolve the path dependency
                    let manifest_path = path.join("Piperine.toml");
                    if manifest_path.exists() {
                        let manifest = PiperineToml::load(&manifest_path).map_err(|e| {
                            ResolverError::ManifestLoad {
                                path: manifest_path,
                                source: e,
                            }
                        })?;
                        self.resolve_deps(&manifest.dependencies)?;
                    }
                }
                DependencySource::Git(git_dep) => {
                    let req = git_dep.requirement();

                    // Conflict check
                    if let Some(existing_req) = self.resolved_reqs.get(&pkg_name) {
                        if existing_req != &req {
                            return Err(ResolverError::Conflict {
                                package: pkg_name,
                                existing_req: existing_req.clone(),
                                new_req: req,
                            });
                        }
                        continue;
                    }

                    self.resolved_reqs.insert(pkg_name.clone(), req.clone());
                    self.visited.insert(pkg_name.clone());

                    // Fetch and checkout
                    let target_dir = self.deps_dir.join(&pkg_name);

                    let target_str = match req {
                        GitRequirement::Version(ref v) => format!("release/v{}", v),
                        GitRequirement::Branch(ref b) => b.clone(),
                        GitRequirement::Rev(ref r) => r.clone(),
                        GitRequirement::Latest => "latest".to_string(),
                    };

                    let locked_hash = self.lockfile.get_hash(&pkg_name);
                    let target_to_checkout = locked_hash.as_deref().unwrap_or(&target_str);

                    let commit_hash =
                        sync_and_checkout(&git_dep.git, &target_dir, target_to_checkout).map_err(
                            |e| ResolverError::Git {
                                package: pkg_name.clone(),
                                source: e,
                            },
                        )?;

                    if locked_hash.is_none() {
                        // Update lockfile with the new hash
                        self.lockfile.package.push(LockEntry::dependency(
                            pkg_name.clone(),
                            git_dep.git.clone(),
                            commit_hash,
                        ));
                    }

                    let pkg_dir =
                        apply_subdir(&pkg_name, &target_dir, git_dep.subdir.as_deref())?;
                    self.resolved_paths.insert(pkg_name.clone(), pkg_dir.clone());

                    // Recursively resolve
                    let manifest_path = pkg_dir.join("Piperine.toml");
                    if manifest_path.exists() {
                        let manifest = PiperineToml::load(&manifest_path).map_err(|e| {
                            ResolverError::ManifestLoad {
                                path: manifest_path,
                                source: e,
                            }
                        })?;
                        self.resolve_deps(&manifest.dependencies)?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subdir_none_is_the_checkout_itself() {
        let dir = std::env::temp_dir();
        assert_eq!(apply_subdir("p", &dir, None).unwrap(), dir);
    }

    #[test]
    fn subdir_resolves_inside_the_checkout() {
        let root = std::env::temp_dir().join("piperine-subdir-test");
        let inner = root.join("piperine-spice");
        std::fs::create_dir_all(&inner).unwrap();
        assert_eq!(apply_subdir("spice", &root, Some("piperine-spice")).unwrap(), inner);
    }

    #[test]
    fn subdir_escapes_fail_loud() {
        let root = std::env::temp_dir();
        apply_subdir("p", &root, Some("../outside")).unwrap_err();
        apply_subdir("p", &root, Some("/etc")).unwrap_err();
        apply_subdir("p", &root, Some("no-such-dir-here")).unwrap_err();
    }

    #[test]
    fn subdir_parses_from_toml() {
        let toml = r#"
[project]
name = "demo"
version = "0.1.0"
authors = []
edition = "2024"

[plugins.spice]
git = "https://example.com/plugins"
subdir = "piperine-spice"
"#;
        let manifest: crate::PiperineToml = toml::from_str(toml).unwrap();
        let crate::DependencySource::Git(dep) = &manifest.plugins["spice"] else {
            panic!("expected a git source");
        };
        assert_eq!(dep.subdir.as_deref(), Some("piperine-spice"));
    }
}
