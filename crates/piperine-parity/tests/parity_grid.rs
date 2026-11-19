//! Scope: the print grid against ngspice's own (feature `long-run`, LR-15): `tran_rc` run through
//! `Session` with `print_step` equal to ngspice's TSTEP (the difference of the first two times of
//! its export), so that every row ngspice exports has a table row at its time and `v(out)` of the
//! pair agrees under `TRAN_VOLTAGE`. The test interpolates nothing: the grid is the host's, the
//! recorder's. With the comparison disabled the twin still runs and the harness prints its skip
//! line.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_parity::backend::Backend;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_transpile::Settings;

const STOP: f64 = 10e-6;
/// How close to ngspice's time a grid row must lie, as a fraction of the print step (LR-15 AC 1).
const TIME_MATCH: f64 = 1e-9;

struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// The rows of `tran_rc` to 10 us on a `print_step` grid, a step cap of a tenth of it, on `backend`.
fn grid(backend: Backend, print_step: f64) -> Table {
    let cache = tempfile::tempdir().expect("tmp");
    let twin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/tran_rc.msl");
    let project = Project::file(&twin).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        analysis: Some("transient".into()),
        stop: Some(STOP),
        print_step: Some(print_step),
        max_step: Some(print_step / 10.0),
        print: vec!["out".into()],
        backend: Some(backend.name().into()),
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
    session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("tran_rc on {}: {f}", backend.name()))
}

fn real(cell: &Cell) -> f64 {
    match cell {
        Cell::Real(x) => *x,
        other => panic!("a real, found {other:?}"),
    }
}

/// ngspice's export of `tran_rc`: the rows `(time, v(out))` and the version line.
fn export(reference: &Reference) -> Option<(Vec<(f64, f64)>, String)> {
    let Reference::Golden(golden) = reference else { return None };
    let rows = golden.real("tran_rc_tran");
    golden.expect_rows("tran_rc_tran", &rows, 100);
    Some((rows, golden.version().to_string()))
}

/// The table row whose time is within `1e-9 * print_step` of `time`, if any.
fn row_at(table: &Table, time: f64, print_step: f64) -> Option<&Vec<Cell>> {
    table.rows.iter().find(|row| (real(&row[0]) - time).abs() <= TIME_MATCH * print_step)
}

#[test]
fn the_grid_of_tran_rc_has_a_row_at_every_time_ngspice_exports() {
    // LR-15 AC 1
    let reference = Ngspice::from_environment().run("tran_rc");
    let Some((rows, _)) = export(&reference) else { return };
    let print_step = rows[1].0 - rows[0].0;
    for backend in Backend::selected() {
        let table = grid(backend, print_step);
        for (time, _) in &rows {
            assert!(row_at(&table, *time, print_step).is_some(), "{}: no grid row within 1e-9 * {print_step:e} s of ngspice's time {time:e} s", backend.name());
        }
        // the proof line of C45: only a run that compared prints it, a skipped reference returns
        // above before any
        println!("tran_rc grid on {}: {} rows matched", backend.name(), rows.len());
    }
}

#[test]
fn tran_rc_on_the_grid_agrees_with_ngspice() {
    // LR-15 AC 2: the pairs are read as they are, with no interpolation
    let reference = Ngspice::from_environment().run("tran_rc");
    let Some((rows, version)) = export(&reference) else { return };
    let print_step = rows[1].0 - rows[0].0;
    for backend in Backend::selected() {
        let table = grid(backend, print_step);
        let mut worst = 0.0_f64;
        for (time, wanted) in &rows {
            let row = row_at(&table, *time, print_step).unwrap_or_else(|| panic!("{}: no grid row at {time:e} s", backend.name()));
            let at = Point::new("tran_rc", "v(out)", Coordinate::Time(*time)).with_ngspice(&version).with_backend(backend.name());
            ToleranceContract::TRAN_VOLTAGE.assert(&at, *wanted, real(&row[1]));
            worst = worst.max((real(&row[1]) - wanted).abs());
        }
        // the proof line of C46: only a run that compared prints it
        println!("tran_rc on {}: {} pairs, largest |v(out)| deviation from ngspice {worst:e} V", backend.name(), rows.len());
    }
}

#[test]
fn a_skipped_reference_still_runs_the_twin() {
    // LR-15 AC 3: with the comparison disabled the harness prints its skip line (on standard
    // output, by `Ngspice::run`) and the twin still runs on its grid, 0 to 10 us at 0.1 us
    let reference = Ngspice::new(Lookup::Skip).run("tran_rc");
    assert!(matches!(reference, Reference::Skipped));
    for backend in Backend::selected() {
        let table = grid(backend, 0.1e-6);
        assert_eq!(table.rows.len(), 101, "{}: 0 to 10 us at 0.1 us", backend.name());
    }
}
