//! Scope: the host's sources on a top's node ports (PI-07; `docs/spec/part_viii_host.md` §23.1
//! rule 1, §8.1.14): `Session::source` sets the DC value at the operating point and the AC
//! stimulus, a run with a node port the host never drove is `E0810` naming it, and a port tied
//! to `gnd` inside the design is still refused undriven. The designs are MSL written by the
//! test; an expected value is a closed form read from the source.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Failure, NodeSource, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// A sink that keeps nothing: these runs read the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// A resistor and the top the tests drive: node port `a` across 1 kohm to `gnd`.
const DEVICES: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";

fn session(top: &str, print: &[&str]) -> Session {
    open(top, Overrides { analysis: Some("op".into()), print: print.iter().map(|s| s.to_string()).collect(), ..Overrides::default() })
}

fn transient(top: &str, stop: f64, max_step: f64, print_step: f64) -> Session {
    open(top, Overrides { analysis: Some("transient".into()), stop: Some(stop), max_step: Some(max_step), print_step: Some(print_step), print: vec!["a".into()], ..Overrides::default() })
}

fn open(top: &str, extra: Overrides) -> Session {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, format!("{DEVICES}\n{top}")).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), ..extra };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
}

fn run(session: &mut Session) -> Result<Table, Failure> {
    session.run(Rc::new(RefCell::new(Ignore)))
}

const LOADED: &str = "module Top(a: Electrical) {\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\n";

#[test]
fn a_dc_source_sets_the_port_at_the_operating_point() {
    // PI-07 AC 1, §23.1 rule 1: a source of 1.5 V on `a` before `op` makes v(a) 1.5 V
    let mut s = session(LOADED, &["a"]);
    s.source("a", NodeSource::dc(1.5)).expect("a source on `a`");
    let table = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    let Cell::Real(volts) = table.rows[0][1] else { panic!("a real, found {:?}", table.rows[0][1]) };
    assert!((volts - 1.5).abs() < 1e-9, "v(a) = {volts}");
    // written in full, and a later call replaces the earlier one
    let mut s = session(LOADED, &["a"]);
    s.source("Top.a", NodeSource::dc(1.5)).expect("written in full");
    s.source("a", NodeSource::dc(2.5)).expect("replaced");
    let table = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    let Cell::Real(volts) = table.rows[0][1] else { panic!("a real") };
    assert!((volts - 2.5).abs() < 1e-9, "v(a) = {volts}");
}

#[test]
fn an_undriven_node_port_is_e0810() {
    // PI-07 AC 5, §23.1 rule 1: the run is refused before any analysis, naming the port
    let mut s = session(LOADED, &["a"]);
    match run(&mut s) {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0810");
            assert!(message.contains("`a`"), "{message}");
        }
        other => panic!("E0810, found {other:?}"),
    }
    // the refusal consumed nothing: a source given now lets the same session run
    s.source("a", NodeSource::dc(1.0)).expect("a source");
    assert!(run(&mut s).is_ok());
    // a source on a path that is no node port is E0801
    let mut s = session(LOADED, &["a"]);
    assert!(matches!(s.source("nowhere", NodeSource::dc(1.0)), Err(Failure::Host { code: "E0801", .. })));
}

#[test]
fn a_node_port_tied_to_gnd_inside_is_still_refused_undriven() {
    // PI-07 edge case: a port the design ties to gnd itself is still the host's to drive
    let top = "module Top(a: Electrical, b: Electrical) {\n    let r1 = Resistor(b, gnd) { r: 1k };\n}\nanalog for Top { v(a, gnd) +> 0.0; }\n";
    let mut s = session(top, &["b"]);
    s.source("b", NodeSource::dc(1.0)).expect("b is driven");
    match run(&mut s) {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0810");
            assert!(message.contains("`a`"), "{message}");
        }
        other => panic!("E0810 naming `a`, found {other:?}"),
    }
}

