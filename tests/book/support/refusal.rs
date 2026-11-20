//! `Refusal`: what a Book test reads from a run that failed, and the assertion that a block the
//! Book shows refused is still refused (RE-02 AC 2 and 3).

use std::ops::Range;

use piperine::Failure;
use piperine_lang::msl::Diagnostic;

use super::model::Ran;

/// One diagnostic of a failed run: the parts a reader sees in a terminal.
pub struct Reported {
    pub code: String,
    pub message: String,
    /// The byte range of the primary label in the source file.
    pub primary_span: Range<usize>,
    pub primary_label: String,
    pub secondary_labels: Vec<String>,
    /// The byte range each secondary label points at, in the order of `secondary_labels`.
    pub secondary_spans: Vec<Range<usize>>,
    pub help: Option<String>,
}

impl Reported {
    /// The parts of `diagnostic`.
    fn of(diagnostic: &Diagnostic) -> Reported {
        Reported {
            code: diagnostic.code.to_string(),
            message: diagnostic.message.clone(),
            primary_span: diagnostic.primary.0.start..diagnostic.primary.0.end,
            primary_label: diagnostic.primary.1.clone(),
            secondary_labels: diagnostic.secondary.iter().map(|(_, label)| label.clone()).collect(),
            secondary_spans: diagnostic.secondary.iter().map(|(span, _)| span.start..span.end).collect(),
            help: diagnostic.help.clone(),
        }
    }

    /// The text of `source` the primary label points at.
    pub fn primary_text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.primary_span.clone()]
    }

    /// The text of `source` the secondary label `index` points at.
    pub fn secondary_text<'a>(&self, source: &'a str, index: usize) -> &'a str {
        &source[self.secondary_spans[index].clone()]
    }
}

/// The diagnostics a failed run gave.
pub struct Refusal;

impl Refusal {
    /// Every diagnostic `failure` carries, in the order the tool reports them. A failure with no
    /// span (`Host`, a solver error) is one diagnostic with an empty span.
    pub fn reported(failure: &Failure) -> Vec<Reported> {
        let spanless = |code: String, message: String| Reported { code, message, primary_span: 0..0, primary_label: String::new(), secondary_labels: Vec::new(), secondary_spans: Vec::new(), help: None };
        match failure {
            Failure::Package(errors) => errors.iter().map(|error| Reported::of(&error.diagnostic)).collect(),
            Failure::Diagnostics { found, .. } => found.iter().map(|found| Reported::of(&found.diagnostic)).collect(),
            Failure::Request { diagnostic, .. } => vec![Reported::of(diagnostic)],
            Failure::Host { code, message } => vec![spanless((*code).to_string(), message.clone())],
            other => {
                let message = other.to_string();
                vec![spanless(Self::code_in(&message).unwrap_or("-").to_string(), message)]
            }
        }
    }

    /// The first error code (`E` and four digits) written in `message`: a run failure carries its code
    /// in the text the solver gives.
    fn code_in(message: &str) -> Option<&str> {
        message.split(|character: char| !character.is_ascii_alphanumeric()).find(|word| word.len() == 5 && word.starts_with('E') && word[1..].chars().all(|digit| digit.is_ascii_digit()))
    }

    /// The diagnostics of `result` as `(code, message)`; `result` must have failed: panics naming
    /// `row` and saying the row now runs when it did not.
    pub fn of(result: Result<Ran, Failure>, row: &str) -> Vec<(String, String)> {
        Self::reported_by(result, row).into_iter().map(|reported| (reported.code, reported.message)).collect()
    }

    /// The diagnostics of `result` with all their parts; `result` must have failed (see [`Refusal::of`]).
    pub fn reported_by(result: Result<Ran, Failure>, row: &str) -> Vec<Reported> {
        match result {
            Ok(_) => panic!("{row}: now runs: make the row `run` and drop its `GAPS.md` section 11 line"),
            Err(failure) => Self::reported(&failure),
        }
    }

    /// Passes when `result` failed with a diagnostic whose code is `code` and whose message is
    /// `message`. Panics naming `row` and saying the row now runs when the run succeeded, and
    /// listing the codes and messages the run gave when none matches.
    pub fn expect(result: Result<Ran, Failure>, row: &str, code: &str, message: &str) {
        let given = Self::of(result, row);
        assert!(given.iter().any(|(given_code, given_message)| given_code == code && given_message == message), "{row}: expected {code}: {message}, the run gave {given:?}");
    }
}
