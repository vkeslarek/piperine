//! `Verdict`: pass or fail of one run under `piperine test` (CLI-16). The records of the
//! reserved `checks` stream are the checks; a check whose `pass` is false, or unknown, fails
//! the run. Every check record counts, a passing one too: what a terminal hides never reaches
//! this type's input as a filter. A `sim` record with `failed = true` (what `fatal` writes) fails
//! the run with its `msg` (HII-15); the last margin of each `target` is kept and never fails it
//! (HII-16, the user's decision X3: a target informs, `require` is the constraint).

use crate::record::{Field, Record, RecordSink};

/// The checks of a run and the ones that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    checks: usize,
    failed: Vec<Record>,
    /// The first `sim` record whose `failed` is known true: a `fatal` the model called.
    fatal: Option<Record>,
    /// The last margin of each target name, in the order of its first record.
    targets: Vec<(String, f64)>,
}

impl Verdict {
    /// An empty verdict: no check seen, none failed. Hand it to a run as a `RecordSink`.
    pub fn new() -> Verdict {
        Verdict { checks: 0, failed: Vec::new(), fatal: None, targets: Vec::new() }
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

    /// Whether no check failed and no `fatal` was called.
    pub fn passed(&self) -> bool {
        self.failed.is_empty() && self.fatal.is_none()
    }

    /// The first `sim` record whose `failed` is known true (a `fatal(msg)` of the model), with
    /// its `msg` among its fields.
    pub fn fatal(&self) -> Option<&Record> {
        self.fatal.as_ref()
    }

    /// The margin of each target, the last record of each name, in the order of the first record
    /// of each name. A margin that is unknown is NaN, so it shows and never reads as met.
    pub fn targets(&self) -> &[(String, f64)] {
        &self.targets
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
    /// A record of the `checks` stream is counted, and kept when its `pass` is not true (a
    /// `target` keeps its margin instead); the first `sim` record whose `failed` is known true is
    /// kept as the fatal one; every other record is dropped, so a run's memory holds the failures
    /// only.
    fn record(&mut self, record: &Record) {
        if record.stream == "sim" && self.fatal.is_none() && record.fields.iter().any(|(name, field)| name == "failed" && *field == Field::Bool(true)) {
            self.fatal = Some(record.clone());
        }
        if record.stream != "checks" {
            return;
        }
        self.checks += 1;
        let field = |name: &str| record.fields.iter().find(|(n, _)| n == name).map(|(_, f)| f);
        if field("kind") == Some(&Field::Text("target".into())) {
            let Some(Field::Text(name)) = field("name") else { return };
            let margin = match field("margin") {
                Some(Field::Real(margin)) => *margin,
                _ => f64::NAN,
            };
            match self.targets.iter_mut().find(|(n, _)| n == name) {
                Some(kept) => kept.1 = margin,
                None => self.targets.push((name.clone(), margin)),
            }
        }
        if field("pass").is_some_and(|pass| *pass != Field::Bool(true)) {
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

    fn sim(fields: Vec<(String, Field)>) -> Record {
        Record { time: 2e-9, instance: "Top.m".into(), stream: "sim".into(), fields }
    }

    #[test]
    fn a_fatal_record_fails_the_verdict_with_its_message() {
        // FIX-02 AC 2 and 3: a `sim` record whose `failed` is known true fails the run and
        // `fatal()` returns it with its `msg`; the first one is kept
        let boom = sim(vec![("finish".into(), Field::Bool(true)), ("failed".into(), Field::Bool(true)), ("msg".into(), Field::Text("boom".into()))]);
        let later = sim(vec![("failed".into(), Field::Bool(true)), ("msg".into(), Field::Text("second".into()))]);
        let verdict = Verdict::judge(&[check("a", Field::Bool(true)), boom.clone(), later]);
        assert!(!verdict.passed());
        assert_eq!(verdict.fatal(), Some(&boom));
        assert_eq!(verdict.checks(), 1);
        assert!(verdict.failed().is_empty(), "no check failed");
        // an unknown `failed` is not a fatal
        let unknown = sim(vec![("failed".into(), Field::Unknown)]);
        assert!(Verdict::judge(&[unknown]).passed());
    }

    #[test]
    fn a_finish_record_does_not_fail_the_verdict() {
        // FIX-02 AC 4: `finish()` writes `sim` with `finish = true` and no `failed`
        let finish = sim(vec![("finish".into(), Field::Bool(true))]);
        let verdict = Verdict::judge(&[check("a", Field::Bool(true)), finish]);
        assert!(verdict.passed());
        assert_eq!(verdict.fatal(), None);
        // a `failed` that is false is no fatal either
        assert!(Verdict::judge(&[sim(vec![("failed".into(), Field::Bool(false))])]).passed());
    }

    fn target(name: &str, margin: Field) -> Record {
        Record {
            time: 1e-6,
            instance: "Top.mon".into(),
            stream: "checks".into(),
            fields: vec![("kind".into(), Field::Text("target".into())), ("name".into(), Field::Text(name.into())), ("margin".into(), margin)],
        }
    }

    #[test]
    fn the_last_record_of_each_target_is_kept_in_first_seen_order() {
        // HII-16 AC 1: the last margin of each name, in the order of each name's first record
        let records = [target("gain", Field::Real(0.5)), target("power", Field::Real(2.0)), target("gain", Field::Real(0.25)), target("power", Field::Real(1.0)), target("gain", Field::Real(0.125))];
        let verdict = Verdict::judge(&records);
        assert_eq!(verdict.targets(), [("gain".to_string(), 0.125), ("power".to_string(), 1.0)]);
        // an unknown margin shows as NaN
        assert!(Verdict::judge(&[target("x", Field::Unknown)]).targets()[0].1.is_nan());
    }

    #[test]
    fn a_negative_margin_does_not_fail() {
        // HII-16 AC 3 (decision X3): a target never fails a run, whatever its sign
        let verdict = Verdict::judge(&[target("gain", Field::Real(-3.0))]);
        assert!(verdict.passed());
        assert_eq!(verdict.targets(), [("gain".to_string(), -3.0)]);
        assert_eq!(verdict.checks(), 1);
    }
}
