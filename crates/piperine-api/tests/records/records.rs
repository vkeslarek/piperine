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
use std::sync::Arc;

use piperine_api::{Cell, Field, Failure, Overrides, Project, Record, Session, Streams, Verdict};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind};
use piperine_parity::compile_cache::CompileCache;
use piperine_parity::sinks::Kept;

/// A source across a resistor: the analog part every fixture's top holds.
const ANALOG: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// The designs of the records tests: modules and their placements, and the two-stream design.
struct RecordSource;

impl RecordSource {
    /// `module Top() { <a source and a resistor>; <placements> }` over the modules `modules`.
    fn design(modules: &str, placements: &str) -> String {
        format!("{ANALOG}\n{modules}\nmodule Top() {{\n    signal a: Electrical;\n    let v1 = Vsource(a, gnd) {{ dc: 1.0 }};\n    let r1 = Resistor(a, gnd) {{ r: 1k }};\n{placements}}}\n")
    }

    /// Two modules emitting `s` (at 1 us) and `t` (at 2 us), `M` placed as `m` and `n`, `N` as `o`.
    fn two_streams() -> String {
        RecordSource::design(
            "module M() {}\ndigital for M { on at(1u) { emit!(\"s\", a = 1.0); } }\nmodule N() {}\ndigital for N { on at(2u) { emit!(\"t\", a = 2.0); } }\n",
            "    let m = M();\n    let n = M();\n    let o = N();\n",
        )
    }
}

/// A 5 us transient of a design, and the records or the table it kept.
struct RecordRun;

impl RecordRun {
    /// The records of `analysis` (to `stop` for a transient) of the lone file `path`, top `Top`.
    fn records_of(path: &Path, top: &str, analysis: &str, stop: Option<f64>) -> Result<Vec<Record>, Failure> {
        let project = Project::file(path).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some(analysis.into()), stop, top: Some(top.into()), max_step: stop.map(|s| s / 100.0), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings())?;
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
        RecordRun::records_of(&file, "Top", "transient", Some(5e-6)).unwrap_or_else(|f| panic!("{f}"))
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
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings())?;
        let kept = Rc::new(RefCell::new(Kept::default()));
        session.run(kept.clone())?;
        let records = kept.borrow().0.clone();
        Ok(records)
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
        let mut session = Session::open(&project, &spec, &CompileCache::shared().settings())?;
        session.run(Rc::new(RefCell::new(Kept::default())))
    }
}

/// What the records and the table of a run are read as: stream names and the time column.
struct RecordReading;

impl RecordReading {
    /// The `<instance> <stream>` pairs of `records`, sorted.
    fn named(records: &[Record]) -> Vec<(String, String)> {
        let mut all: Vec<(String, String)> = records.iter().map(|r| (r.instance.clone(), r.stream.clone())).collect();
        all.sort();
        all
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
}

#[test]
fn an_emit_gives_one_record_with_time_instance_stream_and_fields() {
    // CLI-06 AC 1: one timer at 1 us, one emit with `a` then `b`
    let text = RecordSource::design("module M() {}\ndigital for M { on at(1u) { emit!(\"s\", a = 1.5, b = true); } }\n", "    let m = M();\n");
    let all = RecordRun::records(&text);
    assert_eq!(all.len(), 1, "{all:?}");
    let r = &all[0];
    assert!((r.time - 1e-6).abs() <= 1e-15, "the timer's instant, to the solver's tick: {}", r.time);
    assert_eq!((r.instance.as_str(), r.stream.as_str()), ("Top.m", "s"));
    assert_eq!(r.fields, [("a".to_string(), Field::Real(1.5)), ("b".to_string(), Field::Bool(true))]);

    // task T14: bytes of another length than the stream's layout are a failure, never a
    // truncated record. `s` is `a: f64` (8 bytes) then `b: bool` (1 byte at 8), rounded up to the
    // alignment 8: 16 bytes in all; the record above is decoded from such bytes, one short is not
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, &text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let session = {
                Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"))
    };
    let streams = Streams::of(Arc::new(session.ir().clone()));
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&1.5f64.to_le_bytes());
    bytes[8] = 1;
    let observation = |bytes: Vec<u8>| Observation {
        path: "Top.m.s".into(),
        analysis: AnalysisKind::Transient,
        time: 1e-6,
        kind: ObservationKind::Record { site: Some((0, 0)), bytes },
    };
    let whole = streams.decode(&observation(bytes.clone())).expect("a record").unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(whole.fields, r.fields, "16 bytes decode as the run's record");
    bytes.pop();
    let short = streams.decode(&observation(bytes)).expect("a record observation");
    assert!(matches!(&short, Err(Failure::Backend(m)) if m.contains("15 bytes")), "{short:?}");
}

