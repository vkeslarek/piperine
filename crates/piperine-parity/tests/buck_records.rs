//! Records from MSL source (feature `msl-digital`, T52; checks C104, C107, DIG-11 AC 4 and 5):
//! the stdlib `Never` on a clock with `bad` held false and then true, and `log` from a reaction
//! (`tests/buck/records.msl`). A `Sink` of this file decodes each record the solver delivers: a
//! commit to a `RECORD` pin arrives as a `Commit` observation (the instance and the statement)
//! followed by a `Record` observation (the stream and the field bytes), and
//! `Body::record_site` gives the stream and the text fields of the statement, which never cross
//! the ABI. Expected instants and values are written from std/monitor.msl, std/io.msl and
//! `docs/spec/part_vi_io.md` §19, not read back.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::Ir;
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
use support::{build, with_buck};

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

/// The commit to a record pin that waits for its `Record` observation.
struct Waiting {
    /// The record pin's path: the instance path and the stream.
    pin: String,
    instance: String,
    stream: String,
    texts: Vec<(String, String)>,
}

/// A sink that decodes the records of the transient with `Body::record_site`. Inside one
/// microstep the solver delivers every commit (by instance, then statement) and then every
/// record (by pin, then in the order made), so the `k`-th record of a pin belongs to the `k`-th
/// waiting commit whose site is on that pin.
struct Decoder {
    ir: Arc<Ir>,
    pending: Vec<Waiting>,
    out: Rc<RefCell<Vec<Decoded>>>,
}

impl Sink for Decoder {
    fn observe(&mut self, o: &Observation) {
        if o.analysis != AnalysisKind::Transient {
            return;
        }
        match &o.kind {
            ObservationKind::Commit { statement, .. } => {
                let Some(instance) = self.ir.instance_at(&o.path) else { return };
                // every body of this file has one reaction, so the statement alone is the site
                if let Some(site) = instance.body().record_site(0, *statement) {
                    self.pending.push(Waiting {
                        pin: format!("{}.{}", o.path, site.stream),
                        instance: o.path.clone(),
                        stream: site.stream.to_string(),
                        texts: site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
                    });
                }
            }
            ObservationKind::Record { bytes } => {
                let at = self.pending.iter().position(|w| w.pin == o.path).expect("a record follows the commit that made it");
                let w = self.pending.remove(at);
                self.out.borrow_mut().push(Decoded {
                    instance: w.instance,
                    made: (o.time * 1e15).round() as i64,
                    stream: w.stream,
                    texts: w.texts,
                    bytes: bytes.clone(),
                });
            }
            _ => {}
        }
    }
}

/// Runs a transient to `stop` and returns every record decoded.
fn records(built: &Built, stop: f64) -> Vec<Decoded> {
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let out = Rc::new(RefCell::new(Vec::new()));
    let mut options = TransientAnalysisOptions::new(stop, 1e-9);
    options.dt_max = 1e-9;
    let sink = Decoder { ir: built.ir.clone(), pending: Vec::new(), out: out.clone() };
    let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(sink)).build();
    solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
    let all = out.borrow().clone();
    all
}

#[test]
fn never_reports_each_rising_edge() {
    // 30 ns: rising edges 1 to 6 at 2.5, 7.5, ..., 27.5 ns
    let all = records(&build(&with_buck("records.msl"), "NeverBench"), 30e-9);
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
    let all = records(&build(&with_buck("records.msl"), "LogBench"), 12e-9);
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
