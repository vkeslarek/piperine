//! Scope: the gallery gate of the standard library (STD-13, STD-14). The 18 gallery fences of
//! Appendices D, E and F name models that other sections define; the fixtures under
//! `tests/fixtures/gallery/` hold those models, every line copied from `docs/spec/`, and a table
//! in this file says which fixtures each fence is assembled with and which `std` items it must
//! reference. The table is data; each check is a plain function that returns its failures, and the
//! tests assert the list is empty on the real tree and non-empty on text written in the test.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use std::collections::BTreeSet;
use std::path::PathBuf;

use spec_corpus::{Block, Class, corpus};

/// One gallery section: (section, fixture models appended to its fence, `std` items it must
/// reference at least, by qualified name).
struct Row {
    section: &'static str,
    fixtures: &'static [&'static str],
    std_items: &'static [&'static str],
}

/// The twelve models the galleries use and no gallery fence defines, except `Vco` (D.3) and
/// `RSwitch` (D.6), which F.2 and F.3 take from there.
const MODELS: [&str; 12] =
    ["Resistor", "Capacitor", "Switch", "SampleHold", "Comparator", "Cell", "Dram1T1C", "MemoryBank", "Stream", "Tx", "Vco", "RSwitch"];

const TABLE: [Row; 18] = [
    Row { section: "D.1", fixtures: &[], std_items: &["carriers::Electrical", "values::Known.known_or"] },
    Row { section: "D.2", fixtures: &[], std_items: &["carriers::Bit", "analog::normal"] },
    Row { section: "D.3", fixtures: &[], std_items: &["analog::idtmod", "analog::above"] },
    Row { section: "D.4", fixtures: &[], std_items: &["analog::transition", "carriers::Electrical"] },
    Row { section: "D.5", fixtures: &[], std_items: &["carriers::Electrical", "values::impl Bit.rise"] },
    Row { section: "D.6", fixtures: &["Capacitor"], std_items: &["carriers::Electrical", "values::impl Bit.high"] },
    Row { section: "E.1", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise", "values::impl Bit.fall"] },
    Row { section: "E.2", fixtures: &[], std_items: &["carriers::Bit"] },
    Row { section: "E.3", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "E.4", fixtures: &["Tx"], std_items: &["carriers::Bit"] },
    Row { section: "E.5", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "E.6", fixtures: &["Stream"], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "E.7", fixtures: &[], std_items: &["nets::Wire", "nets::Pullup", "nets::Logic"] },
    Row { section: "F.1", fixtures: &["SampleHold", "Comparator"], std_items: &["carriers::Electrical", "values::Known.known_or"] },
    Row { section: "F.2", fixtures: &["Resistor", "Capacitor", "Vco"], std_items: &["carriers::Electrical", "values::impl Bit.rise"] },
    Row { section: "F.3", fixtures: &["Cell", "Dram1T1C", "MemoryBank", "Capacitor", "RSwitch"], std_items: &["carriers::Bit", "carriers::Electrical"] },
    Row { section: "F.4", fixtures: &[], std_items: &["nets::Wire", "nets::Keeper", "nets::Tran"] },
    Row { section: "F.5", fixtures: &["Capacitor", "Switch"], std_items: &["carriers::Electrical", "carriers::Bit"] },
];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gallery")
}

fn spec_text(file: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec").join(file)).expect("spec file")
}

/// A gallery fence with its section (`D.1` ...): the fences of Appendices D, E and F.
struct Gallery {
    section: String,
    block: Block,
}

/// The nearest heading `## X.n` above each `rust` fence of Appendices D, E and F.
fn galleries() -> Vec<Gallery> {
    let mut out = Vec::new();
    for block in corpus().into_iter().filter(|b| ["appendix_d", "appendix_e", "appendix_f"].iter().any(|p| b.file.starts_with(p))) {
        if block.class != Class::Valid {
            continue;
        }
        let text = spec_text(&block.file);
        let section = text
            .lines()
            .take(block.line - 1)
            .filter_map(|l| l.strip_prefix("## "))
            .filter_map(|h| h.split_whitespace().next())
            .filter(|w| w.len() == 3 && w.as_bytes()[1] == b'.' && w.as_bytes()[0].is_ascii_uppercase() && w.as_bytes()[2].is_ascii_digit())
            .last()
            .unwrap_or_else(|| panic!("{}:{}: a gallery fence has a section heading above it", block.file, block.line))
            .to_string();
        out.push(Gallery { section, block });
    }
    out
}

fn fixture(model: &str) -> String {
    std::fs::read_to_string(fixture_dir().join(format!("{model}.msl"))).unwrap_or_else(|e| panic!("fixture {model}.msl: {e}"))
}

