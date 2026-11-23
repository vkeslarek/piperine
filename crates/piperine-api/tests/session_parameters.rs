//! Scope: the session parameters of a run (analog-operators AO-07 and AO-08): a design that reads
//! `simparam(name, default)` gets the six options and the run's `simparam` entries as `GLOBAL
//! NAMED` pins; a read with no default of a name the run lacks is `E0456` naming it, before
//! anything binds. Each fixture is MSL written by the test and run on the Rust backend; expected
//! values come from the option's own definition.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Keeps nothing: these tests read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// A design whose output `o` is held at `simparam(<read>)` volts.
struct Reading;

impl Reading {
    fn settings() -> Settings {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
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
fn simparam_without_default_and_without_entry_is_e0456() {
    // AO-07 AC 3: `simparam("vth0")` with no default, no run entry named `vth0` and no option of that
    // name fails at the start of the analysis with `E0456` naming `vth0`, before anything binds.
    let failure = Reading::open("simparam(\"vth0\", None)", Overrides::default()).err().expect("the read has no value");
    match failure {
        Failure::Host { code, message } => {
            assert_eq!(code, "E0456");
            assert!(message.contains("vth0") && message.contains("Top"), "{message}");
        }
        other => panic!("{other}"),
    }
    // the same name given by the run, or an option of the session, opens
    let given = Overrides { simparam: vec![("vth0".into(), 0.7)], ..Overrides::default() };
    assert!(Reading::open("simparam(\"vth0\", None)", given).is_ok());
    assert!(Reading::open("simparam(\"reltol\", None)", Overrides::default()).is_ok());
    // a read with a default needs no entry
    assert!(Reading::open("simparam(\"vth0\", Some(0.5))", Overrides::default()).is_ok());
}

#[test]
fn the_session_hands_its_options_and_entries_to_the_circuit() {
    // AO-07 AC 1 and 2, AO-08 AC 3 (D13): the options by their key names, the run's entries by theirs,
    // and a default only where the run gives none.
    assert_eq!(Reading::output("simparam(\"vth0\", Some(0.5))", Overrides { simparam: vec![("vth0".into(), 0.7)], ..Overrides::default() }), 0.7);
    assert_eq!(Reading::output("simparam(\"vth0\", Some(0.5))", Overrides::default()), 0.5);
    assert_eq!(Reading::output("simparam(\"reltol\", None)", Overrides { reltol: Some(1e-4), ..Overrides::default() }), 1e-4);
    assert_eq!(Reading::output("simparam(\"temp\", None)", Overrides { temp: Some(75.0), ..Overrides::default() }), 75.0);
    assert_eq!(Reading::output("simparam(\"temp\", None)", Overrides::default()), 27.0);
    // `tnom` is no session value (user decision 2026-11-23): a read of it with no default is `E0456`
    assert!(Reading::open("simparam(\"tnom\", None)", Overrides::default()).is_err());
}
