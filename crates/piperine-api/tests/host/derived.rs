//! Scope: a host `set` recomputes the params derived from the one it changes (feature `host-ii`,
//! X9, HII-18 to HII-20): `--param`, the `dc` sweep and a timed `Session::set_at` of `Mid`'s `rr`
//! reach the `Resistor` `Mid` places with `r: rr` (S2 of `GAPS.md`). Expected values are the
//! divider formulas: 1 V over `R1 = 1 kOhm` into the child `r` to ground gives `r / (r + 1k)`.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Failure, Overrides, Project, Session, Table};
use piperine_transpile::Settings;
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Ignore;

const PARTS: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// Associated test helpers of `derived`, grouped on one fixture type.
struct DerivedFixture;

impl DerivedFixture {
    /// The one design: `Top` is `Vsource` at `a`, `R1 = 1k` from `a` to `b`, and `Mid` from `b` to
    /// ground, whose resistor is `r: <derived>`; `Mid`'s own behavior reads `rr`, so a `set` of `rr`
    /// has a pin (without it the run stops at the known H5 error).
    fn source(derived: &str, rr: &str) -> String {
        format!(
            "{PARTS}module Mid(p: Electrical, n: Electrical) {{\n    rr: f64 = {rr};\n    let res = Resistor(p, n) {{ r: {derived} }};\n}}\nanalog for Mid {{ i(p, n) +> 0.0 * rr; }}\n\nmodule Top() {{\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, b) {{ r: 1k }};\n    let m = Mid(b, gnd);\n}}\n"
        )
    }

    fn open(text: &str, overrides: Overrides) -> Session {
        DerivedFixture::open_with(text, overrides, &CompileCache::shared().settings())
    }

    /// [`open`] building into `settings`' cache.
    fn open_with(text: &str, overrides: Overrides, settings: &Settings) -> Session {
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, text).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let print = if overrides.print.is_empty() { vec!["b".into()] } else { overrides.print.clone() };
        let backend = overrides.backend.clone().or(Some("rust".into()));
        let overrides = Overrides { backend, top: Some("Top".into()), print, ..overrides };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        Session::open(&project, &spec, settings).unwrap_or_else(|f| panic!("{f}"))
    }

    fn op() -> Overrides {
        Overrides { analysis: Some("op".into()), ..Overrides::default() }
    }

    fn run(session: &mut Session) -> Result<Table, Failure> {
        session.run(Rc::new(RefCell::new(Ignore)))
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

    /// Every file under `dir`, by path.
    fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir).expect("a readable cache").flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(DerivedFixture::walk(&path));
            } else {
                found.push(path);
            }
        }
        found
    }

    /// `Top` holds the params `q` and `k`, which no device pin reads, and places `Mid` with `rr: q * k`;
    /// `Mid` places the resistor with `r: rr`. Two levels derive from two inputs: 1 V over `R1 = 1 kOhm`
    /// into `q * k` to ground gives `q * k / (q * k + 1k)`.
    fn product_source() -> String {
        format!(
            "{PARTS}module Mid(p: Electrical, n: Electrical) {{\n    rr: f64 = 1k;\n    let res = Resistor(p, n) {{ r: rr }};\n}}\nanalog for Mid {{ i(p, n) +> 0.0 * rr; }}\n\nmodule Top() {{\n    q: f64 = 500.0;\n    k: f64 = 1.0;\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, b) {{ r: 1k }};\n    let m = Mid(b, gnd) {{ rr: q * k }};\n}}\n"
        )
    }

    /// A transient to 3 us with a row each microsecond, printing `b` and the param `q`.
    fn product_run() -> Overrides {
        Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), print: vec!["b".into(), "Top.q".into()], ..Overrides::default() }
    }

    fn column(table: &Table, at: usize) -> Vec<f64> {
        table.rows.iter().map(|r| DerivedFixture::real(&r[at])).collect()
    }

    /// `Mid` conducts `v / rr` itself, so `rr` is visible at the node: the circuit's `rr` is read back
    /// through the node after a refused set. Its child is `r: rr - 500`, which must be above 0.
    fn observable_source() -> String {
        format!(
            "{PARTS}module Mid(p: Electrical, n: Electrical) {{\n    rr: f64 = 1500.0;\n    let res = Resistor(p, n) {{ r: rr - 500.0 }};\n}}\nanalog for Mid {{ i(p, n) +> v(p, n) / rr; }}\n\nmodule Top() {{\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, b) {{ r: 1k }};\n    let m = Mid(b, gnd);\n}}\n"
        )
    }
}

