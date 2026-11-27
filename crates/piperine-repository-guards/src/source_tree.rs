//! The repository's Rust sources, read once, parsed once and marked product or test code.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::module_declaration::ModuleDeclaration;

/// One `.rs` file: its place in the repository, its syntax and whether it is test code.
pub struct SourceFile {
    path: PathBuf,
    crate_name: String,
    is_test: bool,
    line_count: usize,
    syntax: Result<syn::File, String>,
}

impl SourceFile {
    /// The path relative to the repository root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The crate directory the file sits in (`crates/<name>/...`), or `piperine` for the root package.
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// Whether the file is test code: under a `tests/` directory, or reached by a `#[cfg(test)]`
    /// module declaration or by a declaration written in test code.
    pub fn is_test(&self) -> bool {
        self.is_test
    }

    /// How many lines the file has.
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// The parsed syntax, or the parse error as text.
    pub fn syntax(&self) -> Result<&syn::File, &str> {
        self.syntax.as_ref().map_err(String::as_str)
    }
}

/// Every Rust source of the workspace.
pub struct SourceTree {
    files: Vec<SourceFile>,
}

impl SourceTree {
    /// The sources of the repository this crate sits in: the root package's `src/`, `tests/` and
    /// `build.rs`, and everything under `crates/`, skipping `target` and hidden directories.
    pub fn of_workspace() -> SourceTree {
        let root = Self::workspace_root();
        let mut texts = Vec::new();
        for entry in ["src", "tests", "build.rs", "crates"] {
            Self::collect(&root, &root.join(entry), &mut texts);
        }
        Self::from_texts(texts)
    }

    /// A tree over in-memory sources, each a path relative to the repository root and its text.
    pub fn of_sources(sources: &[(&str, &str)]) -> SourceTree {
        Self::from_texts(sources.iter().map(|(path, text)| (PathBuf::from(path), text.to_string())).collect())
    }

    /// The repository root: two directories above this crate's manifest.
    pub fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
    }

    /// Every file, in path order.
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    fn collect(root: &Path, path: &Path, texts: &mut Vec<(PathBuf, String)>) {
        if path.is_dir() {
            let Ok(entries) = std::fs::read_dir(path) else { return };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                Self::collect(root, &entry.path(), texts);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let (Ok(text), Ok(relative)) = (std::fs::read_to_string(path), path.strip_prefix(root)) {
                texts.push((relative.to_path_buf(), text));
            }
        }
    }

    fn from_texts(texts: Vec<(PathBuf, String)>) -> SourceTree {
        let sorted: BTreeMap<PathBuf, String> = texts.into_iter().collect();
        let mut files: Vec<SourceFile> = sorted
            .into_iter()
            .map(|(path, text)| SourceFile {
                crate_name: Self::crate_name_of(&path),
                is_test: path.components().any(|c| c.as_os_str() == "tests"),
                line_count: text.lines().count(),
                syntax: syn::parse_file(&text).map_err(|error| error.to_string()),
                path,
            })
            .collect();
        Self::mark_reached_test_files(&mut files);
        SourceTree { files }
    }

    fn crate_name_of(path: &Path) -> String {
        let mut components = path.components();
        match (components.next(), components.next()) {
            (Some(first), Some(second)) if first.as_os_str() == "crates" => second.as_os_str().to_string_lossy().to_string(),
            _ => "piperine".to_string(),
        }
    }

    /// Follows every module declaration to the file it reaches; a file reached by a `#[cfg(test)]`
    /// declaration, or by a declaration written in test code, is test code too.
    fn mark_reached_test_files(files: &mut [SourceFile]) {
        let index: BTreeMap<PathBuf, usize> = files.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
        let edges: Vec<Vec<(usize, bool)>> = files
            .iter()
            .map(|file| {
                let Ok(syntax) = file.syntax() else { return Vec::new() };
                ModuleDeclaration::of_file(syntax)
                    .iter()
                    .filter_map(|declaration| {
                        let reached = declaration.candidates(&file.path).into_iter().find_map(|candidate| index.get(&candidate).copied())?;
                        Some((reached, declaration.cfg_test))
                    })
                    .collect()
            })
            .collect();
        let mut changed = true;
        while changed {
            changed = false;
            for (from, reached) in edges.iter().enumerate() {
                for &(to, cfg_test) in reached {
                    if (cfg_test || files[from].is_test) && !files[to].is_test {
                        files[to].is_test = true;
                        changed = true;
                    }
                }
            }
        }
    }
}

impl SourceTree {
    /// The file at `root` and every file its module declarations reach, transitively.
    pub fn reached_from(&self, root: &Path) -> Vec<&SourceFile> {
        let index: BTreeMap<&Path, &SourceFile> = self.files.iter().map(|file| (file.path(), file)).collect();
        let mut reached: Vec<&SourceFile> = Vec::new();
        let mut pending: Vec<PathBuf> = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let Some(&file) = index.get(path.as_path()) else { continue };
            if reached.iter().any(|seen| std::ptr::eq(*seen, file)) {
                continue;
            }
            reached.push(file);
            let Ok(syntax) = file.syntax() else { continue };
            for declaration in ModuleDeclaration::of_file(syntax) {
                if let Some(next) = declaration.candidates(file.path()).into_iter().find(|candidate| index.contains_key(candidate.as_path())) {
                    pending.push(next);
                }
            }
        }
        reached
    }
}
