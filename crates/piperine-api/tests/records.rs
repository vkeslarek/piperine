//! `emit!` records from a run (CLI-06 AC 1, 3 to 6 and 8; spec edge 7): `Session::run` hands
//! every record to the host's sink as the solver delivers it, decoded by its site
//! (`Ir::record_at`) and its stream's layout (contract D1, D2). Each fixture is MSL written by
//! the test; its expected records are read from that source and from `std/io.msl` (`require`
//! emits `checks` with `kind`, `name` and `pass`) and `docs/spec/part_vi_io.md`, cited at each
//! assertion. Every session builds on the Rust backend into a cache directory of this target.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use piperine_api::{Cell, Field, Failure, Overrides, Project, Record, RecordSink, Session, Streams};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind};
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

/// A source across a resistor: the analog part every fixture's top holds.
const ANALOG: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

fn settings() -> Settings {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
    Settings { rustc: None, cache: dir.path().to_path_buf() }
}

/// `module Top() { <a source and a resistor>; <placements> }` over the modules `modules`.
fn design(modules: &str, placements: &str) -> String {
    format!("{ANALOG}\n{modules}\nmodule Top() {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n{placements}}}\n")
}

/// The records of `analysis` (to `stop` for a transient) of the lone file `path`, top `Top`.
fn records_of(path: &Path, top: &str, analysis: &str, stop: Option<f64>) -> Result<Vec<Record>, Failure> {
    let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some(analysis.into()), stop, top: Some(top.into()), max_step: stop.map(|s| s / 100.0), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings())?;
    let kept = Rc::new(RefCell::new(Kept::default()));
    session.run(kept.clone())?;
    let records = kept.borrow().0.clone();
    Ok(records)
}

/// The records of a 5 us transient of `text`.
fn records(text: &str) -> Vec<Record> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    records_of(&file, "Top", "transient", Some(5e-6)).unwrap_or_else(|f| panic!("{f}"))
}

#[test]
fn an_emit_gives_one_record_with_time_instance_stream_and_fields() {
    // CLI-06 AC 1: one timer at 1 us, one emit with `a` then `b`
    let text = design("module M() {}\ndigital for M { on at(1u) { emit!(\"s\", a = 1.5, b = true); } }\n", "    let m = M();\n");
    let all = records(&text);
    assert_eq!(all.len(), 1, "{all:?}");
    let r = &all[0];
    assert!((r.time - 1e-6).abs() <= 1e-15, "the timer's instant, to the solver's tick: {}", r.time);
    assert_eq!((r.instance.as_str(), r.stream.as_str()), ("Top.m", "s"));
    assert_eq!(r.fields, [("a".to_string(), Field::Real(1.5)), ("b".to_string(), Field::Bool(true))]);

    // task T14: bytes of another length than the stream's layout are a failure, never a
    // truncated record. `s` is `a: f64` (16 bytes: value, known byte, padding) then `b: bool`
    // (2 bytes), 24 bytes in all; the record above is decoded from such bytes, one short is not
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, &text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let session = {
                Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"))
    };
    let streams = Streams::of(Arc::new(session.ir().clone()));
    let mut bytes = vec![0u8; 24];
    bytes[..8].copy_from_slice(&1.5f64.to_le_bytes());
    bytes[8] = 1;
    bytes[16..18].copy_from_slice(&[1, 1]);
    let observation = |bytes: Vec<u8>| Observation {
        path: "Top.m.s".into(),
        analysis: AnalysisKind::Transient,
        time: 1e-6,
        kind: ObservationKind::Record { site: Some((0, 0)), bytes },
    };
    let whole = streams.decode(&observation(bytes.clone())).expect("a record").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(whole.fields, r.fields, "24 bytes decode as the run's record");
    bytes.pop();
    let short = streams.decode(&observation(bytes)).expect("a record observation");
    assert!(matches!(&short, Err(Failure::Backend(m)) if m.contains("23 bytes")), "{short:?}");
}

#[test]
fn maybe_is_decoded_beside_the_fields() {
    // US-05 AC 1, 2 and §19 rule 1: `maybe` is the last field of the stream's layout, a `bool`
    // laid out as a carrier; the host reads it into `Record::maybe`, and it is no entry of
    // `fields`. `s` is `a: f64` (bytes 0..16), `b: bool` (16..18), `maybe` (18..20), 24 bytes
    let text = design("module M() {}\ndigital for M { on at(1u) { emit!(\"s\", a = 1.5, b = true); } }\n", "    let m = M();\n");
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, &text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
    let streams = Streams::of(Arc::new(session.ir().clone()));
    let decode = |maybe: [u8; 2]| {
        let mut bytes = vec![0u8; 24];
        bytes[..8].copy_from_slice(&1.5f64.to_le_bytes());
        bytes[8] = 1;
        bytes[16..18].copy_from_slice(&[1, 1]);
        bytes[18..20].copy_from_slice(&maybe);
        let observation = Observation { path: "Top.m.s".into(), analysis: AnalysisKind::Transient, time: 1e-6, kind: ObservationKind::Record { site: Some((0, 0)), bytes } };
        streams.decode(&observation).expect("a record").unwrap_or_else(|f| panic!("{f}"))
    };
    let fields = [("a".to_string(), Field::Real(1.5)), ("b".to_string(), Field::Bool(true))];
    let reached = decode([1, 1]);
    assert!(reached.maybe, "a known true `maybe` byte pair");
    assert_eq!(reached.fields, fields, "`maybe` is kept out of the fields");
    let sure = decode([0, 1]);
    assert!(!sure.maybe, "a known false `maybe`");
    assert_eq!(sure.fields, fields);
}

