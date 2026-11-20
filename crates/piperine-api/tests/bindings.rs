//! Before-run bindings, through `Session` (feature `analog-structure`, AS-15 AC 1): a binding of a
//! param that fixes structure is an elaboration input, so the design lowers the variant of its value
//! and the value holds from the start; a binding of any other param is still the host's write.
//! Expected values are the circuit's closed forms: 1 V behind 1 kOhm into the device under test.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

struct Kept;

impl RecordSink for Kept {
    fn record(&mut self, _: &Record) {}
}

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    Settings { rustc: None, cache: CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path().to_path_buf() }
}

/// `Sw` is a potential source of 0.25 V while `short` selects it and an open branch otherwise (the
/// selector depends on the param only, so `short` is structural); `Load` reads the plain param
/// `scale` into a resistor's value. Behind 1 kOhm from 1 V.
const SOURCE: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }
analog for Resistor { i(p, n) +> v(p, n) / r; }
pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }
analog for Vsource { v(p, n) +> dc; }
pub module Sw(a: Electrical, b: Electrical) { short: f64 = 1.0; }
analog for Sw { if short > 0.5 { v(a, b) +> 0.25; } else { i(a, b) +> 0.0; } }
pub module Load(a: Electrical, b: Electrical) { scale: f64 = 1.0; }
analog for Load { i(a, b) +> v(a, b) * scale / 1k; }
module Top() {
    signal s: Electrical;
    signal a: Electrical;
    signal c: Electrical;
    let v1 = Vsource(s, gnd) { dc: 1.0 };
    let r1 = Resistor(s, a) { r: 1k };
    let m = Sw(a, gnd);
    let r2 = Resistor(s, c) { r: 1k };
    let l = Load(c, gnd);
}
";

fn run(params: Vec<(String, f64)>) -> Table {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, SOURCE).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), backend: Some("rust".into()), top: Some("Top".into()), print: vec!["a".into(), "c".into()], params, ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
    session.run(Rc::new(RefCell::new(Kept))).unwrap_or_else(|f| panic!("{f}"))
}

fn real(cell: &Cell) -> f64 {
    match cell {
        Cell::Real(v) => *v,
        other => panic!("a real, found {other:?}"),
    }
}

fn close(got: f64, wanted: f64, what: &str) {
    assert!((got - wanted).abs() <= 1e-6, "{what}: {got} against {wanted}");
}

#[test]
fn a_before_run_binding_of_a_structural_param_selects_its_variant() {
    // `short` selects the 0.25 V source by default: v(a) = 0.25. `--param Top.m.short=0` selects the
    // other variant at elaboration, the open branch, so v(a) is the source's 1 V (no current flows).
    let table = run(vec![]);
    close(real(&table.rows[0][1]), 0.25, "the default variant");
    let table = run(vec![("Top.m.short".into(), 0.0)]);
    close(real(&table.rows[0][1]), 1.0, "short = 0");
}

#[test]
fn a_before_run_binding_of_a_plain_param_is_still_the_hosts_write() {
    // `scale` is read by no selector: it is no key, the elaborator ignores the binding and the host
    // writes it before the run: the load is 1 kOhm / scale behind 1 kOhm, so `c` is 1/3 V with
    // `scale` = 2 (500 Ohm over 1.5 kOhm) and 0.5 V with 1.
    let table = run(vec![]);
    close(real(&table.rows[1][1]), 0.5, "scale = 1");
    let table = run(vec![("Top.l.scale".into(), 2.0)]);
    close(real(&table.rows[1][1]), 1.0 / 3.0, "scale = 2");
}
