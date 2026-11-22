//! Scope: the one run of chapter 15 of the Book, "The command line", that simulates `examples/buck`
//! (feature `integrated-tests`, IT-11): the 2 us start of the chapter's bench, which compiles the whole
//! converter. The target runs on demand (`test = false`, MD-77; `examples/buck/README.md`, "On demand");
//! the chapter's text equality stays in `book_cli`. Each test opens with the inventory ids it holds,
//! from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod model;
    pub mod printed;
    pub mod records;
    pub mod refusal;
}

use piperine::{Cell, Overrides};
use support::model::Model;

#[test]
fn the_buck_bench_starts_with_the_supervisor_off() {
    // cli#44 (run): the buck's `benches/startup.msl`, which the package of examples/buck runs as `startup`:
    // the first rows of its table (a short run of the same top) have the supervisor off (mode `Off`) and the
    // reference at 0, as the chapter says of the listing
    let buck = Model::example("buck");
    let ran = buck.of_run("startup").run(Overrides { stop: Some(2e-6), ..Overrides::default() }).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.rows.len(), 5, "0 to 2 us on the 0.5 us grid of the run");
    for row in 0..5 {
        assert_eq!(ran.cell(row, "Startup.mode"), &Cell::Text("Off".into()), "row {row}: the supervisor is off");
        assert_eq!(ran.cell(row, "Startup.buck.ctl.reference"), &Cell::Int(0), "row {row}: the reference is 0");
    }
}
