//! The structural guards over the specification and the Book's source: the pages of `docs/spec/` and `mkdocs.yml`, the error-code tables against the sources that raise them, and the MSL Book's source in `docs/book/`. Each former target is a module of this one.

#[path = "../support/spec_corpus.rs"]
#[allow(dead_code)] // item scope: this target needs `fragment_diagnostics` and nothing else of the module
mod spec_corpus;
#[path = "../support/spec_tables.rs"]
#[allow(dead_code)]
mod spec_tables;

mod book;
mod codes;
mod structure;
