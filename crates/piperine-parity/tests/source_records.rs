//! Records from MSL source (feature `msl-digital`, T52 and T67; checks C104, C107, C180, C181, DIG-11
//! AC 1, 4 and 5): the stdlib `Never` on a clock with `bad` held false and then true, `log` from
//! a reaction, and `log` from two reactions of one body (`tests/digital/records.msl`). A `Sink` of
//! this file decodes each record the solver delivers: a commit to a `RECORD` pin arrives as a
//! `Record` observation carrying its pin path, the reaction and the statement that made it
//! (abi.md C5) and the field bytes, and `Ir::record_at` gives the instance and the stream and
//! text fields of that site, which never cross the ABI. T67's fix round (F1): three `log`s over
//! two reactions at different statement indices keep their own texts. Expected instants and values are written from std/monitor.msl, std/io.msl and
//! `docs/spec/part_vi_io.md` §19, not read back.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::Ir;
use piperine_parity::project::Project;
use piperine_solver::prelude::{Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, rise};
use support::build;

/// The fixture of this target: `tests/digital/records.msl` as a project of its own.
struct Source;

impl Source {
    /// The project of the fixture file.
    fn project() -> Project {
        Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/digital/records.msl"))
    }
}

/// One record, decoded.
#[derive(Debug, Clone, PartialEq)]
struct Decoded {
    /// The instance that made it and the instant (ticks) it was made.
    instance: String,
    made: i64,
    stream: String,
    texts: Vec<(String, String)>,
    bytes: Vec<u8>,
}

/// A sink that decodes the records of the transient with `Ir::record_at`: each `Record`
/// observation carries the reaction and the statement that made it (abi.md C5), so it is decoded
/// on its own, whatever order the records and commits of one microstep arrive in.
struct Decoder {
    ir: Arc<Ir>,
    out: Rc<RefCell<Vec<Decoded>>>,
}

impl Sink for Decoder {
    fn observe(&mut self, o: &Observation) {
        if o.analysis != AnalysisKind::Transient {
            return;
        }
        let ObservationKind::Record { site, bytes } = &o.kind else { return };
        let (reaction, statement) = site.expect("a record of an emit names its site");
        let (instance, site) = self.ir.record_at(&o.path, reaction, statement).expect("a record decodes to its emit");
        self.out.borrow_mut().push(Decoded {
            instance: instance.path().to_string(),
            made: (o.time * 1e15).round() as i64,
            stream: site.stream.to_string(),
            texts: site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            bytes: bytes.clone(),
        });
    }
}

/// Runs a transient to `stop` and returns every record decoded.
fn records(built: &Built, stop: f64) -> Vec<Decoded> {
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let out = Rc::new(RefCell::new(Vec::new()));
    let mut options = TransientAnalysisOptions::new(stop, 1e-9);
    options.dt_max = 1e-9;
    let sink = Decoder { ir: built.ir.clone(), out: out.clone() };
    let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(sink)).build();
    solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
    let all = out.borrow().clone();
    all
}

#[test]
fn never_reports_each_rising_edge() {
    // 30 ns: rising edges 1 to 6 at 2.5, 7.5, ..., 27.5 ns
    let all = records(&build(&Source::project(), "NeverBench"), 30e-9);
    // std/monitor.msl: `Never` runs `require("never", !bad)` at each rising edge: one record per
    // edge, on `checks`, with the texts `kind = require` and `name = never`
    assert_eq!(all.len(), 6, "one record per rising edge: {all:?}");
    for (k, r) in all.iter().enumerate() {
        assert_eq!(r.instance, "NeverBench.never");
        assert_eq!(r.stream, "checks");
        assert_eq!(r.texts, vec![("kind".to_string(), "require".to_string()), ("name".to_string(), "never".to_string())]);
        assert_eq!(r.made, rise(k as i64 + 1), "made at rising edge {}", k + 1);
        // `pass` is the one value field, a known `bool`: `{ v @0, known @1 }`
        assert_eq!(r.bytes[1], 1, "pass is known at edge {}: {:?}", k + 1, r.bytes);
    }
    // `Bad` drives `bad` true at the third rising edge, so `Never` reads it false at edges 1 to 3
    // (a pass) and true at edges 4 to 6 (a failure): each read is of the value before the edge
    let pass: Vec<u8> = all.iter().map(|r| r.bytes[0]).collect();
    assert_eq!(pass, vec![1, 1, 1, 0, 0, 0]);
}

