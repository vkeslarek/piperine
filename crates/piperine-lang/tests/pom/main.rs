//! The program object model: lowering of source to the design, the body and bundle forms, name resolution, classification, the tree and the reflection handles, the corpus of every spec fence through the POM, the documentation read back, the crate view, and the buck-converter example package read through the POM. Each former target is a module of this one.

#[path = "../support/lowering.rs"]
mod lowering;
#[path = "../support/spec_corpus.rs"]
mod spec_corpus;
#[path = "../support/pom.rs"]
mod support;

mod body;
mod buck_example;
mod bundles;
mod classify;
mod corpus;
mod crates;
mod design;
mod docs;
mod reflect;
mod resolve;
mod tree;
