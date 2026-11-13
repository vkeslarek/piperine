//! Handles over bodies and files: [`SourceFile`] here, expression and
//! statement handles as the body tasks add them.

use std::path::Path;

use super::super::design::Design;
use super::super::ids::FileId;
use crate::msl::ast;

/// A source file of a [`Design`]: its name, text and retained syntax tree.
#[derive(Clone, Copy)]
pub struct SourceFile<'d> {
    design: &'d Design,
    id: FileId,
}

impl<'d> SourceFile<'d> {
    pub(crate) fn new(design: &'d Design, id: FileId) -> Self {
        SourceFile { design, id }
    }

    pub fn id(self) -> FileId {
        self.id
    }

    /// The name it was registered under, or its file name on disk.
    pub fn name(self) -> &'d str {
        &self.design.file_data(self.id).name
    }

    /// The path it was read from, virtual or on disk.
    pub fn path(self) -> Option<&'d Path> {
        self.design.file_data(self.id).path.as_deref()
    }

    pub fn text(self) -> &'d str {
        &self.design.file_data(self.id).text
    }

    /// The syntax tree the parser returned, errors recovered.
    pub fn ast(self) -> &'d ast::File {
        &self.design.file_data(self.id).ast
    }

    pub fn is_prelude(self) -> bool {
        self.design.file_data(self.id).prelude
    }
}

impl PartialEq for SourceFile<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for SourceFile<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SourceFile({:?}, {})", self.id, self.name())
    }
}

impl Design {
    /// Every file, prelude sources first, then the root, then loaded files.
    pub fn files(&self) -> Vec<SourceFile<'_>> {
        (0..self.file_count()).map(|i| SourceFile::new(self, FileId(i as u32))).collect()
    }
}
