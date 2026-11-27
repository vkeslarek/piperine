//! The resolver: a depth-first walk from the root package that locates every dependency and
//! returns the crate graph. A package is identified by where it is, so a package reached twice
//! is one crate.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use piperine_lang::msl::{Diagnostic, Span};
use piperine_lang::pom::{CrateGraph, CrateId, GraphError};

use crate::error::PackageError;
use crate::git::{GitCheckout, GitError};
use crate::lockfile::{LOCKFILE, LockedPackage, Lockfile};
use crate::manifest::{Dependency, GitRef, MANIFEST, Manifest, Source};

/// What a resolution may do beyond reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Resolve every reference again, ignoring the recorded commits, and rewrite the lockfile.
    pub update: bool,
}

/// The packages of a project as a crate graph, and what each crate was read from.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageGraph {
    /// The root package is the first crate.
    pub graph: CrateGraph,
    /// One per crate of `graph`, in crate order.
    pub packages: Vec<ResolvedPackage>,
}

/// A package the walk located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackage {
    pub krate: CrateId,
    /// The package directory, canonical.
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// The commit of a package fetched by git.
    pub commit: Option<String>,
}

/// Where a package is: the thing two dependencies must share to be one crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Identity {
    Path(PathBuf),
    /// A repository and the commit checked out of it.
    Git(String, String),
}

type Errors = Vec<PackageError>;





struct Walk {
    /// The commit the lockfile records for each git source.
    lock: HashMap<String, String>,
    /// The git packages of this run: what the lockfile will say.
    recorded: BTreeSet<LockedPackage>,
    /// `<root package>/target/deps`: the repositories and checkouts of git dependencies.
    deps: PathBuf,
    /// The commit each git source resolved to in this run.
    resolved: HashMap<String, String>,
    graph: CrateGraph,
    packages: Vec<ResolvedPackage>,
    ids: HashMap<Identity, CrateId>,
    /// The packages being walked, outermost first, with their names.
    stack: Vec<(Identity, String)>,
}

impl Walk {
    /// Make the crate of the package at `dir` and walk its dependencies.
    fn enter(&mut self, dir: &Path, identity: Identity, manifest: Manifest, commit: Option<String>) -> Result<CrateId, Errors> {
        let name = manifest.package.name.clone();
        let id = self.graph.add_crate(&name, Resolver::crate_root(dir)).map_err(|e| vec![Resolver::graph_fault(dir, Span::new(0, 0), &e)])?;
        self.ids.insert(identity.clone(), id);
        self.packages.push(ResolvedPackage { krate: id, dir: dir.to_path_buf(), manifest: manifest.clone(), commit });
        self.stack.push((identity, name));
        for dependency in &manifest.dependencies {
            let child = self.dependency(dir, dependency)?;
            self.graph.add_dependency(id, &dependency.name, child).map_err(|e| vec![Resolver::graph_fault(dir, dependency.name_span, &e)])?;
        }
        self.stack.pop();
        Ok(id)
    }

    /// The directory holding the commit of `url` that `reference` names: the repository is
    /// cloned or fetched into `<deps>/<url>/db`, and the commit checked out into
    /// `<deps>/<url>/<commit>`.
    fn checkout(&mut self, url: &str, reference: &GitRef) -> Result<(PathBuf, String), GitError> {
        let home = self.deps.join(Resolver::sanitize(&GitCheckout::normalize_url(url)));
        let source = GitCheckout::source_text(&GitCheckout::normalize_url(url), reference);
        let commit = match self.resolved.get(&source) {
            Some(commit) => commit.clone(),
            None => {
                let commit = match self.lock.get(&source) {
                    // Pinned, and checked out already: the repository is not opened.
                    Some(locked) if home.join(locked).is_dir() => locked.clone(),
                    Some(locked) => GitCheckout::sync_and_checkout(url, &home.join("db"), &GitRef::Rev(locked.clone()))?,
                    None => GitCheckout::sync_and_checkout(url, &home.join("db"), reference)?,
                };
                self.resolved.insert(source, commit.clone());
                commit
            }
        };
        let checkout = home.join(&commit);
        if !checkout.is_dir() {
            // Checked out beside, then renamed: a failure leaves no commit directory.
            let partial = home.join(format!("{commit}.partial"));
            let _ = std::fs::remove_dir_all(&partial);
            let db = home.join("db");
            let made = GitCheckout::sync_and_checkout(&db.to_string_lossy(), &partial, &GitRef::Rev(commit.clone())).and_then(|_| {
                std::fs::rename(&partial, &checkout).map_err(|e| GitError::CheckoutFailed { path: checkout.clone(), target: commit.clone(), msg: e.to_string() })
            });
            if let Err(e) = made {
                let _ = std::fs::remove_dir_all(&partial);
                return Err(e);
            }
        }
        let checkout = checkout.canonicalize().map_err(|e| GitError::CheckoutFailed { path: checkout.clone(), target: commit.clone(), msg: e.to_string() })?;
        Ok((checkout, commit))
    }