/// The value of the waveform through `corners` at `time`: linear between corners, held before the
/// first and after the last (§23.1 rule 1).
fn piecewise(corners: &[(f64, f64)], time: f64) -> f64 {
    let (first, last) = (corners[0], corners[corners.len() - 1]);
    if time <= first.0 {
        return first.1;
    }
    match corners.windows(2).find(|pair| time <= pair[1].0) {
        Some(pair) => pair[0].1 + (pair[1].1 - pair[0].1) * (time - pair[0].0) / (pair[1].0 - pair[0].0),
        None => last.1,
    }
}

fn real(cell: &Cell) -> f64 {
    match cell {
        Cell::Real(value) => *value,
        other => panic!("a real, found {other:?}"),
    }
}

#[test]
fn a_pwl_source_lands_on_every_corner() {
    // PI-07 AC 2, §23.1 rule 1 and D9: a transient lands on each corner and v(a) equals the
    // waveform there and between (linear), whatever the step cap: the corners are not on the
    // step grid (3e-7), and the table's print grid (1e-6) holds each of them
    let corners = [(0.0, 0.0), (2e-6, 1.0), (5e-6, 1.0), (6e-6, 0.25)];
    let mut s = transient(LOADED, 8e-6, 3e-7, 1e-6);
    s.source("a", NodeSource::dc(0.0).with_corners(corners.to_vec())).expect("a piecewise-linear source");
    let table = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    assert!(table.rows.len() >= 9, "{} rows", table.rows.len());
    for row in &table.rows {
        let (time, volts) = (real(&row[0]), real(&row[1]));
        let wanted = piecewise(&corners, time);
        assert!((volts - wanted).abs() < 1e-6, "v(a) at {time:e} s is {volts}, the waveform says {wanted}");
    }
    for (time, wanted) in corners {
        let row = table.rows.iter().find(|row| (real(&row[0]) - time).abs() < 1e-12).unwrap_or_else(|| panic!("no row at the corner {time:e} s"));
        assert!((real(&row[1]) - wanted).abs() < 1e-6, "v(a) at the corner {time:e} s is {}, not {wanted}", real(&row[1]));
    }
}

#[test]
fn a_timed_source_change_is_a_jump() {
    // PI-07 AC 4, §15.2: the host changes the source at 2 us from 1 V to 3 V; before it the port
    // is at 1 V, after it at 3 V (the change is a time point and a jump)
    let mut s = transient(LOADED, 4e-6, 1e-7, 5e-7);
    s.source("a", NodeSource::dc(1.0)).expect("a source");
    s.source_at("a", 3.0, 2e-6).expect("a jump at 2 us");
    let table = run(&mut s).unwrap_or_else(|f| panic!("{f}"));
    for row in &table.rows {
        let (time, volts) = (real(&row[0]), real(&row[1]));
        if time < 1.9e-6 {
            assert!((volts - 1.0).abs() < 1e-6, "before the jump v(a) at {time:e} s is {volts}");
        } else if time > 2.1e-6 {
            assert!((volts - 3.0).abs() < 1e-6, "after the jump v(a) at {time:e} s is {volts}");
        }
    }
    // a time already past is E0806 and changes nothing
    let mut s = transient(LOADED, 4e-6, 1e-7, 5e-7);
    assert!(matches!(s.source_at("a", 3.0, -1.0), Err(Failure::Host { code: "E0806", .. })));
}

#[test]
fn corners_that_do_not_increase_are_e0802_and_need_a_transient() {
    // the corners are strictly increasing in time (design D9): E0802 and nothing is set; a run
    // with no time axis has nothing to land on (P0010)
    let mut s = transient(LOADED, 4e-6, 1e-7, 5e-7);
    let backwards = NodeSource::dc(0.0).with_corners(vec![(1e-6, 0.0), (1e-6, 1.0)]);
    assert!(matches!(s.source("a", backwards), Err(Failure::Host { code: "E0802", .. })));
    match run(&mut s) {
        Err(Failure::Host { code: "E0810", .. }) => {}
        other => panic!("the refused source left the port undriven: {other:?}"),
    }
    let mut s = session(LOADED, &["a"]);
    let corners = NodeSource::dc(0.0).with_corners(vec![(0.0, 0.0), (1e-6, 1.0)]);
    assert!(matches!(s.source("a", corners), Err(Failure::Host { code: "P0010", .. })));
}
