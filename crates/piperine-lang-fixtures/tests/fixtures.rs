//! Scope: each fixture type of `piperine-lang-fixtures` held to its own contract: a source builds a design without a
//! diagnostic, a checked source reports what the checker says, an elaborated source yields an `Ir` that passes `check`,
//! the buck's items are found and the specification's corpus and tables are read. A fixture that silently stopped doing
//! what its doc says would make every suite built on it vacuous.

use piperine_lang_fixtures::buck::BuckSource;
use piperine_lang_fixtures::checked::CheckedSource;
use piperine_lang_fixtures::elaboration::SourceElaboration;
use piperine_lang_fixtures::pom::PomSource;
use piperine_lang_fixtures::spec_corpus::SpecCorpus;
use piperine_lang_fixtures::spec_tables::SpecTables;
use piperine_lang_fixtures::twin::TwinSource;

#[test]
fn a_pom_source_builds_a_clean_design_and_names_the_codes_of_a_bad_one() {
    let clean = PomSource::build("module Top() { }");
    assert_eq!(PomSource::codes(&clean), Vec::<String>::new());
    let refused = PomSource::build("module Top() { let x = missing; }");
    assert!(!PomSource::codes(&refused).is_empty(), "an unresolved name is a diagnostic of the design");
}

#[test]
fn a_checked_source_reads_back_what_the_checker_says() {
    let src = CheckedSource::source("", "let a = 1.0; let b = true;");
    CheckedSource::assert_clean(&src);
    assert_eq!(CheckedSource::local_types_of(&src, &["a", "b"]).len(), 2);
    let (bad, found) = CheckedSource::body_diagnostics("", "let a: bool = 1.0;");
    assert_eq!(found.len(), 1, "{:?}", CheckedSource::messages(&found));
    assert_eq!(CheckedSource::codes(&found), [found[0].diagnostic.code.to_string()]);
    assert_eq!(CheckedSource::snippet(&bad, &found[0]), "1.0");
    CheckedSource::assert_refused(&bad, &found[0].diagnostic.code.to_string(), &found[0].diagnostic.message);
}

#[test]
#[should_panic(expected = "expected one diagnostic")]
fn a_clean_source_is_not_refused() {
    CheckedSource::assert_refused(&CheckedSource::source("", "let a = 1.0;"), "E0180", "");
}

#[test]
fn a_source_elaborates_to_an_ir_that_passes_its_own_check() {
    let ir = SourceElaboration::ir("module Top() { signal a: Electrical; }", "Top");
    assert!(ir.nodes().count() > 0, "the signal of `Top` is a node of the Ir");
}

#[test]
fn the_buck_items_are_found_as_written() {
    let item = BuckSource::item("supervisor.msl", "pub enum Mode");
    assert!(item.contains("pub enum Mode") && item.trim_end().ends_with('}'), "{item}");
    assert!(BuckSource::function("dpwm.msl", "rev3").starts_with("fn rev3("));
    assert!(!BuckSource::spec().is_empty());
}

#[test]
fn a_twin_design_loads_from_disk() {
    assert!(TwinSource::design("divider").diagnostics().is_empty());
}

#[test]
fn the_specification_corpus_and_tables_are_read() {
    assert!(!SpecCorpus::corpus().is_empty(), "docs/spec holds fenced blocks");
    let section = SpecTables::section("# Title\n\n## Part\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n## Next\n", "## Part");
    assert_eq!(SpecTables::table_rows(&section), vec![vec!["1".to_string(), "2".to_string()]]);
}
