//! Before-run bindings, through `Session` (feature `analog-structure`, AS-15 AC 1): a binding of a
//! param that fixes structure is an elaboration input, so the design lowers the variant of its value
//! and the value holds from the start; a binding of any other param is still the host's write.
//! Expected values are the circuit's closed forms: 1 V behind 1 kOhm into the device under test.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Ignore;

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

/// A session whose params are bound before the run, and what it emitted.
struct BoundSession;

impl BoundSession {
    fn open(overrides: Overrides) -> Result<Session, piperine_api::Failure> {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, SOURCE).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        Session::open(&project, &spec, &CompileCache::shared().settings())
    }

    fn op_overrides(params: Vec<(String, f64)>) -> Overrides {
        Overrides { analysis: Some("op".into()), backend: Some("rust".into()), top: Some("Top".into()), print: vec!["a".into(), "c".into()], params, ..Overrides::default() }
    }

    fn run(params: Vec<(String, f64)>) -> Table {
        let mut session = BoundSession::open(BoundSession::op_overrides(params)).unwrap_or_else(|f| panic!("{f}"));
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"))
    }

    /// What a 3 ms transient of `WATCHED` records, as (stream, fields), after `params` bound before the run
    /// and a timed `set_at` of `Top.w.r` when `timed` gives (value, instant).
    fn watched(params: Vec<(String, f64)>, timed: Option<(f64, f64)>) -> Vec<(f64, String, Vec<(String, piperine_api::Field)>)> {
        #[derive(Default)]
        struct All(Vec<piperine_api::Record>);
        impl RecordSink for All {
            fn record(&mut self, record: &Record) {
                self.0.push(record.clone());
            }
        }
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, WATCHED).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some("transient".into()), stop: Some(3e-3), max_step: Some(1e-4), backend: Some("rust".into()), top: Some("Top".into()), params, ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
        if let Some((value, at)) = timed {
            session.set_at("Top.w.r", value, at).unwrap_or_else(|f| panic!("{f}"));
        }
        let kept = Rc::new(RefCell::new(All::default()));
        session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
        let records = kept.borrow().0.iter().map(|r| (r.time, r.stream.clone(), r.fields.clone())).collect();
        records
    }

    /// What a 1 ms transient of the lone module `Top` of `module` records, as (stream, fields), after `params`
    /// bound before the run.
    fn emitted(module: &str, params: Vec<(String, f64)>) -> Vec<(String, Vec<(String, piperine_api::Field)>)> {
        #[derive(Default)]
        struct All(Vec<piperine_api::Record>);
        impl RecordSink for All {
            fn record(&mut self, record: &Record) {
                self.0.push(record.clone());
            }
        }
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, format!("{}{module}", &WATCHED[..WATCHED.find("module Watch").expect("the parts")])).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some("transient".into()), stop: Some(1e-3), max_step: Some(1e-4), backend: Some("rust".into()), top: Some("Top".into()), params, ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
        let kept = Rc::new(RefCell::new(All::default()));
        session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
        let records = kept.borrow().0.iter().map(|r| (r.stream.clone(), r.fields.clone())).collect();
        records
    }
}

/// A real read out of a cell or a record field, and the closeness of two reals.
struct ValueReading;

impl ValueReading {
    fn real(cell: &Cell) -> f64 {
        match cell {
            Cell::Real(v) => *v,
            other => panic!("a real, found {other:?}"),
        }
    }

    fn close(got: f64, wanted: f64, what: &str) {
        assert!((got - wanted).abs() <= 1e-6, "{what}: {got} against {wanted}");
    }

    /// The value of the field `name` of a record's fields.
    fn field(fields: &[(String, piperine_api::Field)], name: &str) -> f64 {
        match fields.iter().find(|(n, _)| n == name) {
            Some((_, piperine_api::Field::Real(v))) => *v,
            other => panic!("a real field `{name}`, found {other:?}"),
        }
    }
}

#[test]
fn a_before_run_binding_of_a_structural_param_selects_its_variant() {
    // `short` selects the 0.25 V source by default: v(a) = 0.25. `--param Top.m.short=0` selects the
    // other variant at elaboration, the open branch, so v(a) is the source's 1 V (no current flows).
    let table = BoundSession::run(vec![]);
    ValueReading::close(ValueReading::real(&table.rows[0][1]), 0.25, "the default variant");
    let table = BoundSession::run(vec![("Top.m.short".into(), 0.0)]);
    ValueReading::close(ValueReading::real(&table.rows[0][1]), 1.0, "short = 0");
}

