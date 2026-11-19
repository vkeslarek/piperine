//! `Faulty`: one faulty model of a Book chapter, run, and what the tool said of it. The smallest model
//! that makes the mistake a chapter names is written by the test and run through `piperine::`; the
//! diagnostics come back with the source they point into.

use piperine::Overrides;

use super::model::Model;
use super::refusal::{Refusal, Reported};

/// A faulty lone file `m.msl`, the diagnostics its run gave, and its text.
pub struct Faulty {
    pub source: String,
    pub given: Vec<Reported>,
}

impl Faulty {
    /// The overrides of an operating point of the top `top`, the run a lone file's check makes.
    pub fn operating_point_of(top: &str) -> Overrides {
        Overrides { analysis: Some("op".to_string()), top: Some(top.to_string()), ..Overrides::default() }
    }

    /// The overrides of a transient to `stop` with the step capped at `max_step`, on the top `Top`.
    pub fn transient_of_top(stop: f64, max_step: Option<f64>) -> Overrides {
        Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(stop), max_step, ..Overrides::default() }
    }

    /// The operating point of the top `top` of the lone file `m.msl` holding `source`, which the tool
    /// must refuse; panics naming `row` when it runs.
    pub fn elaborating(row: &str, source: &str, top: &str) -> Faulty {
        Faulty::running(row, source, Faulty::operating_point_of(top))
    }

    /// The run `overrides` ask of the lone file `m.msl` holding `source`, which must fail.
    pub fn running(row: &str, source: &str, overrides: Overrides) -> Faulty {
        let model = Model::written("m.msl", source);
        Faulty { source: model.text_of("m.msl"), given: Refusal::reported_by(model.run(overrides), row) }
    }

    /// The first diagnostic with the code `code`; panics listing what the run gave when there is none.
    pub fn first(&self, code: &str) -> &Reported {
        self.given.iter().find(|reported| reported.code == code).unwrap_or_else(|| panic!("no {code} in {:?}", self.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>()))
    }

    /// The text of the source that the primary label of the first `code` points at.
    pub fn pointed_at(&self, code: &str) -> &str {
        self.first(code).primary_text(&self.source)
    }
}
