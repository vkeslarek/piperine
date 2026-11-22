//! Scope: a host and the composite values of a design (composite-values CV-18, Q2 (c), D13): a
//! composite param is set by the path of one leaf, `Top.num[2]`, `Top.g.a`, `Top.t.1`, and its
//! `where` is checked on the whole value after the write (`E0317`). The design is MSL written by
//! the test, a reaction emits what it reads of each param, and an expected value is the value the
//! test wrote, cited at each assertion.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Failure, Field, Overrides, ParamValue, Project, Record, RecordSink, Session};
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

/// A struct, an array and a tuple param, each read by a reaction that emits what it holds.
const DESIGN: &str = "#[derive(Repr, PartialEq)]\nstruct Gains { a: u8, b: f64, armed: bool }\n\
module Top(in clk: Bit) {\n    g: Gains = Gains { a: 3, b: 0.5, armed: false } where g.a < 10;\n    num: [f64; 3] = [1.0, 2.0, 3.0];\n    t: (u8, bool) = (4, false);\n}\n\
digital for Top {\n    on clk.rise() { emit!(\"seen\", a = g.a, b = g.b, armed = g.armed, n = num[2], t0 = t.0, t1 = t.1); }\n}\n";

/// The session of `DESIGN` over a transient that sees one rising edge of `clk` at 100 ns.
struct Host {
    session: Session,
}

impl Host {
    fn open() -> Host {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, DESIGN).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { backend: Some("rust".into()), top: Some("Top".into()), analysis: Some("transient".into()), stop: Some(2e-7), max_step: Some(1e-8), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        session.drive("clk", ParamValue::Bool(false), 0.0).expect("drive clk low");
        session.drive("clk", ParamValue::Bool(true), 1e-7).expect("drive clk high");
        Host { session }
    }

    /// The failure `set` of `path` gives, as its code.
    fn refused(&mut self, path: &str, value: f64) -> (&'static str, String) {
        match self.session.set_at(path, value, 0.0) {
            Err(Failure::Host { code, message }) => (code, message),
            other => panic!("`{path}` = {value} is refused: {other:?}"),
        }
    }

    /// The `seen` record of the run.
    fn seen(&mut self) -> Vec<(String, Field)> {
        let kept = Rc::new(RefCell::new(Kept::default()));
        self.session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
        let records = kept.borrow().0.clone();
        let seen: Vec<&Record> = records.iter().filter(|r| r.stream == "seen").collect();
        assert_eq!(seen.len(), 1, "one rising edge: {records:?}");
        seen[0].fields.clone()
    }
}

/// The value of the field `name` of a record.
struct Seen;

impl Seen {
    fn of<'a>(fields: &'a [(String, Field)], name: &str) -> &'a Field {
        &fields.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("no field {name} in {fields:?}")).1
    }
}

#[test]
fn a_host_sets_a_composite_param_by_leaf_and_checks_its_where() {
    // Q2 (c), D13: `Top.g.a`, `Top.num[2]` and `Top.t.1` each write one leaf and leave the rest as
    // the source gave it
    let mut host = Host::open();
    host.session.set_at("Top.g.a", 7.0, 0.0).expect("g.a, a whole number of a u8");
    host.session.set_at("Top.num[2]", 0.5, 0.0).expect("num[2], a leaf of an array");
    host.session.set_at("Top.t.1", 1.0, 0.0).expect("t.1, a bool leaf: 1 is true");
    host.session.set_at("Top.g.armed", 1.0, 0.0).expect("g.armed");
    // §8.1 rule 8: the `where` is checked on the whole value after the write (E0317), and a refused write
    // changes nothing
    let (code, message) = host.refused("Top.g.a", 12.0);
    assert_eq!(code, "E0317", "{message}");
    assert!(message.contains("Top.g"), "the refusal names the param: {message}");
    // a path naming no leaf is refused naming it and the leaves there are
    let (code, message) = host.refused("Top.g.zz", 1.0);
    assert_eq!(code, "E0801", "{message}");
    assert!(message.contains("zz") && message.contains("`a`") && message.contains("`armed`"), "{message}");
    // a composite param named whole is refused, and a value its leaf cannot hold
    assert_eq!(host.refused("Top.g", 1.0).0, "E0802");
    assert_eq!(host.refused("Top.g.a", 2.5).0, "E0802", "a u8 leaf holds a whole number");
    assert_eq!(host.refused("Top.t.1", 2.0).0, "E0802", "a bool leaf holds 0 or 1");
    // the run reads what was written: g.a = 7, num[2] = 0.5, t.1 = true, g.on = true, the rest the defaults
    let seen = host.seen();
    assert_eq!(Seen::of(&seen, "a"), &Field::Int(7));
    assert_eq!(Seen::of(&seen, "n"), &Field::Real(0.5));
    assert_eq!(Seen::of(&seen, "t1"), &Field::Bool(true));
    assert_eq!(Seen::of(&seen, "armed"), &Field::Bool(true));
    assert_eq!(Seen::of(&seen, "b"), &Field::Real(0.5), "g.b is the default");
    assert_eq!(Seen::of(&seen, "t0"), &Field::Int(4), "t.0 is the default");
}

#[test]
fn a_timed_write_of_a_leaf_is_refused_by_name() {
    // a leaf is written before the run: the schedule holds no bytes of the rest of the param
    let mut host = Host::open();
    match host.session.set_at("Top.g.a", 5.0, 5e-8) {
        Err(Failure::Host { code, message }) => assert!(code == "E0806" && message.contains("composite"), "{code}: {message}"),
        other => panic!("a timed leaf write is refused: {other:?}"),
    }
}
