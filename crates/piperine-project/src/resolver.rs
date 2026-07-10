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
                    resolved.insert(name.clone(), target_dir);
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

                    self.resolved_paths
                        .insert(pkg_name.clone(), target_dir.clone());

                    // Recursively resolve
                    let manifest_path = target_dir.join("Piperine.toml");
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
