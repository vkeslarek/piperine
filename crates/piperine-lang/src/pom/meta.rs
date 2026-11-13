//! Documentation and attributes attached to declarations.

use serde::{Deserialize, Serialize};

use crate::msl::Span;

/// A doc comment: the text after the marker, or between the markers of a
/// block comment, verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub text: String,
    pub block: bool,
    pub span: Span,
}

/// What precedes an item, field, port, variant or match arm.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Meta {
    pub docs: Vec<Doc>,
}