#[test]
fn the_s2_probe_prints_three_quarters() {
    // HII-18 AC 1 and 6: `--param Top.m.rr=3000` reaches the child (`r = rr`): the middle node is
    // 3000 / (3000 + 1000) = 0.75 (it was 0.5 with the child at its elaborated 1k); with no `set`
    // the design prints what it always printed, 0.5 (HII-20 AC 3)
    let mut set = DerivedFixture::open(&DerivedFixture::source("rr", "1k"), Overrides { params: vec![("Top.m.rr".into(), 3000.0)], ..DerivedFixture::op() });
    let table = DerivedFixture::run(&mut set).unwrap_or_else(|f| panic!("{f}"));
    DerivedFixture::close(DerivedFixture::real(&table.rows[0][1]), 0.75, "rr = 3000");
    let mut plain = DerivedFixture::open(&DerivedFixture::source("rr", "1k"), DerivedFixture::op());
    let table = DerivedFixture::run(&mut plain).unwrap_or_else(|f| panic!("{f}"));
    DerivedFixture::close(DerivedFixture::real(&table.rows[0][1]), 0.5, "no set");
}

#[test]
fn a_sweep_recomputes_the_child_at_each_point() {
    // HII-18 AC 2: three points of `rr`, three different rows: r / (r + 1k) for 1k, 2k, 3k
    let sweep = Overrides { analysis: Some("dc".into()), sweep: Some("Top.m.rr".into()), from: Some(1000.0), to: Some(3000.0), step: Some(1000.0), ..Overrides::default() };
    let mut session = DerivedFixture::open(&DerivedFixture::source("rr", "1k"), sweep);
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| DerivedFixture::real(&r[1])).collect();
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 2.0 / 3.0, 0.75]) {
        DerivedFixture::close(*got, wanted, "a sweep point");
    }
}

#[test]
fn a_timed_set_recomputes_at_the_same_instant() {
    // HII-18 AC 3: `rr = 3000` at 1.5 us: the grid rows at 0 and 1 us hold 0.5, the rows at 2 and
    // 3 us hold 0.75, so the child moved with its parent at that instant
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), ..Overrides::default() };
    let mut session = DerivedFixture::open(&DerivedFixture::source("rr", "1k"), tran);
    session.set_at("Top.m.rr", 3000.0, 1.5e-6).unwrap_or_else(|f| panic!("{f}"));
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| DerivedFixture::real(&r[1])).collect();
    assert_eq!(rows.len(), 4, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 0.5, 0.75, 0.75]) {
        DerivedFixture::close(*got, wanted, "a grid row");
    }
}