#[test]
fn a_write_of_a_structural_param_after_open_is_e0803() {
    // F1: `short` fixes the variant at elaboration, so a sweep of it, or a `set` between open and
    // the run, would run the variant of the old value: both are refused naming it (§23.1 rule 7)
    let sweep = Overrides { analysis: Some("dc".into()), sweep: Some("Top.m.short".into()), from: Some(0.0), to: Some(1.0), step: Some(1.0), backend: Some("rust".into()), top: Some("Top".into()), print: vec!["a".into()], ..Overrides::default() };
    match BoundSession::open(sweep) {
        Err(piperine_api::Failure::Host { code: "E0803", message }) => assert!(message.contains("Top.m.short"), "{message}"),
        other => panic!("a sweep of a key param is E0803, found {:?}", other.map(|_| ())),
    }
    let mut session = BoundSession::open(BoundSession::op_overrides(vec![])).unwrap_or_else(|f| panic!("{f}"));
    match session.set_at("Top.m.short", 0.0, 0.0) {
        Err(piperine_api::Failure::Host { code: "E0803", message }) => assert!(message.contains("Top.m.short"), "{message}"),
        other => panic!("a set of a key param is E0803, found {other:?}"),
    }
    session.set_at("Top.l.scale", 2.0, 0.0).expect("a plain param is still the host's write");
}

#[test]
fn a_before_run_binding_of_a_plain_param_is_still_the_hosts_write() {
    // `scale` is read by no selector: it is no key, the elaborator ignores the binding and the host
    // writes it before the run: the load is 1 kOhm / scale behind 1 kOhm, so `c` is 1/3 V with
    // `scale` = 2 (500 Ohm over 1.5 kOhm) and 0.5 V with 1.
    let table = BoundSession::run(vec![]);
    ValueReading::close(ValueReading::real(&table.rows[1][1]), 0.5, "scale = 1");
    let table = BoundSession::run(vec![("Top.l.scale".into(), 2.0)]);
    ValueReading::close(ValueReading::real(&table.rows[1][1]), 1.0 / 3.0, "scale = 2");
}

/// `Load` takes its `scale` from a body `let` of `Top` over the plain param `k` (AS-12 AC 4): the
/// host's `set` of `k` recomputes what the `let` gives the child.
const DERIVED: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }
analog for Resistor { i(p, n) +> v(p, n) / r; }
pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }
analog for Vsource { v(p, n) +> dc; }
pub module Load(a: Electrical, b: Electrical) { scale: f64 = 1.0; }
analog for Load { i(a, b) +> v(a, b) * scale / 1k; }
module Top() {
    k: f64 = 1.0;
    signal s: Electrical;
    signal c: Electrical;
    let derived = k * 3.0;
    let v1 = Vsource(s, gnd) { dc: 1.0 };
    let r2 = Resistor(s, c) { r: 1k };
    let l = Load(c, gnd) { scale: derived };
}
";

#[test]
fn a_binding_of_k_recomputes_the_scale_a_body_let_derives() {
    // AS-12 AC 4 (host-ii X9): `l.scale` is the body `let` `derived` = 3 * k. A divider of 1 kOhm over
    // a load of conductance scale / 1 kOhm gives v(c) = 1 V / (1 + scale): 0.25 V at k = 1 (scale 3), and
    // after the host binds `Top.k` = 2 the child's scale is recomputed to 6: v(c) = 1 / 7.
    let run_derived = |params: Vec<(String, f64)>| {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, DERIVED).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some("op".into()), backend: Some("rust".into()), top: Some("Top".into()), print: vec!["c".into()], params, ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"))
    };
    ValueReading::close(ValueReading::real(&run_derived(vec![]).rows[0][1]), 0.25, "k = 1");
    ValueReading::close(ValueReading::real(&run_derived(vec![("Top.k".into(), 2.0)]).rows[0][1]), 1.0 / 7.0, "k = 2");
}

