//! Scope: traces as records of the host library (analog-operators AO-09, D15, Q5): an `emit!` in
//! `analog` is delivered as a `Record` of its stream with the Newton iteration and the analysis,
//! and only when the run names the stream in `streams`. Each fixture is MSL written by the test
//! and run on the Rust backend.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Field, Overrides, Project, Record, RecordSink, Session};
use piperine_solver::protocol::AnalysisKind;
use piperine_transpile::Settings;

/// Keeps every record, in the order it is handed over.
#[derive(Default)]
struct Kept(Vec<Record>);

impl RecordSink for Kept {
    fn record(&mut self, record: &Record) {
        self.0.push(record.clone());
    }
}

/// A node held at 0.3 V by a source and traced at every evaluation, to the streams `conv` and `aux`.
const BENCH: &str = "module Top() { signal a: Electrical; }\nanalog for Top { v(a, gnd) +> 0.3; emit!(\"conv\", v = v(a, gnd), who = \"top\"); emit!(\"aux\", v = v(a, gnd)); }\n";

/// The bench `BENCH` and its runs.
struct Bench;

impl Bench {
    /// The records of an operating point of `BENCH` that names `streams`.
    fn records(streams: &[&str]) -> Vec<Record> {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = tempfile::tempdir().expect("tmp");
        let file = dir.path().join("bench.msl");
        std::fs::write(&file, BENCH).expect("write");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { top: Some("Top".into()), analysis: Some("op".into()), print: vec!["a".into()], streams: streams.iter().map(|s| s.to_string()).collect(), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        let kept = Rc::new(RefCell::new(Kept::default()));
        session.run(kept.clone()).unwrap_or_else(|f| panic!("{f}"));
        let records = kept.borrow().0.clone();
        records
    }
}

#[test]
fn traces_are_recorded_only_for_a_named_stream() {
    // AO-09 AC 6 (Q5): the stream `conv` named, its records arrive; none named, none arrive; only `aux`
    // named, `aux`'s records arrive and no `conv` record does.
    let named = Bench::records(&["conv"]);
    assert!(!named.is_empty());
    for record in &named {
        assert_eq!((record.instance.as_str(), record.stream.as_str()), ("Top", "conv"));
        assert_eq!(record.analysis, Some(AnalysisKind::OperatingPoint));
        assert!(record.iteration.is_some(), "a trace carries its Newton iteration");
        // the value field, then the site's text field
        assert!(matches!(record.fields.as_slice(), [(v, Field::Real(_)), (who, Field::Text(text))] if v == "v" && who == "who" && text == "top"), "{:?}", record.fields);
    }
    // the last evaluation is at the operating point, 0.3 V
    let last = named.last().expect("a record");
    assert!(matches!(last.fields[0].1, Field::Real(x) if (x - 0.3).abs() <= 1e-12), "{:?}", last.fields);
    assert!(Bench::records(&[]).is_empty(), "no stream named, no trace");
    let other = Bench::records(&["aux"]);
    assert!(!other.is_empty() && other.iter().all(|record| record.stream == "aux"), "only `aux` named: no `conv` record arrives, {other:?}");
}