#[test]
fn records_arrive_in_time_order() {
    // CLI-06 AC 3: timers written at 3 us, 1 us, 2 us arrive at 1, 2, 3 us
    let m = "module M() {}\ndigital for M {\n    on at(3u) { emit!(\"s\", k = 3); }\n    on at(1u) { emit!(\"s\", k = 1); }\n    on at(2u) { emit!(\"s\", k = 2); }\n}\n";
    let all = records(&design(m, "    let m = M();\n"));
    let seen: Vec<(i64, Field)> = all.iter().map(|r| ((r.time * 1e9).round() as i64, r.fields[0].1.clone())).collect();
    assert_eq!(seen, [(1000, Field::Int(1)), (2000, Field::Int(2)), (3000, Field::Int(3))]);
}

#[test]
fn a_text_field_is_the_sources_literal() {
    // CLI-06 AC 4: a text never crosses the ABI; it is the site's literal
    let m = "module M() {}\ndigital for M { on at(1u) { emit!(\"log\", msg = \"overrange\"); } }\n";
    let all = records(&design(m, "    let m = M();\n"));
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].fields, [("msg".to_string(), Field::Text("overrange".into()))]);
}

#[test]
fn a_failed_require_is_one_checks_record_with_pass_false() {
    // CLI-06 AC 6; std/io.msl: `require(name, pass)` is `emit!("checks", kind = "require",
    // name = name, pass = pass)`
    let m = "module M() {}\ndigital for M { on at(1u) { require(\"lim\", false); } }\n";
    let all = records(&design(m, "    let m = M();\n"));
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].stream, "checks");
    assert_eq!(
        all[0].fields,
        [("kind".to_string(), Field::Text("require".into())), ("name".to_string(), Field::Text("lim".into())), ("pass".to_string(), Field::Bool(false))]
    );
}

#[test]
fn a_design_without_emit_gives_no_record() {
    // CLI-06 AC 8: the divider twin has no `emit!` site
    let divider: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins/divider.msl");
    let all = records_of(&divider, "Divider", "op", None).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(all, []);
}

#[test]
fn a_reserved_stream_keeps_its_name_and_its_site() {
    // spec edge 7: `checks` is reserved (docs/spec/part_vi_io.md); its record keeps the stream
    // name and the instance path of the reaction that wrote it, here the second of two monitors
    let m = "module M() { ok: bool = true; }\ndigital for M { on at(1u) { require(\"held\", ok); } }\n";
    let all = records(&design(m, "    let first = M();\n    let second = M() { ok: false };\n"));
    let mut seen: Vec<(&str, &str, &Field)> = all.iter().map(|r| (r.instance.as_str(), r.stream.as_str(), &r.fields[2].1)).collect();
    seen.sort_by_key(|(i, _, _)| *i);
    assert_eq!(seen, [("Top.first", "checks", &Field::Bool(true)), ("Top.second", "checks", &Field::Bool(false))]);
}

/// The records of a 5 us transient of `text`, the run keeping only the streams `keep` names.
fn kept_records(text: &str, keep: &[&str]) -> Result<Vec<Record>, Failure> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        analysis: Some("transient".into()),
        stop: Some(5e-6),
        top: Some("Top".into()),
        max_step: Some(5e-8),
        streams: keep.iter().map(|s| s.to_string()).collect(),
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings())?;
    let kept = Rc::new(RefCell::new(Kept::default()));
    session.run(kept.clone())?;
    let records = kept.borrow().0.clone();
    Ok(records)
}

/// Two modules emitting `s` (at 1 us) and `t` (at 2 us), `M` placed as `m` and `n`, `N` as `o`.
fn two_streams() -> String {
    design(
        "module M() {}\ndigital for M { on at(1u) { emit!(\"s\", a = 1.0); } }\nmodule N() {}\ndigital for N { on at(2u) { emit!(\"t\", a = 2.0); } }\n",
        "    let m = M();\n    let n = M();\n    let o = N();\n",
    )
}

/// The `<instance> <stream>` pairs of `records`, sorted.
fn named(records: &[Record]) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = records.iter().map(|r| (r.instance.clone(), r.stream.clone())).collect();
    all.sort();
    all
}

