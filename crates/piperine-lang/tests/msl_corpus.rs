//! Scope: the specification's own code blocks as the acceptance corpus of the
//! MSL parser (requirement MSLP-03). Every ```` ```rust* ```` fence in
//! `docs/spec/*.md` is extracted at test time and classified by its info
//! string: `rust` must parse with zero diagnostics, `rust,compile_fail`
//! illustrates an error and must report at least one, `rust,fragment` is a
//! piece of a file (statements, items without their context) and must parse
//! as a file or as the body of a block. A `rust,*` info string of any other
//! class fails the suite, so a new class cannot slip in unchecked.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::msl::parser::SourceParser;
use spec_corpus::{Block, Class, corpus, extract, fragment_diagnostics};

fn count(blocks: &[Block], class: Class) -> usize {
    blocks.iter().filter(|b| b.class == class).count()
}

#[test]
fn every_valid_spec_block_parses_clean() {
    let blocks = corpus();
    println!(
        "corpus: {} rust, {} rust,compile_fail, {} rust,fragment",
        count(&blocks, Class::Valid),
        count(&blocks, Class::CompileFail),
        count(&blocks, Class::Fragment)
    );
    assert!(count(&blocks, Class::Valid) > 0, "the corpus is not empty");
    let mut failures = Vec::new();
    for b in &blocks {
        let diags: Vec<String> = match b.class {
            Class::Valid => SourceParser::parse(&b.text).diagnostics.iter().map(|d| format!("{}: {}", d.code, d.message)).collect(),
            Class::Fragment => fragment_diagnostics(&b.text),
            Class::CompileFail => continue,
        };
        if !diags.is_empty() {
            failures.push(format!("{}:{} ({:?}): {}", b.file, b.line, b.class, diags.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{} block(s) do not parse:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_compile_fail_block_diagnoses() {
    let blocks = corpus();
    let fails: Vec<&Block> = blocks.iter().filter(|b| b.class == Class::CompileFail).collect();
    println!("compile_fail blocks in docs/spec: {}", fails.len());
    // Every `rust,compile_fail` fence illustrates an error: the file parse
    // alone must report it (no context that would accept it).
    let silent: Vec<String> = fails
        .iter()
        .filter(|b| SourceParser::parse(&b.text).diagnostics.is_empty())
        .map(|b| format!("{}:{}", b.file, b.line))
        .collect();
    assert!(silent.is_empty(), "compile_fail blocks that parse clean: {silent:?}");

    // The spec holds no error illustration as a fence today, so the loop above
    // can be empty: a control fence runs the same classification and check, so
    // the test still fails if `rust,compile_fail` stopped being recognized or
    // a clean source were accepted as an error example.
    let control = extract("control.md", "```rust,compile_fail\nfn f( { }\n```\n```rust\nfn f() {}\n```\n");
    assert_eq!(control.len(), 2);
    assert_eq!(control[0].class, Class::CompileFail);
    assert!(!SourceParser::parse(&control[0].text).diagnostics.is_empty(), "a broken source diagnoses");
    assert_eq!(control[1].class, Class::Valid);
    assert!(SourceParser::parse(&control[1].text).diagnostics.is_empty());
}
