//! `Model`: a model of the Book written into a directory of its own and run through `piperine::`
//! (`Project`, `Session`), the way the host library runs it. A model is the Book's text, never a
//! copy a test wrote by hand: blocks come by id through `Inventory::checked_block`, so a block
//! that moved or changed fails by name instead of running another text.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use piperine::{Cell, Failure, Metadata, Overrides, Project, Record, Session, Table};
use piperine_transpile::Settings;

use super::book::Book;
use super::inventory::Inventory;
use super::records::KeptRecords;
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::copied_tree::CopiedTree;
use piperine_parity::table_columns::TableColumns;

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
    /// The run of the package that `run` runs; `None` for the only run, or for a lone file.
    named: Option<String>,
}

/// What a run gave: the table of its analysis, the records its streams emitted and what the design
/// declares (params, top ports, streams).
pub struct Ran {
    pub table: Table,
    pub records: Vec<Record>,
    pub metadata: Metadata,
}

impl Model {
    /// A model made of the blocks `ids` (joined in order, each followed by a newline) written as the
    /// lone file `file_name`.
    pub fn lone(book: &Book, inventory: &Inventory, file_name: &str, ids: &[&str]) -> Model {
        let text: String = ids.iter().map(|id| format!("{}\n", inventory.checked_block(book, id).text())).collect();
        let directory = Self::directory();
        Self::write(directory.path(), file_name, &text);
        let opening = Opening::File(directory.path().join(file_name));
        Model { directory, opening, named: None }
    }

    /// A lone file made of `before`, the block `id` and `after`: a block that is a piece of a file (a
    /// statement, a few items) set inside the text a test writes around it, so the piece itself runs.
    pub fn wrapped(book: &Book, inventory: &Inventory, file_name: &str, before: &str, id: &str, after: &str) -> Model {
        let text = format!("{before}{}\n{after}", inventory.checked_block(book, id).text());
        Model::written(file_name, &text)
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
                Model { directory, opening, named: None }
            }
            Some((package, inside)) => {
                let root = directory.path().join(package);
                CopiedTree::copy(&Self::repository().join("examples").join(package), &root);
                Self::write(&root, inside, &text);
                let opening = Opening::File(root.join(inside));
                Model { directory, opening, named: None }
            }
        }
    }

    /// A lone file `file_name` holding exactly `text`: the smallest model a test writes to make one
    /// mistake the Book names.
    pub fn written(file_name: &str, text: &str) -> Model {
        let directory = Self::directory();
        Self::write(directory.path(), file_name, text);
        let opening = Opening::File(directory.path().join(file_name));
        Model { directory, opening, named: None }
    }

    /// A package whose `Piperine.toml` is `manifest`, opened as a package; the rest of its files are
    /// added with [`Model::with_source`].
    pub fn package(manifest: &str) -> Model {
        let directory = Self::directory();
        let root = directory.path().join("package");
        Self::write(&root, "Piperine.toml", manifest);
        Model { directory, opening: Opening::Package(root), named: None }
    }

    /// A copy of the package `examples/<name>`, opened as a package.
    pub fn example(name: &str) -> Model {
        let directory = Self::directory();
        let root = directory.path().join(name);
        CopiedTree::copy(&Self::repository().join("examples").join(name), &root);
        Model { directory, opening: Opening::Package(root), named: None }
    }

    /// The text of the file `file` of the model, as the run reads it (a diagnostic's span is a byte
    /// range of it). Panics naming the file when it cannot be read.
    pub fn text_of(&self, file: &str) -> String {
        let path = self.base().join(file);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// The path a run opens: the lone file, or the package's root. A test that needs a session of its own
    /// (a build cache it can read back) opens this path itself.
    pub fn opened_path(&self) -> &Path {
        match &self.opening {
            Opening::File(path) | Opening::Package(path) => path,
        }
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

    /// The model whose `run` runs the run `name` of its package, as `piperine run <name>` does.
    pub fn of_run(mut self, name: &str) -> Model {
        self.named = Some(name.to_string());
        self
    }

    /// The model run as `overrides` ask, on the default backend, in a build directory shared by the
    /// target: the table and the records, or the failure.
    pub fn run(&self, overrides: Overrides) -> Result<Ran, Failure> {
        let project = match &self.opening {
            Opening::File(path) => Project::file(path)?,
            Opening::Package(root) => Project::at(root)?,
        };
        let spec = project.request(self.named.as_deref(), &overrides)?;
        let mut session = Session::open(&project, &spec, &Self::settings())?;
        let metadata = session.metadata();
        let kept = Rc::new(RefCell::new(KeptRecords::default()));
        let table = session.run(kept.clone())?;
        let records = kept.borrow().records().to_vec();
        Ok(Ran { table, records, metadata })
    }

    /// The settings of every run of this target: the compiled backend builds into the run's one
    /// compile cache, as the host library's own tests do.
    fn settings() -> Settings {
        CompileCache::shared().settings()
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
}

impl Ran {
    /// The cell of `row` under the column named `column`. Panics naming the column when the table
    /// has none.
    pub fn cell(&self, row: usize, column: &str) -> &Cell {
        TableColumns::cell(&self.table, row, column)
    }

    /// The real number of `row` under `column`. Panics when the cell is not a real number.
    pub fn real(&self, row: usize, column: &str) -> f64 {
        match self.cell(row, column) {
            Cell::Real(value) => *value,
            other => panic!("row {row}, column `{column}`: a real number, found {other:?}"),
        }
    }

    /// The value of the signal `name` of an operating point (a `signal | value` table). Panics naming
    /// the signal when the table has no such row.
    pub fn signal(&self, name: &str) -> f64 {
        let row = self.table.rows.iter().position(|row| row[0] == Cell::Text(name.to_string())).unwrap_or_else(|| panic!("no signal `{name}` in {:?}", self.table.rows));
        self.real(row, "value")
    }

    /// The records of the stream `stream`, in order.
    pub fn stream(&self, stream: &str) -> Vec<&Record> {
        self.records.iter().filter(|record| record.stream == stream).collect()
    }
}
