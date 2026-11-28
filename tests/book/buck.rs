//! Scope: the one run of chapter 15 of the Book, "The command line", that simulates `examples/buck`
//! (feature `integrated-tests`, IT-11): the 2 us start of the chapter's bench, which compiles the whole
//! converter. The target runs on demand (`test = false`, MD-77; `examples/buck/README.md`, "On demand");
//! the chapter's text equality stays in `book_cli`. Each test opens with the inventory ids it holds,
//! from `tests/book/inventory.tsv`.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use piperine::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::copied_tree::CopiedTree;
use piperine_parity::table_columns::TableColumns;

/// A sink that drops every record: the buck's start is read from the table.
struct DroppedRecords;

impl RecordSink for DroppedRecords {
    /// Ignores `record`.
    fn record(&mut self, _record: &Record) {}
}

/// The buck example of the repository, copied to a directory of its own and run as the package it is.
struct BuckStart {
    /// The session of the bench, opened and not yet run.
    session: Session,
    /// The directory the example was copied to; it lives as long as the session.
    _directory: tempfile::TempDir,
}

impl BuckStart {
    /// The session of `examples/buck`'s bench `name` as `overrides` ask, on the default backend, not yet run.
    fn open(name: &str, overrides: Overrides) -> BuckStart {
        let directory = tempfile::Builder::new().prefix("piperine-book-").tempdir().expect("a model directory");
        let root = directory.path().join("buck");
        CopiedTree::copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/buck"), &root);
        let project = Project::at(&root).unwrap_or_else(|failure| panic!("{failure}"));
        let spec = project.request(Some(name), &overrides).unwrap_or_else(|failure| panic!("{failure}"));
        let session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|failure| panic!("{failure}"));
        BuckStart { session, _directory: directory }
    }
}

#[test]
fn the_buck_bench_starts_with_the_supervisor_off() {
    // cli#44 (run): the buck's `benches/startup.msl`, which the package of examples/buck runs as `startup`:
    // the first rows of its table (a short run of the same top) have the supervisor off (mode `Off`) and the
    // reference at 0, as the chapter says of the listing
    let mut buck = BuckStart::open("startup", Overrides { stop: Some(2e-6), ..Overrides::default() });
    let table = buck.session.run(Rc::new(RefCell::new(DroppedRecords))).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(table.rows.len(), 5, "0 to 2 us on the 0.5 us grid of the run");
    for row in 0..5 {
        assert_eq!(TableColumns::cell(&table, row, "Startup.mode"), &Cell::Text("Off".into()), "row {row}: the supervisor is off");
        assert_eq!(TableColumns::cell(&table, row, "Startup.buck.ctl.reference"), &Cell::Int(0), "row {row}: the reference is 0");
    }
}