#[test]
fn keep_matches_a_stream_name_or_its_full_path() {
    // CLI-12 AC 1: an entry is a stream's name (every instance's stream of that name) or one
    // stream's `<instance path>.<stream>`; two instances of `M` each emit `s` at 1 us, `o` emits
    // `t` at 2 us
    let text = two_streams();
    let pair = |i: &str, s: &str| (i.to_string(), s.to_string());
    let all = kept_records(&text, &[]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&all), [pair("Top.m", "s"), pair("Top.n", "s"), pair("Top.o", "t")]);
    let by_name = kept_records(&text, &["s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&by_name), [pair("Top.m", "s"), pair("Top.n", "s")]);
    let by_path = kept_records(&text, &["Top.m.s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&by_path), [pair("Top.m", "s")]);
    let both = kept_records(&text, &["Top.m.s", "t"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&both), [pair("Top.m", "s"), pair("Top.o", "t")]);
    // the reserved stream is a stream like any other: `checks` of a design with a `require`
    // is kept by name and dropped by its absence
    let checked = design("module R() {}\ndigital for R { on at(1u) { require(\"ok\", true); emit!(\"s\", a = 1.0); } }\n", "    let r = R();\n");
    let only_s = kept_records(&checked, &["s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&only_s), [pair("Top.r", "s")]);
    let only_checks = kept_records(&checked, &["checks"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(named(&only_checks), [pair("Top.r", "checks")]);
}

#[test]
fn a_filter_entry_that_matches_nothing_is_e0801() {
    // CLI-12 AC 2: E0801 naming the entry, at open, before any analysis; a name with the right
    // instance and the wrong stream, and a stream of no design, both match nothing
    let text = two_streams();
    for entry in ["nothing", "Top.m.t", "Top.zzz.s"] {
        match kept_records(&text, &["s", entry]) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(entry), "{message}"),
            other => panic!("`{entry}`: expected E0801, found {:?}", other.map(|r| r.len())),
        }
    }
}

/// The table of a 5 us transient of `text` (max step 50 ns, `print_step` when given), or the failure.
fn table_of(text: &str, print_step: Option<f64>) -> Result<piperine_api::Table, Failure> {
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        analysis: Some("transient".into()),
        stop: Some(5e-6),
        top: Some("Top".into()),
        max_step: Some(5e-8),
        print_step,
        ..Overrides::default()
    };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings())?;
    session.run(Rc::new(RefCell::new(Kept::default())))
}

fn time_column(table: &piperine_api::Table) -> Vec<f64> {
    table
        .rows
        .iter()
        .map(|row| match &row[0] {
            piperine_api::Cell::Real(t) => *t,
            other => panic!("the time column holds a real, found {other:?}"),
        })
        .collect()
}

#[test]
fn a_run_ended_by_finish_gives_its_rows_to_the_instant() {
    // FIX-02 AC 1: `finish()` at 2 us in a run to 5 us gives rows up to 2 us and no row past it,
    // and no short-run failure, with and without a print step (0.5 us: rows 0, 0.5, .., 2.0 us).
    let m = "module M() {}\ndigital for M { on at(2u) { finish(); } }\n";
    let text = design(m, "    let m = M();\n");
    let every_step = table_of(&text, None).unwrap_or_else(|f| panic!("{f}"));
    let times = time_column(&every_step);
    let last = *times.last().expect("rows");
    assert!((last - 2e-6).abs() <= 1e-12, "the last row is at the finish instant, not 5 us: {last}");
    let gridded = table_of(&text, Some(0.5e-6)).unwrap_or_else(|f| panic!("{f}"));
    let times = time_column(&gridded);
    assert_eq!(times.len(), 5, "rows at 0, 0.5, 1.0, 1.5 and 2.0 us: {times:?}");
    assert!((times[4] - 2e-6).abs() <= 1e-12, "{times:?}");
}

#[test]
fn a_digital_only_run_without_finish_reaches_its_stop() {
    // FIX-02 AC 5 (CLI-05 AC 11) rested on a digital-only top stepping nothing; HII-17 (L56) closed
    // that: the same bench now runs to its stop with no `finish` to excuse it, and the record of the
    // instant at 2 us is delivered. A run that genuinely ends short is the recorder's `reached`
    // rule, held by `recorder::tests::the_recorder_reports_the_time_of_the_last_step_it_saw`
    let m = "module M() {}\ndigital for M { on at(2u) { emit!(\"s\", a = 1.0); } }\nmodule Top() { let m = M(); }\n";
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, m).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("transient".into()), stop: Some(5e-6), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &settings()).unwrap_or_else(|f| panic!("{f}"));
    let kept = Rc::new(RefCell::new(Kept::default()));
    let table = session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.last().expect("rows")[0], Cell::Real(5e-6));
    let records = kept.borrow().0.clone();
    assert_eq!(records.iter().filter(|r| r.stream == "s").count(), 1, "{records:?}");
}
