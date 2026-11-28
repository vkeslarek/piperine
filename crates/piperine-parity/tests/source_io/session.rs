//! `simparam` run from MSL source on the Rust backend through the host library (analog-operators
//! AO-07, Part IV §14.4): each bench holds its output `o` at one `simparam` read, an operating
//! point or a transient of the session reads it, and every expected value is the option's or the
//! entry's own value.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;
use piperine_parity::compile_cache::CompileCache;

/// Keeps nothing: these tests read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// A bench of the session: `Top` holds `o` with the body of its `analog` block `body`.
struct Bench;

impl Bench {
    /// The compile cache the runs share.
    fn settings() -> Settings {
        let dir = CompileCache::shared();
        Settings { rustc: None, cache: dir.path().to_path_buf() }
    }

    /// The value of `Top.o` at the end of the run of the bench whose `analog` block is `body`.
    fn output(body: &str, overrides: Overrides) -> f64 {
        let source = format!("module Resistor(p: Electrical, n: Electrical) {{ r: f64 where r > 0.0; }}\nanalog for Resistor {{ i(p, n) +> v(p, n) / r; }}\nmodule Top() {{ signal o: Electrical; let r = Resistor {{ r: 1k }}(o, gnd); }}\nanalog for Top {{ {body} }}\n");
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, source).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some("Top".into()), print: vec!["o".into()], ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Self::settings()).unwrap_or_else(|f| panic!("{f}"));
        let table: Table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let last = table.rows.iter().rev().find(|row| row.iter().any(|c| *c == Cell::Text("Top.o".into()))).or(table.rows.last()).unwrap_or_else(|| panic!("no row in {table:?}"));
        match last.iter().rev().find_map(|c| if let Cell::Real(v) = c { Some(*v) } else { None }) {
            Some(v) => v,
            None => panic!("no real in {last:?}"),
        }
    }

    /// An operating point.
    fn op() -> Overrides {
        Overrides { analysis: Some("op".into()), ..Overrides::default() }
    }

    /// A transient to 1 us in steps of at most 0.1 us.
    fn transient() -> Overrides {
        Overrides { analysis: Some("transient".into()), stop: Some(1e-6), max_step: Some(0.1e-6), ..Overrides::default() }
    }
}

#[test]
fn simparam_reads_the_run_entry_else_its_default() {
    // AO-07 AC 1: `simparam("vth0").unwrap_or(0.5)` is 0.7 when the run gives `simparam = { vth0 = 0.7 }`
    // and 0.5 when it gives none; with both, the run's value.
    let read = "v(o, gnd) +> simparam(\"vth0\").unwrap_or(0.5);";
    assert_eq!(Bench::output(read, Overrides { simparam: vec![("vth0".into(), 0.7)], ..Bench::op() }), 0.7);
    assert_eq!(Bench::output(read, Bench::op()), 0.5);
}

#[test]
fn simparam_reads_the_options_by_their_key_names() {
    // AO-07 AC 2: `simparam("reltol").unwrap_or(0.0)` is the run's `reltol`, or the solver's default 1e-3.
    let read = "v(o, gnd) +> simparam(\"reltol\").unwrap_or(0.0);";
    assert_eq!(Bench::output(read, Overrides { reltol: Some(1e-5), ..Bench::op() }), 1e-5);
    assert_eq!(Bench::output(read, Bench::op()), 1e-3);
}

#[test]
fn simparam_runs_in_a_definition_and_a_selector_with_one_value() {
    // AO-07 AC 4 (§14.4.1): a selector (`if` over a `simparam`) picks the arm the run's value says, for
    // the whole analysis: 2 V with `sel` absent, 1 V with `sel = 1`, at every step of a transient.
    let selector = "if simparam(\"sel\").unwrap_or(0.0) > 0.5 { v(o, gnd) +> 1.0; } else { v(o, gnd) +> 2.0; }";
    assert_eq!(Bench::output(selector, Bench::op()), 2.0);
    assert_eq!(Bench::output(selector, Overrides { simparam: vec![("sel".into(), 1.0)], ..Bench::op() }), 1.0);
    assert_eq!(Bench::output(selector, Overrides { simparam: vec![("sel".into(), 1.0)], ..Bench::transient() }), 1.0);
    // a definition: a body `let` bound to the read is the read at each use
    let definition = "let g = simparam(\"gain\").unwrap_or(2.0); v(o, gnd) +> g * 3.0;";
    assert_eq!(Bench::output(definition, Bench::op()), 6.0);
    assert_eq!(Bench::output(definition, Overrides { simparam: vec![("gain".into(), 4.0)], ..Bench::op() }), 12.0);
}