#[test]
fn a_record_holds_its_declared_fields_only() {
    // EU-15 AC 1 (§19): a stream's record layout is its declared fields, laid out at the next
    // offset their alignment allows, and nothing after them. `s` is `ok: bool` alone: a `bool`
    // carrier is its one value byte, so the record is 1 byte; one byte decodes as the record, and
    // the two bytes the layout once held (a value, then a trailing `maybe`) are a failure, never
    // a truncated or a padded record
    let text = RecordSource::design("module M() {}\ndigital for M { on at(1u) { emit!(\"s\", ok = true); } }\n", "    let m = M();\n");
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, &text).expect("write");
    let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides { analysis: Some("op".into()), top: Some("Top".into()), ..Overrides::default() };
    let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
    let session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
    let streams = Streams::of(Arc::new(session.ir().clone()));
    let decode = |bytes: Vec<u8>| {
        let observation = Observation { path: "Top.m.s".into(), analysis: AnalysisKind::Transient, time: 1e-6, kind: ObservationKind::Record { site: Some((0, 0)), bytes } };
        streams.decode(&observation).expect("a record observation")
    };
    let record = decode(vec![1]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(record.fields, [("ok".to_string(), Field::Bool(true))], "the declared field and no other");
    let long = decode(vec![1, 0]);
    assert!(matches!(&long, Err(Failure::Backend(m)) if m.contains("2 bytes where the stream's layout holds 1")), "{long:?}");
}

#[test]
fn a_require_is_a_pass_or_a_fail() {
    // EU-15 AC 2 (std/io.msl `require`): a `require` that holds is a record with `pass = true`
    // and the run is a pass; one that does not is `pass = false` and the verdict fails it. No
    // record is a third thing: `pass` is a `bool`
    let m = "module M() { ok: bool = true; }\ndigital for M { on at(1u) { require(\"held\", ok); } }\n";
    let held = RecordRun::records(&RecordSource::design(m, "    let m = M();\n"));
    assert_eq!(held.iter().map(|r| r.fields[2].1.clone()).collect::<Vec<_>>(), [Field::Bool(true)], "{held:?}");
    assert!(Verdict::judge(&held).passed());
    let failed = RecordRun::records(&RecordSource::design(m, "    let m = M() { ok: false };\n"));
    assert_eq!(failed.iter().map(|r| r.fields[2].1.clone()).collect::<Vec<_>>(), [Field::Bool(false)], "{failed:?}");
    let verdict = Verdict::judge(&failed);
    assert!(!verdict.passed());
    assert_eq!((verdict.checks(), verdict.failed().len()), (1, 1));
}

#[test]
fn a_fatal_record_ends_the_run() {
    // EU-15 AC 3 (std/io.msl `fatal`): `emit!("sim", finish = true, failed = true, msg = msg)`;
    // a `sim` record that says `failed` is a failed verdict carrying the message, and the run
    // ends at its instant: the rows stop at 2 us in a run to 5 us
    let m = "module M() {}\ndigital for M { on at(2u) { fatal(\"boom\"); } }\n";
    let text = RecordSource::design(m, "    let m = M();\n");
    let all = RecordRun::records(&text);
    let verdict = Verdict::judge(&all);
    assert!(!verdict.passed(), "{all:?}");
    let fatal = verdict.fatal().expect("the fatal record");
    assert_eq!(fatal.fields.last(), Some(&("msg".to_string(), Field::Text("boom".into()))));
    let times = RecordReading::time_column(&RecordRun::table_of(&text, None).unwrap_or_else(|f| panic!("{f}")));
    let last = *times.last().expect("rows");
    assert!((last - 2e-6).abs() <= 1e-12, "the last row is at the fatal instant, not 5 us: {last}");
}