/// The trimmed lines of every `rust` fence of `docs/spec/`, and the code spans of Appendix D's
/// table "Models referenced from the normative parts".
fn spec_lines() -> BTreeSet<String> {
    let mut lines: BTreeSet<String> = corpus().iter().flat_map(|b| b.text.lines().map(|l| l.trim().to_string()).collect::<Vec<_>>()).collect();
    let d = spec_text("appendix_d_gallery_analog.md");
    let start = d.find("**Models referenced from the normative parts.**").expect("Appendix D lists the models it assumes");
    let end = start + d[start..].find("> **Resolution.**").expect("the table ends at the Resolution note");
    for (i, part) in d[start..end].split('`').enumerate() {
        if i % 2 == 1 {
            lines.insert(part.trim().to_string());
        }
    }
    lines
}

/// The lines of `text` that are not in `known`, trimmed; empty lines are skipped.
fn foreign_lines(text: &str, known: &BTreeSet<String>) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !known.contains(*l)).map(str::to_string).collect()
}

fn defines(text: &str, model: &str) -> bool {
    ["module", "struct", "enum", "bundle"].iter().any(|kw| text.contains(&format!("{kw} {model}")))
}

fn uses(text: &str, model: &str) -> bool {
    let bytes = text.as_bytes();
    text.match_indices(model).any(|(i, _)| {
        let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        (i == 0 || !word(bytes[i - 1])) && bytes.get(i + model.len()).is_none_or(|b| !word(*b))
    })
}

/// The (section, model) pairs of `table` that name a model the fence does not use, or one it
/// defines itself.
fn table_misuse(table: &[Row], fences: &[(String, String)]) -> Vec<(String, String)> {
    let mut bad = Vec::new();
    for row in table {
        let fence = fences.iter().find(|(s, _)| s == row.section).map(|(_, t)| t.as_str()).unwrap_or("");
        for model in row.fixtures {
            if !uses(fence, model) || defines(fence, model) {
                bad.push((row.section.to_string(), model.to_string()));
            }
        }
    }
    bad
}

fn fence_texts() -> Vec<(String, String)> {
    galleries().into_iter().map(|g| (g.section, g.block.text)).collect()
}

#[test]
fn gallery_fixtures_exist() {
    let mut on_disk: Vec<String> = std::fs::read_dir(fixture_dir())
        .expect("tests/fixtures/gallery exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = MODELS.iter().map(|m| format!("{m}.msl")).collect();
    want.sort();
    assert_eq!(on_disk, want, "one fixture file for each of the twelve models, and no other");
    for model in MODELS {
        assert!(defines(&fixture(model), model), "{model}.msl declares {model}");
    }
}

#[test]
fn every_fixture_line_is_spec_text() {
    let known = spec_lines();
    for model in MODELS {
        let foreign = foreign_lines(&fixture(model), &known);
        assert!(foreign.is_empty(), "{model}.msl has lines that are in no `rust` fence and no code span of Appendix D's table: {foreign:?}");
    }
    assert_eq!(foreign_lines("module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}\nmodule NotInTheSpec_x9() {}\n", &known), vec!["module NotInTheSpec_x9() {}".to_string()], "an invented line is named");
}

#[test]
fn the_fixture_table_covers_every_gallery() {
    let fences = galleries();
    assert_eq!(fences.len(), 18, "Appendices D, E and F hold 18 gallery fences");
    let from_spec: BTreeSet<&str> = fences.iter().map(|g| g.section.as_str()).collect();
    let from_table: BTreeSet<&str> = TABLE.iter().map(|r| r.section).collect();
    assert_eq!(from_table.len(), TABLE.len(), "no section has two rows");
    assert_eq!(from_table, from_spec, "one row for each gallery section");
    for row in &TABLE {
        for model in row.fixtures {
            assert!(MODELS.contains(model), "{}: `{model}` is not one of the twelve fixture models", row.section);
        }
        for item in row.std_items {
            assert!(item.contains("::"), "{}: `{item}` is a qualified `std` name", row.section);
        }
    }
}

#[test]
fn the_fixture_table_names_only_used_models() {
    assert_eq!(table_misuse(&TABLE, &fence_texts()), Vec::<(String, String)>::new());
    let bad = [
        Row { section: "D.3", fixtures: &["Vco"], std_items: &[] },
        Row { section: "D.1", fixtures: &["Tx"], std_items: &[] },
    ];
    assert_eq!(
        table_misuse(&bad, &fence_texts()),
        vec![("D.3".to_string(), "Vco".to_string()), ("D.1".to_string(), "Tx".to_string())],
        "a model the fence defines (Vco in D.3) and a model it does not use (Tx in D.1) are named as pairs"
    );
}
