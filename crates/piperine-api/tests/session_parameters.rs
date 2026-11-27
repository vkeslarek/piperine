//! Scope: the session parameters of a run (analog-operators AO-07 and AO-08): a design that reads
//! `simparam(name, default)` gets the six options and the run's `simparam` entries as `GLOBAL
//! NAMED` pins; a name the run lacks reads `None` (user decision 2026-11-23, no `E0456`). Each fixture is MSL written by the test and run on the Rust backend; expected
//! values come from the option's own definition.

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;
use piperine_parity::compile_cache::CompileCache;

/// Keeps nothing: these tests read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// A design whose output `o` is held at `simparam(<read>)` volts.
struct Reading;

impl Reading {
    fn settings() -> Settings {
        let dir = CompileCache::shared();
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The source of the bench `Top`: `v(o) +> <read>` with a 1 kohm load, so the point is solvable.
    fn source(read: &str) -> String {
        format!("module Resistor(p: Electrical, n: Electrical) {{ r: f64 where r > 0.0; }}\nanalog for Resistor {{ i(p, n) +> v(p, n) / r; }}\nmodule Top() {{ signal o: Electrical; let r = Resistor {{ r: 1k }}(o, gnd); }}\nanalog for Top {{ v(o, gnd) +> {read}; }}\n")
    }

    /// The session of the bench reading `read`, an operating point printing `o`, with `overrides`.
    fn open(read: &str, overrides: Overrides) -> Result<Session, Failure> {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, Self::source(read)).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some("Top".into()), analysis: Some("op".into()), print: vec!["o".into()], ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        Session::open(&project, &spec, &Self::settings())
    }

    /// `v(o)` of the operating point of the bench reading `read`.
    fn output(read: &str, overrides: Overrides) -> f64 {
        let mut session = Self::open(read, overrides).unwrap_or_else(|f| panic!("{f}"));
        let table: Table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let row = table.rows.iter().find(|r| r[0] == Cell::Text("Top.o".into())).unwrap_or_else(|| panic!("no row `Top.o` in {table:?}"));
        match &row[1] {
            Cell::Real(v) => *v,
            other => panic!("a real, found {other:?}"),
        }
    }
}

#[test]
fn a_name_the_run_lacks_reads_none() {
    // AO-07 (user decision 2026-11-23): `simparam("vth0")` is an `Option<f64>`: `None` when the run gives no
    // such name, so the session opens (there is no `E0456`) and the design takes its own default.
    assert_eq!(Reading::output("simparam(\"vth0\").unwrap_or(0.5)", Overrides::default()), 0.5);
    assert_eq!(Reading::output("simparam(\"vth0\").unwrap_or(0.5)", Overrides { simparam: vec![("vth0".into(), 0.7)], ..Overrides::default() }), 0.7);
}

#[test]
fn the_session_hands_its_options_and_entries_to_the_circuit() {
    // AO-07 AC 1 and 2 (D13): the options by their key names, the run's entries by theirs.
    assert_eq!(Reading::output("simparam(\"reltol\").unwrap_or(0.0)", Overrides { reltol: Some(1e-4), ..Overrides::default() }), 1e-4);
    assert_eq!(Reading::output("simparam(\"temp\").unwrap_or(0.0)", Overrides { temp: Some(75.0), ..Overrides::default() }), 75.0);
    assert_eq!(Reading::output("simparam(\"temp\").unwrap_or(0.0)", Overrides::default()), 27.0);
    // `tnom` is no session value (user decision 2026-11-23): it reads `None`
    assert_eq!(Reading::output("simparam(\"tnom\").unwrap_or(-1.0)", Overrides::default()), -1.0);
}