#[test]
fn records_arrive_in_time_order() {
    // CLI-06 AC 3: timers written at 3 us, 1 us, 2 us arrive at 1, 2, 3 us
    let m = "module M() {}\ndigital for M {\n    on at(3u) { emit!(\"s\", k = 3); }\n    on at(1u) { emit!(\"s\", k = 1); }\n    on at(2u) { emit!(\"s\", k = 2); }\n}\n";
    let all = RecordRun::records(&RecordSource::design(m, "    let m = M();\n"));
    let seen: Vec<(i64, Field)> = all.iter().map(|r| ((r.time * 1e9).round() as i64, r.fields[0].1.clone())).collect();
    assert_eq!(seen, [(1000, Field::Int(1)), (2000, Field::Int(2)), (3000, Field::Int(3))]);
}

#[test]
fn a_text_field_is_the_sources_literal() {
    // CLI-06 AC 4: a text never crosses the ABI; it is the site's literal
    let m = "module M() {}\ndigital for M { on at(1u) { emit!(\"log\", msg = \"overrange\"); } }\n";
    let all = RecordRun::records(&RecordSource::design(m, "    let m = M();\n"));
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].fields, [("msg".to_string(), Field::Text("overrange".into()))]);
}

#[test]
fn a_failed_require_is_one_checks_record_with_pass_false() {
    // CLI-06 AC 6; std/io.msl: `require(name, pass)` is `emit!("checks", kind = "require",
    // name = name, pass = pass)`
    let m = "module M() {}\ndigital for M { on at(1u) { require(\"lim\", false); } }\n";
    let all = RecordRun::records(&RecordSource::design(m, "    let m = M();\n"));
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
    let all = RecordRun::records_of(&divider, "Divider", "op", None).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(all, []);
}

#[test]
fn a_reserved_stream_keeps_its_name_and_its_site() {
    // spec edge 7: `checks` is reserved (docs/spec/part_vi_io.md); its record keeps the stream
    // name and the instance path of the reaction that wrote it, here the second of two monitors
    let m = "module M() { ok: bool = true; }\ndigital for M { on at(1u) { require(\"held\", ok); } }\n";
    let all = RecordRun::records(&RecordSource::design(m, "    let first = M();\n    let second = M() { ok: false };\n"));
    let mut seen: Vec<(&str, &str, &Field)> = all.iter().map(|r| (r.instance.as_str(), r.stream.as_str(), &r.fields[2].1)).collect();
    seen.sort_by_key(|(i, _, _)| *i);
    assert_eq!(seen, [("Top.first", "checks", &Field::Bool(true)), ("Top.second", "checks", &Field::Bool(false))]);
}