    /// Locate one dependency of the package at `dir`.
    fn dependency(&mut self, dir: &Path, dependency: &Dependency) -> Result<CrateId, Errors> {
        let (target, identity, commit) = match &dependency.source {
            Source::Path(path) => {
                let target = dir.join(path);
                if !target.join(MANIFEST).is_file() {
                    let message = format!("dependency `{}` is not at `{}`: no `{MANIFEST}` there", dependency.name, target.display());
                    return Err(vec![Resolver::fault(dir, dependency.name_span, "E0172", message, "no package at this path", None)]);
                }
                let target = target.canonicalize().map_err(|e| {
                    let message = format!("dependency `{}` is not at `{}`: {e}", dependency.name, target.display());
                    vec![Resolver::fault(dir, dependency.name_span, "E0172", message, "no package at this path", None)]
                })?;
                let identity = Identity::Path(target.clone());
                (target, identity, None)
            }
            Source::Git { url, reference } => {
                let (target, commit) = self.checkout(url, reference).map_err(|e| {
                    let message = format!("dependency `{}` cannot be fetched: {e}", dependency.name);
                    vec![Resolver::fault(dir, dependency.name_span, "E0172", message, "git could not provide this", None)]
                })?;
                if !target.join(MANIFEST).is_file() {
                    let message = format!("dependency `{}` is not at `{url}`: no `{MANIFEST}` at the root of commit {commit}", dependency.name);
                    return Err(vec![Resolver::fault(dir, dependency.name_span, "E0172", message, "no package in this repository", None)]);
                }
                let identity = Identity::Git(GitCheckout::normalize_url(url), commit.clone());
                (target, identity, Some(commit))
            }
        };
        let manifest = Manifest::load(&target)?;
        let wanted = dependency.package.as_deref().unwrap_or(&dependency.name);
        if manifest.package.name != wanted {
            let message = format!("dependency `{}` asks for the package `{wanted}`, but `{}` holds the package `{}`", dependency.name, target.display(), manifest.package.name);
            let help = (dependency.package.is_none()).then(|| format!("name the package with `package = \"{}\"`, or rename the dependency", manifest.package.name));
            return Err(vec![Resolver::fault(dir, dependency.name_span, "E0173", message, "not the package asked for", help)]);
        }
        if let Some(requirement) = dependency.requirement {
            if !requirement.admits(manifest.package.version) {
                let message = format!("dependency `{}` requires version `{requirement}`, but the package `{}` is `{}`", dependency.name, manifest.package.name, manifest.package.version);
                return Err(vec![Resolver::fault(dir, dependency.name_span, "E0173", message, "the version does not satisfy the requirement", None)]);
            }
        }
        if let (Source::Git { url, reference }, Some(commit)) = (&dependency.source, &commit) {
            let (name, version) = (manifest.package.name.clone(), manifest.package.version);
            self.recorded.insert(LockedPackage { name, version, source: GitCheckout::source_text(&GitCheckout::normalize_url(url), reference), commit: commit.clone() });
        }
        if let Some(at) = self.stack.iter().position(|(on_stack, _)| *on_stack == identity) {
            let mut chain: Vec<&str> = self.stack[at..].iter().map(|(_, name)| name.as_str()).collect();
            chain.push(&self.stack[at].1);
            let message = format!("the dependencies form a cycle: {}", chain.join(" -> "));
            return Err(vec![Resolver::fault(dir, dependency.name_span, "E0174", message, "closes the cycle", None)]);
        }
        if !Resolver::crate_root(&target).is_file() {
            let message = format!("the package `{}` has no crate root `{}`", manifest.package.name, Resolver::crate_root(&target).display());
            return Err(vec![Resolver::fault(dir, dependency.name_span, "E0175", message, "this package has no `src/lib.msl`", None)]);
        }
        match self.ids.get(&identity) {
            Some(id) => Ok(*id),
            None => self.enter(&target, identity, manifest, commit),
        }
    }
}