#[test]
fn a_child_failing_where_refuses_the_set() {
    // HII-18 AC 4: the child's `r = rr - 500` must satisfy `r > 0`; `rr = 400` makes it -100: E0802
    // naming the child's param, and nothing changed: the run still gives r = 1000 (rr = 1500)
    let mut session = DerivedFixture::open(&DerivedFixture::source("rr - 500.0", "1500.0"), DerivedFixture::op());
    match session.set_at("Top.m.rr", 400.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.m.res.r"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    DerivedFixture::close(DerivedFixture::real(&table.rows[0][1]), 0.5, "the refused set changed nothing");
    // and the derived param itself is no door: HII-20 AC 1
    let mut direct = DerivedFixture::open(&DerivedFixture::source("rr - 500.0", "1500.0"), DerivedFixture::op());
    match direct.set_at("Top.m.res.r", 2000.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("rr") && message.contains("Top.m"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
}

#[test]
fn a_set_neither_elaborates_nor_compiles() {
    // HII-18 AC 5: the cache holds the same entries after a `set` and a run as before them: no
    // second `rustc` build and no new design (a cache of its own: the other tests of this target
    // build into the shared one while this runs)
    let private = tempfile::tempdir().expect("tmp");
    let entries = || {
        let mut names: Vec<_> = DerivedFixture::walk(private.path());
        names.sort();
        names
    };
    let mut session = DerivedFixture::open_with(&DerivedFixture::source("rr", "1k"), DerivedFixture::op(), &Settings { rustc: None, cache: private.path().to_path_buf() });
    let before = entries();
    assert!(!before.is_empty(), "the open compiled the design into the cache");
    session.set_at("Top.m.rr", 2000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.m.rr", 3000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(entries(), before);
}

/// `Mid` is a resistor of `rr` written through a body `let`: `g = 1 / rr` inside its behavior.
const LET_DESIGN: &str = "module Mid(p: Electrical, n: Electrical) {\n    rr: f64 = 1k;\n}\nanalog for Mid { let g = 1.0 / rr; i(p, n) +> v(p, n) * g; }\n\nmodule Top() {\n    signal a: Electrical;\n    signal b: Electrical;\n    let v1 = Vsource(a, gnd) { dc: 1.0 };\n    let r1 = Resistor(a, b) { r: 1k };\n    let m = Mid(b, gnd);\n}\n";

#[test]
fn a_body_let_reads_the_new_param() {
    // HII-19 AC 1: `g = 1.0 / rr` is a `let` of `Mid`'s behavior reading `rr`; after `rr = 3000`
    // it reads 3000 on every evaluation, so the node is 3000 / (3000 + 1000) = 0.75 (0.5 before)
    let text = format!("{PARTS}{LET_DESIGN}");
    let mut before = DerivedFixture::open(&text, DerivedFixture::op());
    DerivedFixture::close(DerivedFixture::real(&DerivedFixture::run(&mut before).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.5, "rr = 1k");
    let mut after = DerivedFixture::open(&text, DerivedFixture::op());
    after.set_at("Top.m.rr", 3000.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    DerivedFixture::close(DerivedFixture::real(&DerivedFixture::run(&mut after).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.75, "rr = 3000");
}

/// `Scale` defines `y = a * k`: a definition of a carrier that reads the param `k`; the top's `a`
/// is driven by the host.
const DEFINITION_DESIGN: &str = "module Scale(in a: Bit<f64>, out y: Bit<f64>) {\n    k: f64 = 2.0;\n    y = a * k;\n}\n\nmodule Top(in a: Bit<f64>, out y: Bit<f64>) {\n    signal s: Electrical;\n    let v1 = Vsource(s, gnd) { dc: 1.0 };\n    let r1 = Resistor(s, gnd) { r: 1k };\n    let sc = Scale(a, y);\n}\n";

#[test]
fn a_definition_is_recomputed_at_the_set() {
    // HII-19 AC 2: `y = a * k` with `a = 4`: 8 while `k = 2`; `k = 5` at 1.5 us recomputes the
    // definition at that instant: the grid rows at 0 and 1 us hold 8, the rows at 2 and 3 us hold 20
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), print: vec!["y".into()], ..Overrides::default() };
    let mut session = DerivedFixture::open(&format!("{PARTS}{DEFINITION_DESIGN}"), tran);
    session.drive("a", piperine_api::ParamValue::Real(4.0), 0.0).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.sc.k", 5.0, 1.5e-6).unwrap_or_else(|f| panic!("{f}"));
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = table.rows.iter().map(|r| DerivedFixture::real(&r[1])).collect();
    assert_eq!(rows, [8.0, 8.0, 20.0, 20.0]);
}

/// The child's `r` written with a method call over `rr`: the form the walk over a placement's
/// expression once skipped, which left the binding with no reads and the child at its folded 1k.
const METHOD_PLACEMENT: &str = "rr.max(10.0)";

#[test]
fn a_method_call_placement_derives_through_a_param_flag() {
    // HII-18 AC 1 and 6 (F1): `r: rr.max(10.0)` with `--param Top.m.rr=3000` is 3000, so the
    // middle node is 0.75 (it printed 0.5 while the walk skipped the method)
    let mut session = DerivedFixture::open(&DerivedFixture::source(METHOD_PLACEMENT, "1k"), Overrides { params: vec![("Top.m.rr".into(), 3000.0)], ..DerivedFixture::op() });
    DerivedFixture::close(DerivedFixture::real(&DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.75, "rr = 3000");
}

#[test]
fn a_method_call_placement_derives_at_each_sweep_point() {
    // HII-18 AC 2 (F1): three points, three different rows 1k, 2k, 3k: r / (r + 1k)
    let sweep = Overrides { analysis: Some("dc".into()), sweep: Some("Top.m.rr".into()), from: Some(1000.0), to: Some(3000.0), step: Some(1000.0), ..Overrides::default() };
    let mut session = DerivedFixture::open(&DerivedFixture::source(METHOD_PLACEMENT, "1k"), sweep);
    let rows: Vec<f64> = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}")).rows.iter().map(|r| DerivedFixture::real(&r[1])).collect();
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 2.0 / 3.0, 0.75]) {
        DerivedFixture::close(*got, wanted, "a sweep point");
    }
}

#[test]
fn a_method_call_placement_derives_in_a_timed_set() {
    // HII-18 AC 3 (F1): `rr = 3000` at 1.5 us moves the child at that instant
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), ..Overrides::default() };
    let mut session = DerivedFixture::open(&DerivedFixture::source(METHOD_PLACEMENT, "1k"), tran);
    session.set_at("Top.m.rr", 3000.0, 1.5e-6).unwrap_or_else(|f| panic!("{f}"));
    let rows: Vec<f64> = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}")).rows.iter().map(|r| DerivedFixture::real(&r[1])).collect();
    assert_eq!(rows.len(), 4, "{rows:?}");
    for (got, wanted) in rows.iter().zip([0.5, 0.5, 0.75, 0.75]) {
        DerivedFixture::close(*got, wanted, "a grid row");
    }
}

