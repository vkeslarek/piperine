//! `Verdict`: pass or fail of one run under `piperine test` (CLI-16). The records of the
//! reserved `checks` stream are the checks; a check whose `pass` is false, or unknown, fails
//! the run. Every check record counts, a passing one too: what a terminal hides never reaches
//! this type's input as a filter.

use crate::record::{Field, Record};

/// The checks of a run and the ones that failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    checks: usize,
    failed: Vec<Record>,
}

impl Verdict {
    /// Judges `records`, the whole record stream of a run: counts those of the `checks` stream
    /// and keeps those whose `pass` is false or unknown. A `checks` record with no `pass` field
    /// (a `target`, which reports a margin) counts and does not fail.
    pub fn judge(records: &[Record]) -> Verdict {
        let checks: Vec<&Record> = records.iter().filter(|r| r.stream == "checks").collect();
        let failed = checks
            .iter()
            .filter(|r| r.fields.iter().any(|(name, field)| name == "pass" && *field != Field::Bool(true)))
            .map(|r| (*r).clone())
            .collect();
        Verdict { checks: checks.len(), failed }
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
}
