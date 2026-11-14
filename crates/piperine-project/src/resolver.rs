//! The resolver: a depth-first walk from the root package that locates every dependency and
//! returns the crate graph. A package is identified by where it is, so a package reached twice
//! is one crate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use piperine_lang::msl::{Diagnostic, Span};
use piperine_lang::pom::{CrateGraph, CrateId, GraphError};

use crate::error::PackageError;
use crate::manifest::{Dependency, MANIFEST, Manifest, Source};

/// What a resolution may do beyond reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {}

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
}

type Errors = Vec<PackageError>;

/// The crate root of a package directory.
fn crate_root(dir: &Path) -> PathBuf {
    dir.join("src").join("lib.msl")
}

/// Resolve the package at `dir` and everything it depends on.
pub fn resolve(dir: &Path, _options: Options) -> Result<PackageGraph, Vec<PackageError>> {
    let manifest = Manifest::load(dir)?;
    let root = dir.canonicalize().map_err(|e| {
        let message = format!("cannot read the package directory `{}`: {e}", dir.display());
        vec![fault(dir, Span::new(0, 0), "E0170", message, "a package holds a `Piperine.toml`", None)]
    })?;
    let mut walk = Walk { graph: CrateGraph::new(), packages: Vec::new(), ids: HashMap::new(), stack: Vec::new() };
    if !crate_root(&root).is_file() {
        let message = format!("the package `{}` has no crate root `{}`", manifest.package.name, crate_root(&root).display());
        return Err(vec![fault(&root, Span::new(0, 0), "E0175", message, "the crate root is `src/lib.msl`", None)]);
    }
    walk.enter(&root, Identity::Path(root.clone()), manifest)?;
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

struct Walk {
    graph: CrateGraph,
    packages: Vec<ResolvedPackage>,
    ids: HashMap<Identity, CrateId>,
    /// The packages being walked, outermost first, with their names.
    stack: Vec<(Identity, String)>,
}

impl Walk {
    /// Make the crate of the package at `dir` and walk its dependencies.
    fn enter(&mut self, dir: &Path, identity: Identity, manifest: Manifest) -> Result<CrateId, Errors> {
        let name = manifest.package.name.clone();
        let id = self.graph.add_crate(&name, crate_root(dir)).map_err(|e| vec![graph_fault(dir, Span::new(0, 0), &e)])?;
        self.ids.insert(identity.clone(), id);
        self.packages.push(ResolvedPackage { krate: id, dir: dir.to_path_buf(), manifest: manifest.clone(), commit: None });
        self.stack.push((identity, name));
        for dependency in &manifest.dependencies {
            let child = self.dependency(dir, dependency)?;
            self.graph.add_dependency(id, &dependency.name, child).map_err(|e| vec![graph_fault(dir, dependency.name_span, &e)])?;
        }
        self.stack.pop();
        Ok(id)
    }

    /// Locate one dependency of the package at `dir`.
    fn dependency(&mut self, dir: &Path, dependency: &Dependency) -> Result<CrateId, Errors> {
        let (target, identity) = match &dependency.source {
            Source::Path(path) => {
                let target = dir.join(path);
                if !target.join(MANIFEST).is_file() {
                    let message = format!("dependency `{}` is not at `{}`: no `{MANIFEST}` there", dependency.name, target.display());
                    return Err(vec![fault(dir, dependency.name_span, "E0172", message, "no package at this path", None)]);
                }
                let target = target.canonicalize().map_err(|e| {
                    let message = format!("dependency `{}` is not at `{}`: {e}", dependency.name, target.display());
                    vec![fault(dir, dependency.name_span, "E0172", message, "no package at this path", None)]
                })?;
                let identity = Identity::Path(target.clone());
                (target, identity)
            }
            Source::Git { .. } => {
                let message = format!("dependency `{}` is a git dependency; git dependencies are not resolved yet", dependency.name);
                return Err(vec![fault(dir, dependency.name_span, "E0172", message, "not resolved yet", None)]);
            }
        };
        let manifest = Manifest::load(&target)?;
        if !crate_root(&target).is_file() {
            let message = format!("the package `{}` has no crate root `{}`", manifest.package.name, crate_root(&target).display());
            return Err(vec![fault(dir, dependency.name_span, "E0175", message, "this package has no `src/lib.msl`", None)]);
        }
        match self.ids.get(&identity) {
            Some(id) => Ok(*id),
            None => self.enter(&target, identity, manifest),
        }
    }
}

/// A rejected graph call, reported where it was made.
fn graph_fault(dir: &Path, span: Span, error: &GraphError) -> PackageError {
    match error {
        GraphError::Cycle { names } => {
            let chain = names.join(" -> ");
            fault(dir, span, "E0174", format!("the dependencies form a cycle: {chain}"), "closes the cycle", None)
        }
        other => fault(dir, span, "E0171", format!("the crate graph refused the dependency: {other:?}"), "refused", None),
    }
}