#[test]
fn the_interpreter_refuses_a_set_a_method_call_placement_derives_from() {
    // HII-20 AC 2 (F1): the interpreter does not recompute, so it refuses and names the child
    // param and `--backend rust` (it accepted the set and printed the stale 0.5)
    let mut session = DerivedFixture::open(&DerivedFixture::source(METHOD_PLACEMENT, "1k"), Overrides { backend: Some("interp".into()), ..DerivedFixture::op() });
    match session.set_at("Top.m.rr", 3000.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.m.res.r") && message.contains("--backend rust"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
}

#[test]
fn a_timed_set_derives_against_the_sets_due_before_it() {
    // HII-18 AC 3 and HII-13 (F2): `q = 1500` at 2 us given BEFORE `k = 3` at 0. At the time of `q`
    // the table holds k = 3, so rr = 1500 * 3 = 4500 after 2 us: 0.6 (q * k = 500 * 3) at the rows
    // of 0, 1 and 2 us (the row at 2 us holds the old value), 4500 / 5500 = 0.818 at 3 us
    let mut session = DerivedFixture::open(&DerivedFixture::product_source(), DerivedFixture::product_run());
    session.set_at("Top.q", 1500.0, 2e-6).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.k", 3.0, 0.0).unwrap_or_else(|f| panic!("{f}"));
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    for (got, wanted) in DerivedFixture::column(&table, 1).iter().zip([0.6, 0.6, 0.6, 4500.0 / 5500.0]) {
        DerivedFixture::close(*got, wanted, "a grid row of b");
    }
    assert_eq!(DerivedFixture::column(&table, 2), [500.0, 500.0, 500.0, 1500.0], "the param q, in force at each row");
}

#[test]
fn a_timed_set_given_out_of_time_order_derives_in_time_order() {
    // HII-18 AC 3 (F2): `q = 1500` at 2 us given first, `k = 3` at 1 us after it. In time order k
    // changes after 1 us (rr = 500 * 3 = 1500) and q after 2 us (rr = 4500): the rows of 0 and 1 us
    // hold 500 / 1500, the row of 2 us 1500 / 2500 = 0.6, the row of 3 us 4500 / 5500
    let mut session = DerivedFixture::open(&DerivedFixture::product_source(), DerivedFixture::product_run());
    session.set_at("Top.q", 1500.0, 2e-6).unwrap_or_else(|f| panic!("{f}"));
    session.set_at("Top.k", 3.0, 1e-6).unwrap_or_else(|f| panic!("{f}"));
    let table = DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}"));
    for (got, wanted) in DerivedFixture::column(&table, 1).iter().zip([1.0 / 3.0, 1.0 / 3.0, 0.6, 4500.0 / 5500.0]) {
        DerivedFixture::close(*got, wanted, "a grid row of b");
    }
}

