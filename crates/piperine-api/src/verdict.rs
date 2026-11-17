//! `Verdict`: pass or fail of one run under `piperine test` (CLI-16). The records of the
//! reserved `checks` stream are the checks; a check whose `pass` is false, or unknown, fails
//! the run. Every check record counts, a passing one too: what a terminal hides never reaches
//! this type's input as a filter.

use crate::record::{Field, Record, RecordSink};

/// The checks of a run and the ones that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    checks: usize,
    failed: Vec<Record>,
}

impl Verdict {
    /// An empty verdict: no check seen, none failed. Hand it to a run as a `RecordSink`.
    pub fn new() -> Verdict {
        Verdict { checks: 0, failed: Vec::new() }
    }

    /// Judges `records`, the whole record stream of a run, through the sink: counts those of
    /// the `checks` stream and keeps those whose `pass` is false or unknown. A `checks` record
    /// with no `pass` field (a `target`, which reports a margin) counts and does not fail.
    pub fn judge(records: &[Record]) -> Verdict {
        let mut verdict = Verdict::new();
        for record in records {
            verdict.record(record);
        }
        verdict
    }

    /// Whether no check failed.
    pub fn passed(&self) -> bool {
        self.failed.is_empty()
    }

    /// How many checks the run wrote, passing ones included.
    pub fn checks(&self) -> usize {
        self.checks
    }

    /// The checks that failed, in the order they were written.
    pub fn failed(&self) -> &[Record] {
        &self.failed
    }
}

impl Default for Verdict {
    fn default() -> Self {
        Verdict::new()
    }
}

impl RecordSink for Verdict {
    /// A record of the `checks` stream is counted, and kept when its `pass` is not true; every
    /// other record is dropped, so a run's memory holds the failures only.
    fn record(&mut self, record: &Record) {
        if record.stream != "checks" {
            return;
        }
        self.checks += 1;
        if record.fields.iter().any(|(name, field)| name == "pass" && *field != Field::Bool(true)) {
            self.failed.push(record.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(name: &str, pass: Field) -> Record {
        Record {
            time: 1e-6,
            instance: "Top.mon".into(),
            stream: "checks".into(),
            fields: vec![("kind".into(), Field::Text("require".into())), ("name".into(), Field::Text(name.into())), ("pass".into(), pass)],
        }
    }

    fn other(stream: &str) -> Record {
        Record { time: 0.0, instance: "Top.mon".into(), stream: stream.into(), fields: vec![("pass".into(), Field::Bool(false))] }
    }

    #[test]
    fn no_false_and_no_unknown_pass_is_a_pass() {
        // CLI-16 AC 1: every check record counts, the passing ones included; a record of
        // another stream is no check, even when it has a `pass` field that is false
        let records = [check("a", Field::Bool(true)), other("trip"), check("b", Field::Bool(true)), other("sim")];
        let verdict = Verdict::judge(&records);
        assert!(verdict.passed());
        assert_eq!(verdict.checks(), 2);
        assert!(verdict.failed().is_empty());
        // a run with no check at all passes with zero
        let none = Verdict::judge(&[other("trip")]);
        assert!(none.passed() && none.checks() == 0);
        // a `target` has no `pass`: it counts and does not fail
        let target = Record { fields: vec![("kind".into(), Field::Text("target".into())), ("margin".into(), Field::Real(0.1))], ..check("m", Field::Unknown) };
        let verdict = Verdict::judge(&[target]);
        assert!(verdict.passed() && verdict.checks() == 1);
        // a false `pass` fails, and the failing record is kept as written, in order
        let (first, second) = (check("c", Field::Bool(false)), check("d", Field::Bool(false)));
        let verdict = Verdict::judge(&[check("a", Field::Bool(true)), first.clone(), check("b", Field::Bool(true)), second.clone()]);
        assert!(!verdict.passed());
        assert_eq!(verdict.checks(), 4);
        assert_eq!(verdict.failed(), [first, second]);
    }

    #[test]
    fn an_unknown_pass_fails() {
        // CLI-16 AC 3: an unknown `pass` is reported as unknown, never as a pass
        let unknown = check("u", Field::Unknown);
        let verdict = Verdict::judge(&[check("a", Field::Bool(true)), unknown.clone()]);
        assert!(!verdict.passed());
        assert_eq!(verdict.checks(), 2);
        assert_eq!(verdict.failed(), [unknown]);
    }

    #[test]
    fn a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive() {
        // LR-16: the sink counts every check, keeps the failed (false or unknown) in order
        let mut verdict = Verdict::new();
        let (bad, unknown) = (check("b", Field::Bool(false)), check("u", Field::Unknown));
        verdict.record(&check("a", Field::Bool(true)));
        assert_eq!((verdict.checks(), verdict.failed().len()), (1, 0));
        verdict.record(&bad);
        verdict.record(&check("c", Field::Bool(true)));
        verdict.record(&unknown);
        assert_eq!(verdict.checks(), 4);
        assert!(!verdict.passed());
        assert_eq!(verdict.failed(), [bad, unknown]);
    }

    #[test]
    fn a_record_of_another_stream_never_decides() {
        // LR-16: a record of another stream is neither counted nor kept, whatever its `pass`
        let mut verdict = Verdict::new();
        verdict.record(&other("trip"));
        verdict.record(&other("sim"));
        assert!(verdict.passed());
        assert_eq!(verdict.checks(), 0);
        assert!(verdict.failed().is_empty());
    }
}
