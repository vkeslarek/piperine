//! A directory tree copied under a directory of its own, so a test runs a package (`examples/buck`) as the package it
//! is without writing into the repository.

use std::path::Path;

/// The copy of one directory tree.
pub struct CopiedTree;

impl CopiedTree {
    /// Copies the tree `from` to `to`, creating every directory it needs.
    pub fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("a directory for the copy");
        for entry in std::fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display())) {
            let entry = entry.expect("a directory entry");
            let target = to.join(entry.file_name());
            if entry.file_type().expect("a file type").is_dir() {
                Self::copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).unwrap_or_else(|error| panic!("{}: {error}", target.display()));
            }
        }
    }
}