#[test]
fn keep_matches_a_stream_name_or_its_full_path() {
    // CLI-12 AC 1: an entry is a stream's name (every instance's stream of that name) or one
    // stream's `<instance path>.<stream>`; two instances of `M` each emit `s` at 1 us, `o` emits
    // `t` at 2 us
    let text = RecordSource::two_streams();
    let pair = |i: &str, s: &str| (i.to_string(), s.to_string());
    let all = RecordRun::kept_records(&text, &[]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&all), [pair("Top.m", "s"), pair("Top.n", "s"), pair("Top.o", "t")]);
    let by_name = RecordRun::kept_records(&text, &["s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&by_name), [pair("Top.m", "s"), pair("Top.n", "s")]);
    let by_path = RecordRun::kept_records(&text, &["Top.m.s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&by_path), [pair("Top.m", "s")]);
    let both = RecordRun::kept_records(&text, &["Top.m.s", "t"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&both), [pair("Top.m", "s"), pair("Top.o", "t")]);
    // the reserved stream is a stream like any other: `checks` of a design with a `require`
    // is kept by name and dropped by its absence
    let checked = RecordSource::design("module R() {}\ndigital for R { on at(1u) { require(\"ok\", true); emit!(\"s\", a = 1.0); } }\n", "    let r = R();\n");
    let only_s = RecordRun::kept_records(&checked, &["s"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&only_s), [pair("Top.r", "s")]);
    let only_checks = RecordRun::kept_records(&checked, &["checks"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(RecordReading::named(&only_checks), [pair("Top.r", "checks")]);
}

#[test]
fn a_filter_entry_that_matches_nothing_is_e0801() {
    // CLI-12 AC 2: E0801 naming the entry, at open, before any analysis; a name with the right
    // instance and the wrong stream, and a stream of no design, both match nothing
    let text = RecordSource::two_streams();
    for entry in ["nothing", "Top.m.t", "Top.zzz.s"] {
        match RecordRun::kept_records(&text, &["s", entry]) {
            Err(Failure::Host { code: "E0801", message }) => assert!(message.contains(entry), "{message}"),
            other => panic!("`{entry}`: expected E0801, found {:?}", other.map(|r| r.len())),
        }
    }
}

#[test]
fn a_run_ended_by_finish_gives_its_rows_to_the_instant() {
    // FIX-02 AC 1: `finish()` at 2 us in a run to 5 us gives rows up to 2 us and no row past it,
    // and no short-run failure, with and without a print step (0.5 us: rows 0, 0.5, .., 2.0 us).
    let m = "module M() {}\ndigital for M { on at(2u) { finish(); } }\n";
    let text = RecordSource::design(m, "    let m = M();\n");
    let every_step = RecordRun::table_of(&text, None).unwrap_or_else(|f| panic!("{f}"));
    let times = RecordReading::time_column(&every_step);
    let last = *times.last().expect("rows");
    assert!((last - 2e-6).abs() <= 1e-12, "the last row is at the finish instant, not 5 us: {last}");
    let gridded = RecordRun::table_of(&text, Some(0.5e-6)).unwrap_or_else(|f| panic!("{f}"));
    let times = RecordReading::time_column(&gridded);
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
    let mut session = Session::open(&project, &spec, &CompileCache::shared().settings()).unwrap_or_else(|f| panic!("{f}"));
    let kept = Rc::new(RefCell::new(Kept::default()));
    let table = session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.rows.last().expect("rows")[0], Cell::Real(5e-6));
    let records = kept.borrow().0.clone();
    assert_eq!(records.iter().filter(|r| r.stream == "s").count(), 1, "{records:?}");
}

#[test]
fn a_stream_named_like_a_signal_of_the_same_record_size_keeps_the_signals_changes() {
    // a record of `v: u8` and `some: bool` is 2 bytes, the size of an `Option<u8>` (a presence byte and the value): the stream `q`
    // and the signal `Top.q` share the path, and the write of the stream's pin is no change of the signal (part VI §19). The
    // run holds `q` at `Some(42)` from 1 us on, and the stream's two records show the value before and after
    let text = "module Top() { signal q: Bit<Option<u8>>; }\n\
                digital for Top { on at(1u) { q <- Some(42); } on q.changed() { emit!(\"q\", v = q.unwrap_or(7u8), some = q.is_some()); } }\n";
    let table = RecordRun::table_of(text, None).unwrap_or_else(|f| panic!("{f}"));
    let column = table.columns.iter().position(|name| name == "Top.q").expect("the signal is a column");
    let last = table.rows.last().expect("rows")[column].clone();
    let held = Some(42u8);
    assert_eq!(format!("{last:?}"), format!("Composite(Option(Some(Scalar(Int({})))))", held.expect("a value")), "the signal ends at what it was committed");
    let dir = tempfile::tempdir().expect("tmp");
    let file = dir.path().join("bench.msl");
    std::fs::write(&file, text).expect("write");
    let seen = RecordRun::records_of(&file, "Top", "transient", Some(5e-6)).unwrap_or_else(|f| panic!("{f}"));
    let values: Vec<(&Field, &Field)> = seen.iter().map(|r| (&r.fields[0].1, &r.fields[1].1)).collect();
    assert_eq!(values, [(&Field::Int(7), &Field::Bool(false)), (&Field::Int(42), &Field::Bool(true))], "None at the start, then Some(42)");
}
