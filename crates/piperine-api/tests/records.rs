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

use piperine_api::{Field, Failure, Overrides, Project, Record, RecordSink, Session, Streams};
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
fn an_unknown_field_is_unknown() {
    // CLI-06 AC 5: `d` is an in-port the top never drives, so its value is unknown
    let m = "module M(in d: Bit) {}\ndigital for M { on at(1u) { emit!(\"s\", v = d); } }\n";
    let all = records(&design(m, "    signal d: Bit;\n    let m = M(d);\n"));
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].fields, [("v".to_string(), Field::Unknown)]);
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
    let divider: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice/divider.msl");
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