#[test]
fn log_carries_its_severity_and_its_text() {
    // 12 ns: rising edges 1 and 2 at 2.5 and 7.5 ns
    let all = records(&build(&Source::project(), "LogBench"), 12e-9);
    // part VI §19 and std/io.msl: `log(Severity::Warning, "x")` is one record of the stream `log`
    // with `severity` 1 and the site text `x`; the second statement is `Severity::Error` (2),
    // text `y`: one record per statement per run, in statement order
    assert_eq!(all.len(), 4, "two statements at two edges: {all:?}");
    for (k, r) in all.iter().enumerate() {
        let (severity, text) = if k % 2 == 0 { (1, "x") } else { (2, "y") };
        assert_eq!(r.stream, "log");
        assert_eq!(r.texts, vec![("msg".to_string(), text.to_string())]);
        assert_eq!(r.bytes[0], severity, "severity of record {k}: {:?}", r.bytes);
        assert_eq!(r.bytes[1], 1, "severity is known: {:?}", r.bytes);
        assert_eq!(r.made, rise(k as i64 / 2 + 1));
    }
}

#[test]
fn records_of_two_reactions_are_told_apart() {
    // 12 ns: rising edges 1 and 2 at 2.5 and 7.5 ns. `TwoTalk` has two reactions on the same
    // edge, each a `log` at its statement 0: `Warning` (1) with the text `first`, then `Error`
    // (2) with `second` (part VI §19, std/io.msl). The site of a record is the reaction and the
    // statement of its commit (abi.md C5): keyed on the statement alone, both would read
    // `first`. Reactions of one instant run in source order (§12.1.2), so each edge gives the
    // first reaction's record, then the second's.
    let all = records(&build(&Source::project(), "TwoBench"), 12e-9);
    assert_eq!(all.len(), 4, "two reactions at two edges: {all:?}");
    for (k, r) in all.iter().enumerate() {
        let (severity, text) = if k % 2 == 0 { (1, "first") } else { (2, "second") };
        assert_eq!(r.instance, "TwoBench.talk");
        assert_eq!(r.stream, "log");
        assert_eq!(r.texts, vec![("msg".to_string(), text.to_string())], "record {k}");
        assert_eq!((r.bytes[0], r.bytes[1]), (severity, 1), "severity of record {k}: {:?}", r.bytes);
        assert_eq!(r.made, rise(k as i64 / 2 + 1));
    }
}

/// `(severity, text)` of every record, in delivery order.
fn logged(all: &[Decoded]) -> Vec<(i64, u8, String)> {
    all.iter()
        .map(|r| {
            assert_eq!(r.stream, "log");
            assert_eq!(r.bytes[1], 1, "severity is known: {r:?}");
            let [(field, text)] = r.texts.as_slice() else { panic!("one text field: {r:?}") };
            assert_eq!(field, "msg");
            (r.made, r.bytes[0], text.clone())
        })
        .collect()
}

#[test]
fn records_of_two_reactions_at_different_statements_keep_their_texts() {
    // 12 ns: rising edges 1 and 2. Reactions of one instant run in source order (§12.1.2), and
    // `Severity` is `Info` (0), `Warning` (1), `Error` (2) (part VI §19, std/io.msl).
    // `ThreeTalk`'s first reaction logs `Info` `a` at its statement 0 and `Warning` `b` at its
    // statement 1; its second logs `Error` `c` at its statement 0: each edge gives `a`, `b`, `c`.
    // `AfterTalk` is the other way round: `Info` `p` at statement 0 of the first reaction, then
    // `Warning` `q` and `Error` `r` at statements 0 and 1 of the second: each edge gives `p`,
    // `q`, `r`. Each record keeps its own severity and its own text
    for (bench, texts) in [("ThreeBench", ["a", "b", "c"]), ("AfterBench", ["p", "q", "r"])] {
        let all = records(&build(&Source::project(), bench), 12e-9);
        assert!(all.iter().all(|r| r.instance == format!("{bench}.talk")), "{all:?}");
        let mut want = Vec::new();
        for edge in 1..=2 {
            for (severity, text) in texts.iter().enumerate() {
                want.push((rise(edge), severity as u8, text.to_string()));
            }
        }
        assert_eq!(logged(&all), want, "{bench}");
    }
}