/// The resolution of a project's dependencies into a crate graph.
pub struct Resolver;

impl Resolver {
    /// The crate root of a package directory.
    fn crate_root(dir: &Path) -> PathBuf {
        dir.join("src").join("lib.msl")
    }

    /// Resolve the package at `dir` and everything it depends on.
    pub fn resolve(dir: &Path, options: Options) -> Result<PackageGraph, Vec<PackageError>> {
        let manifest = Manifest::load(dir)?;
        let root = dir.canonicalize().map_err(|e| {
            let message = format!("cannot read the package directory `{}`: {e}", dir.display());
            vec![Resolver::fault(dir, Span::new(0, 0), "E0170", message, "a package holds a `Piperine.toml`", None)]
        })?;
        let lock = Lockfile::load(&root).map_err(|e| vec![e])?.unwrap_or_default();
        let mut walk = Walk {
            lock: lock.packages.iter().filter(|_| !options.update).map(|p| (p.source.clone(), p.commit.clone())).collect(),
            recorded: BTreeSet::new(), deps: root.join("target").join("deps"), graph: CrateGraph::new(), packages: Vec::new(), ids: HashMap::new(), stack: Vec::new(), resolved: HashMap::new() };
        if !Resolver::crate_root(&root).is_file() {
            let message = format!("the package `{}` has no crate root `{}`", manifest.package.name, Resolver::crate_root(&root).display());
            return Err(vec![Resolver::fault(&root, Span::new(0, 0), "E0175", message, "the crate root is `src/lib.msl`", None)]);
        }
        walk.enter(&root, Identity::Path(root.clone()), manifest, None)?;
        Resolver::write_lockfile(&root, &Lockfile { packages: walk.recorded.into_iter().collect() })?;
        Ok(PackageGraph { graph: walk.graph, packages: walk.packages })
    }

    /// A package error in the manifest of the package at `dir`.
    fn fault(dir: &Path, span: Span, code: &'static str, message: String, label: &str, help: Option<String>) -> PackageError {
        let file = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let mut diagnostic = Diagnostic::new(code, message, span, label);
        diagnostic.help = help;
        PackageError { file, text, diagnostic }
    }

    /// Write `Piperine.lock` beside the root manifest when its text changes; none is made for a
    /// graph without a git package.
    fn write_lockfile(root: &Path, lockfile: &Lockfile) -> Result<(), Errors> {
        let file = root.join(LOCKFILE);
        let existing = std::fs::read_to_string(&file).ok();
        if lockfile.packages.is_empty() && existing.is_none() {
            return Ok(());
        }
        let text = lockfile.render();
        if existing.as_deref() != Some(text.as_str()) {
            std::fs::write(&file, &text).map_err(|e| {
                let message = format!("cannot write the lockfile `{}`: {e}", file.display());
                vec![Lockfile::error(&file, String::new(), Span::new(0, 0), message, "unwritable")]
            })?;
        }
        Ok(())
    }

    /// A rejected graph call, reported where it was made.
    fn graph_fault(dir: &Path, span: Span, error: &GraphError) -> PackageError {
        match error {
            GraphError::Cycle { names } => {
                let chain = names.join(" -> ");
                Resolver::fault(dir, span, "E0174", format!("the dependencies form a cycle: {chain}"), "closes the cycle", None)
            }
            other => Resolver::fault(dir, span, "E0171", format!("the crate graph refused the dependency: {other:?}"), "refused", None),
        }
    }

    /// A URL as a directory name: every character outside `[A-Za-z0-9._-]` becomes `_`.
    fn sanitize(url: &str) -> String {
        url.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect()
    }
}
