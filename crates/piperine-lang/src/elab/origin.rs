//! Spans to `file:line:col`: the origin of every IR item (the msl-ir origin rule).

use piperine_ir::Origin;

use crate::msl::Span;
use crate::pom::{DeclId, Design, FileId};

/// Turns the byte offsets of the design's spans into the text origins the IR holds.
#[derive(Clone, Copy)]
pub(super) struct Locator<'d> {
    design: &'d Design,
}

impl<'d> Locator<'d> {
    pub(super) fn new(design: &'d Design) -> Self {
        Locator { design }
    }

    /// The start of `span` in `file`: 1-based line and column, the column counted in characters.
    pub(super) fn origin(&self, file: FileId, span: Span) -> Origin {
        let data = self.design.file_data(file);
        let offset = span.start.min(data.text.len());
        let before = data.text.get(..offset).unwrap_or("");
        let line = before.matches('\n').count() + 1;
        let line_start = before.rfind('\n').map_or(0, |at| at + 1);
        let col = before[line_start..].chars().count() + 1;
        Origin { file: data.name.clone(), line: line as u32, col: col as u32 }
    }

    /// The origin of a declaration: the start of its span.
    pub(super) fn decl(&self, decl: DeclId) -> Origin {
        let data = self.design.decl_data(decl);
        self.origin(data.file, data.span)
    }
}
