//! A fault found while reading a package, as a coded diagnostic over the text it points into.

use std::path::PathBuf;

use piperine_lang::msl::Diagnostic;

/// A diagnostic of the `package` phase: the file it is about, the text its span indexes
/// (empty when the file could not be read) and the diagnostic itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageError {
    pub file: PathBuf,
    pub text: String,
    pub diagnostic: Diagnostic,
}

impl PackageError {
    /// The specification's code, `E0170` and the rows after it.
    pub fn code(&self) -> &str {
        &self.diagnostic.code
    }

    /// A `miette` graphical report, named by the file's own name.
    pub fn render(&self) -> String {
        let name = self.file.file_name().map_or_else(|| self.file.display().to_string(), |n| n.to_string_lossy().into_owned());
        self.diagnostic.render(&name, &self.text)
    }
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.code(), self.file.display(), self.diagnostic.message)
    }
}

impl std::error::Error for PackageError {}