#[test]
fn metadata_lists_a_derived_param_at_the_value_the_run_uses() {
    // HII-10 AC 1 and HII-11 (F3): `q = 1500` of the run reaches `Top.m.rr` (q * k = 1500) and
    // `Top.m.res.r`, both of which the metadata listed at their elaborated 500
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, DerivedFixture::product_source()).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), top: Some("Top".into()), backend: Some("rust".into()), params: vec![("Top.q".into(), 1500.0)], ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let described = Session::describe(&project, &spec, &|| Ok(CompileCache::shared().settings())).unwrap_or_else(|f| panic!("{f}")).1;
    let session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
    for metadata in [described, session.metadata()] {
        let value = |path: &str| metadata.params.iter().find(|p| p.path == path).map(|p| p.value.clone());
        assert_eq!(value("Top.q"), Some(Cell::Real(1500.0)));
        assert_eq!(value("Top.m.rr"), Some(Cell::Real(1500.0)));
        assert_eq!(value("Top.m.res.r"), Some(Cell::Real(1500.0)));
    }
}

#[test]
fn a_param_no_pin_holds_prints_the_value_the_run_used() {
    // HII-09 AC 1 (F4): `Top.q` is read by a placement only, so no net holds it: an `op` with
    // `q = 1500` prints 1500, a `dc` sweep prints each point, and the transient's timed set shows
    // 500 until the step after 2 us (the print failed with "no net of the digital plan has that path")
    let print = || vec!["b".to_string(), "Top.q".to_string()];
    let mut set = DerivedFixture::open(&DerivedFixture::product_source(), Overrides { print: print(), params: vec![("Top.q".into(), 1500.0)], ..DerivedFixture::op() });
    DerivedFixture::close(DerivedFixture::real(&DerivedFixture::run(&mut set).unwrap_or_else(|f| panic!("{f}")).rows[1][1]), 1500.0, "op");
    let sweep = Overrides { analysis: Some("dc".into()), sweep: Some("Top.q".into()), from: Some(1000.0), to: Some(3000.0), step: Some(1000.0), print: print(), ..Overrides::default() };
    let table = DerivedFixture::run(&mut DerivedFixture::open(&DerivedFixture::product_source(), sweep)).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(DerivedFixture::column(&table, 2), [1000.0, 2000.0, 3000.0], "dc");
    let mut timed = DerivedFixture::open(&DerivedFixture::product_source(), DerivedFixture::product_run());
    timed.set_at("Top.q", 1500.0, 2e-6).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(DerivedFixture::column(&DerivedFixture::run(&mut timed).unwrap_or_else(|f| panic!("{f}")), 2), [500.0, 500.0, 500.0, 1500.0], "transient");
}

#[test]
fn a_refused_set_leaves_the_param_the_circuit_reads_as_it_was() {
    // HII-18 AC 4 (F12): `rr = 400` writes `Mid`'s own `rr`, then its child `r = -100` fails
    // `r > 0`: E0802 naming the child, and the writes already made are undone. The node shows it:
    // 1 kOhm from 1 V into `1500 || 1000 = 600 Ohm` gives 600 / 1600 = 0.375; with `rr` left at
    // 400 it would be 400 || 1000 = 285.7 Ohm, 0.222
    let mut session = DerivedFixture::open(&DerivedFixture::observable_source(), DerivedFixture::op());
    match session.set_at("Top.m.rr", 400.0, 0.0) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.m.res.r"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
    DerivedFixture::close(DerivedFixture::real(&DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}")).rows[0][1]), 0.375, "the refused set changed nothing");
}

#[test]
fn a_timed_set_whose_derived_child_fails_where_is_refused() {
    // HII-18 AC 3 and 4 (F12): the same `rr = 400` at 1.5 us: the child's `r = -100` fails its
    // `where` when the set is given, E0802 naming it, and the run then holds 0.375 at every row
    let tran = Overrides { analysis: Some("transient".into()), stop: Some(3e-6), max_step: Some(1e-7), print_step: Some(1e-6), ..Overrides::default() };
    let mut session = DerivedFixture::open(&DerivedFixture::observable_source(), tran);
    match session.set_at("Top.m.rr", 400.0, 1.5e-6) {
        Err(Failure::Host { code: "E0802", message }) => assert!(message.contains("Top.m.res.r"), "{message}"),
        other => panic!("expected E0802, found {other:?}"),
    }
    let rows = DerivedFixture::column(&DerivedFixture::run(&mut session).unwrap_or_else(|f| panic!("{f}")), 1);
    assert_eq!(rows.len(), 4, "{rows:?}");
    for got in rows {
        DerivedFixture::close(got, 0.375, "a row of the run the refused set did not touch");
    }
}
