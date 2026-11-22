//! Scope: a host and the composite values of a design (composite-values CV-18, Q2 (c), D13): a
//! composite param is set by the path of one leaf, `Top.num[2]`, `Top.g.a`, `Top.t.1`, and its
//! `where` is checked on the whole value after the write (`E0317`). The design is MSL written by
//! the test, a reaction emits what it reads of each param, and an expected value is the value the
//! test wrote, cited at each assertion.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{CellShape, Cell, Failure, Field, Overrides, ParamValue, Project, Record, RecordSink, Session};
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
    /// The session of `DESIGN`, its `clk` driven low and then high at 100 ns.
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
    /// The field `name` of `fields`; the test fails naming it when there is none.
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

/// A unit enum, a struct and an enum with a payload held by carriers a reaction commits, run as a
/// transient that sees one rising edge of `clk` at 100 ns.
const CARRIED: &str = "#[derive(Repr, PartialEq)]\nenum Mode { Idle, Regulate, Fault }\n#[derive(Repr, PartialEq)]\nstruct Pair { a: u8, b: bool }\n#[derive(Repr, PartialEq)]\nenum Tx { Idle, Data { n: u3 } }\n\
module Top(in clk: Bit, out mode: Bit<Mode>, out pair: Bit<Pair>, out tx: Bit<Tx>) {}\n\
digital for Top {\n    on clk.rise() {\n        mode <- Mode::Regulate;\n        pair <- Pair { a: 7, b: true };\n        tx <- Tx::Data { n: 5 };\n    }\n}\n";

/// The session of `CARRIED` printing `print`, and the last row of its table.
struct Carried {
    session: Session,
}

impl Carried {
    /// The session of `CARRIED`, its `clk` driven low and then high at 100 ns, printing `print`.
    fn open(print: &[&str]) -> Carried {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, CARRIED).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides {
            backend: Some("rust".into()),
            top: Some("Top".into()),
            analysis: Some("transient".into()),
            stop: Some(2e-7),
            max_step: Some(1e-8),
            print: print.iter().map(|s| s.to_string()).collect(),
            ..Overrides::default()
        };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        session.drive("clk", ParamValue::Bool(false), 0.0).expect("drive clk low");
        session.drive("clk", ParamValue::Bool(true), 1e-7).expect("drive clk high");
        Carried { session }
    }

    /// The last row of the run's table: each printed signal's final value.
    fn last(&mut self) -> (Vec<String>, Vec<Cell>) {
        let table = self.session.run(Rc::new(RefCell::new(Kept::default()))).unwrap_or_else(|f| panic!("{f}"));
        (table.columns.clone(), table.rows.last().expect("a row").clone())
    }
}

#[test]
fn a_carried_enum_reads_as_its_variant_name() {
    // CV-22 AC 1: a unit enum reads as its variant's name, not its tag (`Regulate`, not 1); an enum
    // with a payload and a struct read as composite values that print as the language's literals; a
    // path naming a leaf (`pair.a`, `tx.Data.n`) is that leaf alone
    let mut run = Carried::open(&["mode", "pair", "tx", "pair.a", "tx.Data.n"]);
    let (columns, row) = run.last();
    assert_eq!(columns, ["time", "Top.mode", "Top.pair", "Top.tx", "Top.pair.a", "Top.tx.Data.n"]);
    assert_eq!(row[1], Cell::Text("Regulate".into()), "the variant's name");
    let literal = |cell: &Cell| match cell {
        Cell::Composite(value) => value.literal(&|leaf| match leaf {
            Cell::Real(v) => v.to_string(),
            Cell::Bool(v) => v.to_string(),
            Cell::Int(v) => v.to_string(),
            other => format!("{other:?}"),
        }),
        other => panic!("a composite, found {other:?}"),
    };
    assert_eq!(literal(&row[2]), "Pair { a: 7, b: true }");
    assert_eq!(literal(&row[3]), "Tx::Data { n: 5 }");
    assert_eq!(row[4], Cell::Int(7), "pair.a is the leaf alone");
    assert_eq!(row[5], Cell::Int(5), "tx.Data.n is the leaf alone");
}

#[test]
fn enum_names_live_in_the_ir_not_in_the_net_bytes() {
    // CV-22 AC 2: the type arena holds each enum's variant names; the bytes of its net are the tag
    // and the known byte only, so a decoder with the names reads the variant and one without them
    // could not
    let run = Carried::open(&["mode"]);
    let ir = run.session.ir();
    let mode = ir.top().body().carriers().iter().find(|c| c.name == "mode").expect("the carrier mode").ty.clone();
    let shape = CellShape::of(ir, &mode).expect("a shape");
    assert_eq!(shape.decode(&[1, 1]).expect("decodes"), Cell::Text("Regulate".into()), "tag 1, known: the second variant");
    assert_eq!(shape.decode(&[2, 1]).expect("decodes"), Cell::Text("Fault".into()));
    assert_eq!(shape.decode(&[3, 1]).expect("decodes"), Cell::Unknown, "tag 3 names no variant");
    let names: Vec<String> = match &shape {
        CellShape::Enum { variants, tag_width, .. } => {
            assert_eq!(*tag_width, 1, "the net holds a one-byte tag");
            variants.iter().map(|v| v.name.clone()).collect()
        }
        other => panic!("an enum shape, found {other:?}"),
    };
    assert_eq!(names, ["Idle", "Regulate", "Fault"]);
}
