//! Scope: the Book's inventory guard (feature `root-e2e`, RE-01 and RE-02): the Book,
//! `docs/book/content.html`, is read into chapters, blocks and sentences, and every block has a
//! row of `tests/book/inventory.tsv` naming the test that holds it. This target links no workspace
//! crate: it is a repository-policy proof over the Book's text, the shape `ROOT_TARGET_EXEMPTIONS`
//! of `suite_hygiene.rs` registers.

#[path = "support"]
mod support {
    pub mod book;
}

use support::book::{Book, BlockId, Fingerprint};

#[test]
fn the_book_reads_every_block() {
    // RE-01 AC 1: the Book has 18 chapters and 214 code blocks (a mermaid diagram is not one), 99 of
    // them `msl`; every block has a distinct id that reads back, and every chapter has prose
    let book = Book::read();
    assert_eq!(book.chapters().len(), 18, "the chapters of the Book: {:?}", book.chapters());
    assert_eq!(book.blocks().len(), 214, "the code blocks of the Book");
    assert_eq!(book.blocks().iter().filter(|block| block.language() == "msl").count(), 99, "the `msl` blocks of the Book");
    let mut seen = Vec::new();
    for block in book.blocks() {
        let id = block.id().to_string();
        assert_eq!(BlockId::parse(&id).as_ref(), Some(block.id()), "{id} reads back");
        assert!(!seen.contains(&id), "{id} names two blocks");
        seen.push(id);
    }
    for chapter in book.chapters() {
        assert!(!book.sentences(&chapter).is_empty(), "the chapter `{chapter}` has no prose");
        let indices: Vec<usize> = book.blocks().iter().filter(|block| block.id().chapter == chapter).map(|block| block.id().index).collect();
        assert_eq!(indices, (1..=indices.len()).collect::<Vec<_>>(), "the blocks of `{chapter}` are numbered from 1 with no gap");
    }
    let welcome = book.sentences("welcome");
    assert_eq!(welcome[0].text(), "Chapter 1", "the first element of a chapter is its number");
    let goal = "By the end of this chapter you have created a project, run an RC filter from the command line, read its results, and made a model tell you what it is doing.";
    assert!(welcome.iter().any(|sentence| sentence.text() == goal), "the goal of the first chapter is one sentence");
    assert_eq!(Fingerprint::of("a").to_string(), "af63dc4c8601ec8c", "FNV-1a 64 of one byte");
    assert_eq!(welcome[0].fingerprint().to_string().len(), 16, "a fingerprint is sixteen hex digits");
    assert_eq!(book.blocks()[0].fingerprint(), Fingerprint::of(book.blocks()[0].text()), "a block's fingerprint is its text's");
}

#[test]
fn a_marked_file_joins_its_blocks() {
    // RE-02 AC 4: `clipper.msl` is the two blocks the Book marks with it, joined in order, each
    // followed by a newline
    let book = Book::read();
    let marked: Vec<_> = book.blocks().iter().filter(|block| block.file() == Some("clipper.msl")).collect();
    assert_eq!(marked.len(), 2, "the Book marks two blocks `clipper.msl`");
    let file = book.file("clipper.msl", marked[1].id());
    assert_eq!(file, format!("{}\n{}\n", marked[0].text(), marked[1].text()));
    assert!(file.starts_with("module Resistor("), "the file opens with the Book's first block");
    assert_eq!(book.file("clipper.msl", marked[0].id()), format!("{}\n", marked[0].text()), "at the first block the file is the first block");
}