/// A module `Watch` whose param `r` is read by three reactions: `changed` records the instant `r`
/// changed, `before` and `after` record `r.prev()` and `r` at 0.5 ms and at 2 ms (AS-14, §5.3.1).
const WATCHED: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }
analog for Resistor { i(p, n) +> v(p, n) / r; }
pub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }
analog for Vsource { v(p, n) +> dc; }
module Watch() { r: f64 = 2.0; }
digital for Watch {
    on r.changed() { emit!(\"changed\", t = now()); }
    on at(0.5m) { emit!(\"before\", prev = r.prev(), value = r); }
    on at(2m) { emit!(\"after\", prev = r.prev(), value = r); }
}
module Top() {
    signal a: Electrical;
    let v1 = Vsource(a, gnd) { dc: 1.0 };
    let r1 = Resistor(a, gnd) { r: 1k };
    let w = Watch();
}
";

#[test]
fn a_timed_set_fires_changed_once_at_its_time() {
    // AS-14 AC 1: `set_at("Top.w.r", 5.0, 1 ms)`: `on r.changed()` fires once, at 1 ms, and not at the
    // start of the run.
    let records = BoundSession::watched(vec![], Some((5.0, 1e-3)));
    let changed: Vec<f64> = records.iter().filter(|(_, stream, _)| stream == "changed").map(|(time, _, fields)| {
        assert!((ValueReading::field(fields, "t") - *time).abs() <= 1e-9, "the record carries its own instant");
        *time
    }).collect();
    assert_eq!(changed.len(), 1, "one change: {records:?}");
    assert!((changed[0] - 1e-3).abs() <= 1e-9, "at 1 ms, got {}", changed[0]);
}

#[test]
fn prev_of_a_param_reads_its_own_value_then_the_value_before_a_set() {
    // AS-14 AC 2: at 0.5 ms, before any `set`, `r.prev()` is the param's own value (2.0 = r); at 2 ms,
    // after the `set` to 5.0 at 1 ms, it is the value before the set (2.0) while `r` is 5.0.
    let records = BoundSession::watched(vec![], Some((5.0, 1e-3)));
    let at = |stream: &str| records.iter().find(|(_, s, _)| s == stream).map(|(_, _, fields)| (ValueReading::field(fields, "prev"), ValueReading::field(fields, "value"))).unwrap_or_else(|| panic!("no `{stream}` record in {records:?}"));
    assert_eq!(at("before"), (2.0, 2.0), "before any set: its own value");
    assert_eq!(at("after"), (2.0, 5.0), "after the set: the value before it");
}

#[test]
fn a_binding_before_the_run_fires_no_changed() {
    // AS-14 AC 3: `--param Top.w.r=7` is a binding before the run, not a change: no `changed` record,
    // and `r.prev()` at 0.5 ms is the bound value 7.0, its own value, as `r` is.
    let records = BoundSession::watched(vec![("Top.w.r".into(), 7.0)], None);
    assert!(records.iter().all(|(_, stream, _)| stream != "changed"), "no change fires: {records:?}");
    let before = records.iter().find(|(_, s, _)| s == "before").map(|(_, _, f)| (ValueReading::field(f, "prev"), ValueReading::field(f, "value"))).expect("a `before` record");
    assert_eq!(before, (7.0, 7.0), "the bound value is the param's own");
}

#[test]
fn an_initial_value_over_a_param_follows_its_binding() {
    // AS-15 AC 1: `signal s: Bit<f64> = k0;` holds `k0` from (0, 0): 2.5 by default, and 4.0 after
    // `--param Top.k0=4.0` (the param is structural: the binding selects the variant that folds it).
    let top = "module Top() {\n    k0: f64 = 2.5;\n    signal s: Bit<f64> = k0;\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, gnd) { r: 1k };\n}\ndigital for Top { on at(0.5m) { emit!(\"s\", v = s); } }\n";
    let value = |records: &[(String, Vec<(String, piperine_api::Field)>)]| ValueReading::field(&records.iter().find(|(s, _)| s == "s").expect("an `s` record").1, "v");
    assert_eq!(value(&BoundSession::emitted(top, vec![])), 2.5, "the default");
    assert_eq!(value(&BoundSession::emitted(top, vec![("Top.k0".into(), 4.0)])), 4.0, "after --param Top.k0=4.0");
    // and from (0, 0): the record at 0 reads the same value, not one reached later
    let at_zero = top.replace("on at(0.5m)", "on at(0.0)");
    assert_eq!(value(&BoundSession::emitted(&at_zero, vec![("Top.k0".into(), 4.0)])), 4.0, "at (0, 0)");
}
