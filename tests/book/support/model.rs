//! `Model`: a model of the Book written into a directory of its own and run through `piperine::`
//! (`Project`, `Session`), the way the host library runs it. A model is the Book's text, never a
//! copy a test wrote by hand: blocks come by id through `Inventory::checked_block`, so a block
//! that moved or changed fails by name instead of running another text.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

use piperine::{Cell, Failure, Overrides, Project, Record, Session, Table};
use piperine_transpile::Settings;

use super::book::Book;
use super::inventory::Inventory;
use super::records::KeptRecords;

/// What opens a model: a lone file, or the package in the model's directory.
enum Opening {
    /// A lone `.msl` file, by path.
    File(PathBuf),
    /// The package whose root is the path.
    Package(PathBuf),
}

/// A model of the Book in a directory of its own, removed when the model is dropped.
pub struct Model {
    directory: tempfile::TempDir,
    opening: Opening,
}

/// What a run gave: the table of its analysis and the records its streams emitted.
pub struct Ran {
    pub table: Table,
    pub records: Vec<Record>,
}

impl Model {
    /// A model made of the blocks `ids` (joined in order, each followed by a newline) written as the
    /// lone file `file_name`.
    pub fn lone(book: &Book, inventory: &Inventory, file_name: &str, ids: &[&str]) -> Model {
        let text: String = ids.iter().map(|id| format!("{}\n", inventory.checked_block(book, id).text())).collect();
        let directory = Self::directory();
        Self::write(directory.path(), file_name, &text);
        let opening = Opening::File(directory.path().join(file_name));
        Model { directory, opening }
    }

    /// The file the Book assembles at the path `path` as it stands at the block `upto`, opened as a
    /// lone file as `piperine run <file>` opens it: a path with no directory stands alone;
    /// `rc/benches/delayed.msl` is that file added to a copy of `examples/rc`.
    pub fn marked(book: &Book, inventory: &Inventory, path: &str, upto: &str) -> Model {
        let block = inventory.checked_block(book, upto);
        let text = book.file(path, block.id());
        let directory = Self::directory();
        match path.split_once('/') {
            None => {
                Self::write(directory.path(), path, &text);
                let opening = Opening::File(directory.path().join(path));
                Model { directory, opening }
            }
            Some((package, inside)) => {
                let root = directory.path().join(package);
                Self::copy_tree(&Self::repository().join("examples").join(package), &root);
                Self::write(&root, inside, &text);
                let opening = Opening::File(root.join(inside));
                Model { directory, opening }
            }
        }
    }

    /// A copy of the package `examples/<name>`, opened as a package.
    pub fn example(name: &str) -> Model {
        let directory = Self::directory();
        let root = directory.path().join(name);
        Self::copy_tree(&Self::repository().join("examples").join(name), &root);
        Model { directory, opening: Opening::Package(root) }
    }

    /// The text of the file `file` of the model, as the run reads it (a diagnostic's span is a byte
    /// range of it). Panics naming the file when it cannot be read.
    pub fn text_of(&self, file: &str) -> String {
        let path = self.base().join(file);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The directory the model's files are relative to: a lone file's directory, a package's root.
    fn base(&self) -> PathBuf {
        match &self.opening {
            Opening::File(path) => path.parent().unwrap_or(self.directory.path()).to_path_buf(),
            Opening::Package(root) => root.clone(),
        }
    }

    /// The model with `text` added to the file `file` of its directory (the file is created when it
    /// does not exist): the top a test writes around a fragment, named in the test. A package's
    /// files are under the package's root, a lone file's directory is the model's own.
    pub fn with_source(self, file: &str, text: &str) -> Model {
        let base = self.base();
        let target = base.join(file);
        let existing = std::fs::read_to_string(&target).unwrap_or_default();
        Self::write(&base, file, &format!("{existing}{text}"));
        self
    }

    /// The model run as `overrides` ask, on the default backend, in a build directory shared by the
    /// target: the table and the records, or the failure.
    pub fn run(&self, overrides: Overrides) -> Result<Ran, Failure> {
        self.run_of(None, overrides)
    }

    /// The run `name` of the model's package, with `overrides` over its keys, as `piperine run <name>`.
    pub fn run_named(&self, name: &str, overrides: Overrides) -> Result<Ran, Failure> {
        self.run_of(Some(name), overrides)
    }

    /// The run `name` (the only run when `None`) of the model, with `overrides`.
    fn run_of(&self, name: Option<&str>, overrides: Overrides) -> Result<Ran, Failure> {
        let project = match &self.opening {
            Opening::File(path) => Project::file(path)?,
            Opening::Package(root) => Project::at(root)?,
        };
        let spec = project.request(name, &overrides)?;
        let mut session = Session::open(&project, &spec, &Self::settings())?;
        let kept = Rc::new(RefCell::new(KeptRecords::default()));
        let table = session.run(kept.clone())?;
        let records = kept.borrow().records().to_vec();
        Ok(Ran { table, records })
    }

    /// The settings of every run of this target: the compiled backend builds into one cache directory
    /// shared by the target, as the host library's own tests do.
    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("a build cache directory"));
        Settings { rustc: None, cache: cache.path().to_path_buf() }
    }

    /// A new directory for a model.
    fn directory() -> tempfile::TempDir {
        tempfile::Builder::new().prefix("piperine-book-").tempdir().expect("a model directory")
    }

    /// The repository's root.
    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    /// Writes `text` to `relative` under `base`, creating the directories it needs.
    fn write(base: &Path, relative: &str, text: &str) {
        let path = base.join(relative);
        std::fs::create_dir_all(path.parent().expect("a file has a directory")).expect("a directory for the model's file");
        std::fs::write(&path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }

    /// Copies the tree `from` to `to`.
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a directory for the copy");
        for entry in std::fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display())) {
            let entry = entry.expect("a directory entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("a file type").is_dir() {
                Self::copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap_or_else(|error| panic!("{}: {error}", target.display()));
            }
        }
    }
}

impl Ran {
    /// The cell of `row` under the column named `column`. Panics naming the column when the table
    /// has none.
    pub fn cell(&self, row: usize, column: &str) -> &Cell {
        let at = self.table.columns.iter().position(|name| name == column).unwrap_or_else(|| panic!("no column `{column}` in {:?}", self.table.columns));
        &self.table.rows[row][at]
    }

    /// The real number of `row` under `column`. Panics when the cell is not a real number.
    pub fn real(&self, row: usize, column: &str) -> f64 {
        match self.cell(row, column) {
            Cell::Real(value) => *value,
            other => panic!("row {row}, column `{column}`: a real number, found {other:?}"),
        }
    }

    /// The records of the stream `stream`, in order.
    pub fn stream(&self, stream: &str) -> Vec<&Record> {
        self.records.iter().filter(|record| record.stream == stream).collect()
    }
}
