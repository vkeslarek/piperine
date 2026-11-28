//! The Book, chapter by chapter, run through `piperine::` (feature `root-e2e`, RE-01 to RE-03): each chapter's models
//! run through `Project` and `Session`, a model the Book shows refused is asserted refused with the codes and messages
//! the tool gives, and the two guards read the Book's text itself: `inventory_guard` (every block has one row of
//! `tests/book/inventory.tsv`) and `claims_guard` (every sentence that states a value or an outcome has a row).
//! One module per chapter; the support modules the chapters share are declared once, here.

#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod gaps;
    pub mod inventory;
    pub mod manifests;
    pub mod model;
    pub mod printed;
    pub mod records;
    pub mod refusal;
    pub mod standard_library;
    pub mod value_file;
}

mod abi;
mod analog;
mod cheatsheet;
mod claims_guard;
mod cli;
mod compiler;
mod core_std;
mod digital;
mod errors;
mod generics;
mod inventory_guard;
mod language;
mod mixed;
mod modules;
mod natures;
mod packages;
mod pom;
mod simulation;
mod solver;
mod structure;
mod types;
mod values;
mod welcome;
