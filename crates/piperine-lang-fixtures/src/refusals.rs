//! What elaborating a source refuses, as the codes and messages of its diagnostics.

use crate::elaboration::SourceElaboration;

/// The diagnostics a source's elaboration reports.
pub struct ElaborationRefusals;

impl ElaborationRefusals {
    /// What elaborating `Top` of `src` refuses, as `(code, message)` pairs; empty when it elaborates.
    pub fn of(src: &str) -> Vec<(String, String)> {
        match SourceElaboration::run(src, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }
}
