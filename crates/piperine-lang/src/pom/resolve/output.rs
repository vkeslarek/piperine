//! What the resolution stages compute before it is applied.

use std::collections::BTreeMap;

use super::super::ids::{DeclId, FileId, PatId, PathId};
use super::super::resolution::Resolution;
use crate::msl::Diagnostic;

/// What the stages computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Output {
    /// `(path, segment index, what the segment names)`.
    pub segments: Vec<(PathId, usize, Resolution)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
    /// `(behavior block, module)`.
    pub links: BTreeMap<DeclId, DeclId>,
    /// Identifier patterns that name a constant, a unit variant or a unit struct.
    pub pats: Vec<(PatId, Resolution)>,
}
