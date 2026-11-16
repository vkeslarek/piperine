//! `Project`: the package a command works on, its runs, and the crate graph of one run (CLI-02,
//! CLI-03 AC 8 to 11). A run with a bench roots its graph at the bench file, a crate named
//! `bench` that depends on the package under the package's name and on each of the package's
//! direct dependencies under their names (Cargo's rule for an integration test); a run without
//! one roots at the package's crate. A lone file is a bench of the package found upward from it,
//! or a crate alone when there is none.

use std::path::{Path, PathBuf};

use piperine_lang::msl::{Diagnostic, Span};
use piperine_lang::pom::{CrateGraph, CrateId, Design};
use piperine_project::{Options, PackageError, PackageGraph, find_project_root, resolve};

use crate::failure::Failure;
use crate::request::{Overrides, RunSpec};

/// The name the root crate of a run with a bench takes.
const BENCH: &str = "bench";
const MANIFEST: &str = "Piperine.toml";

/// A package, or a lone file, ready to run.
#[derive(Debug)]
pub struct Project {
    name: String,
    /// The package directory; a lone file's own directory when it has no package.
    dir: PathBuf,
    /// The resolved packages; `None` for a lone file with no manifest upward.
    packages: Option<PackageGraph>,
    /// The manifest text of the package, read once.
    manifest: Option<String>,
    /// The runs of the package's own manifest (a dependency's are never read).
    runs: Vec<RunSpec>,
    /// The file `Project::file` was given: the bench of every request.
    lone: Option<PathBuf>,
}

impl Project {
    /// The package of the nearest `Piperine.toml` upward from `start` (`E0170` when none).
    pub fn discover(start: &Path) -> Result<Project, Failure> {
        match find_project_root(start) {
            Some(dir) => Project::at(&dir),
            None => {
                let message = format!("no `{MANIFEST}` found in `{}` or any directory above it", start.display());
                let diagnostic = Diagnostic::new("E0170", message, Span::new(0, 0), "a package holds a `Piperine.toml`");
                Err(Failure::Package(vec![PackageError { file: start.join(MANIFEST), text: String::new(), diagnostic }]))
            }
        }
    }

    /// The package at `dir`; no search upward.
    pub fn at(dir: &Path) -> Result<Project, Failure> {
        let packages = resolve(dir, Options::default()).map_err(Failure::Package)?;
        let root = &packages.packages[0];
        let file = root.dir.join(MANIFEST);
        let text = std::fs::read_to_string(&file).map_err(|error| Failure::Io { path: file.clone(), error })?;
        let runs = RunSpec::read(&text, &file).unwrap_or_default();
        Ok(Project {
            name: root.manifest.package.name.clone(),
            dir: root.dir.clone(),
            runs,
            manifest: Some(text),
            packages: Some(packages),
            lone: None,
        })
    }

    /// A lone file: a bench of the package found upward from its directory, a crate alone when
    /// there is none; a file under that package's `src/` is refused (`P0010`).
    pub fn file(path: &Path) -> Result<Project, Failure> {
        let file = path.canonicalize().map_err(|error| Failure::Io { path: path.to_path_buf(), error })?;
        let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let Some(root) = find_project_root(&dir) else {
            return Ok(Project { name: stem, dir, packages: None, manifest: None, runs: Vec::new(), lone: Some(file) });
        };
        let mut project = Project::at(&root)?;
        if file.starts_with(project.dir.join("src")) {
            let message = format!(
                "`{}` is part of the crate of package `{}`, not a bench: run a file outside `src/`, or a run of the package",
                path.display(),
                project.name
            );
            return Err(Failure::Host { code: "P0010", message });
        }
        project.name = stem;
        project.runs = Vec::new();
        project.lone = Some(file);
        Ok(project)
    }

    /// The package's name; a lone file's stem.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The package directory; a lone file's directory when it has no package.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The runs of the package's manifest, in source order; none for a lone file, and none when
    /// the run table is malformed (`request` reports why).
    pub fn runs(&self) -> &[RunSpec] {
        &self.runs
    }

    /// Every fault of the package's run table, each a `P0010` at its span.
    pub fn faults(&self) -> Vec<Failure> {
        match (&self.manifest, &self.lone) {
            (Some(text), None) => RunSpec::read(text, &self.dir.join(MANIFEST)).err().unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The run `name` (the only run when `name` is `None`) with `overrides` applied, validated;
    /// a lone file's run is made from `overrides` alone and named by the file's stem.
    pub fn request(&self, name: Option<&str>, overrides: &Overrides) -> Result<RunSpec, Failure> {
        if self.lone.is_some() {
            return RunSpec::from_flags(&self.name, overrides);
        }
        if let Some(first) = self.faults().into_iter().next() {
            return Err(first);
        }
        let names = || self.runs.iter().map(|r| r.name.clone()).collect::<Vec<_>>();
        let run = match name {
            Some(name) => self.runs.iter().find(|r| r.name == name),
            None if self.runs.len() == 1 => self.runs.first(),
            None => None,
        };
        let Some(run) = run else { return Err(Failure::NoRun { asked: name.map(str::to_string), runs: names() }) };
        let spec = run.merged(overrides)?;
        if let Some(bench) = &spec.bench {
            if !self.dir.join(bench).is_file() {
                let message = format!("run `{}`: the bench `{}` does not exist", spec.name, bench.display());
                return Err(spec.fault(spec.bench_at(), message, "no such file"));
            }
        }
        Ok(spec)
    }

    /// The crate graph of `spec`, read: rooted at its bench (or the lone file) when it has one,
    /// at the package's crate otherwise. A language error is a diagnostic of the design, not a
    /// failure here.
    pub fn design(&self, spec: &RunSpec) -> Result<Design, Failure> {
        let bench = spec.bench.as_ref().map(|b| self.dir.join(b)).or_else(|| self.lone.clone());
        let Some(packages) = &self.packages else {
            // a lone file with no package: the file and `std`
            return Ok(Design::load(bench.unwrap_or_default()));
        };
        let Some(bench) = bench else { return Ok(Design::load_crates(packages.graph.clone())) };
        let graph = Project::bench_graph(&bench, packages).map_err(|message| Failure::Host { code: "P0010", message })?;
        Ok(Design::load_crates(graph))
    }

    /// `bench` as the root crate over the package graph: the package's crates in their order
    /// and with their edges, then `bench` → the package and → each of its direct dependencies.
    fn bench_graph(bench: &Path, packages: &PackageGraph) -> Result<CrateGraph, String> {
        let refused = |e| format!("the crate graph of the bench `{}`: {e:?}", bench.display());
        let given = packages.graph.crates();
        let mut graph = CrateGraph::new();
        let root = graph.add_crate(BENCH, bench).map_err(refused)?;
        let mut ids = Vec::with_capacity(given.len());
        for krate in given {
            ids.push(graph.add_crate(&krate.name, krate.root.clone()).map_err(refused)?);
        }
        let id = |c: CrateId| ids[c.index()];
        for (k, krate) in given.iter().enumerate() {
            for d in &krate.dependencies {
                graph.add_dependency(ids[k], &d.name, id(d.krate)).map_err(refused)?;
            }
        }
        let package = &given[0];
        graph.add_dependency(root, &package.name, ids[0]).map_err(refused)?;
        for d in &package.dependencies {
            graph.add_dependency(root, &d.name, id(d.krate)).map_err(refused)?;
        }
        Ok(graph)
    }
}
