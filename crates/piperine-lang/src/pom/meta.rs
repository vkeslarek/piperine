//! Documentation and attributes attached to declarations.

use serde::{Deserialize, Serialize};

use super::body::Lit;
use super::decl::Name;
use super::ids::{ExprId, FileId, PathId};
use crate::msl::Span;

/// A doc comment: the text after the marker, or between the markers of a
/// block comment, verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub text: String,
    pub block: bool,
    /// Written as `//!` or `/*! */`: it documents the namespace around it,
    /// not the item after it.
    pub inner: bool,
    /// The file the comment was written in (its `span` is an offset there).
    pub file: FileId,
    pub span: Span,
}

/// What precedes an item, field, port, variant or match arm, in order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Meta {
    pub docs: Vec<Doc>,
    pub attrs: Vec<AttrNode>,
}

/// `#[path]`, `#[path(args)]` or `#[path = expr]`. The path is stored as
/// written and is not resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttrNode {
    pub path: PathId,
    pub input: AttrInputNode,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrInputNode {
    None,
    /// `#[doc = "text"]`
    Eq(ExprId),
    /// `#[derive(Repr, Clone)]`, `#[link("x.osdi", model = "bsim4")]`
    List(Vec<AttrArgNode>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttrArgNode {
    Path(PathId),
    Lit(Lit),
    NameValue(Name, Lit),
}
