//! The standard library as shipped and the specification's appendices as the library holds them: the excerpts and galleries of the spec read through the front end, and the embedded library read through the POM. Each former target is a module of this one.

#[path = "../support/spec_corpus.rs"]
mod spec_corpus;
#[path = "../support/spec_tables.rs"]
mod spec_tables;
#[path = "../support/elab.rs"]
#[allow(dead_code)]
mod support;

mod excerpts;
mod galleries;
mod library;
