//! Scope: the gallery gate of the standard library (STD-13, STD-14). The 16 gallery fences of
//! Appendices D, E and F name models that other sections define; the fixtures under
//! `tests/fixtures/gallery/` hold those models, every line copied from `docs/spec/`, and a table
//! in this file says which fixtures each fence is assembled with and which `std` items it must
//! reference. The table is data; each check is a plain function that returns its failures, and the
//! tests assert the list is empty on the real tree and non-empty on text written in the test.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::msl::format::Formatter;
use piperine_lang::pom::{Design, ExprKind, Reference, Reflect, Resolution};
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

const TABLE: [Row; 16] = [
    Row { section: "D.1", fixtures: &[], std_items: &["carriers::Electrical", "carriers::Bit"] },
    Row { section: "D.2", fixtures: &[], std_items: &["carriers::Bit", "analog::normal"] },
    Row { section: "D.3", fixtures: &[], std_items: &["analog::idtmod", "analog::above"] },
    Row { section: "D.4", fixtures: &[], std_items: &["analog::transition", "carriers::Electrical"] },
    Row { section: "D.5", fixtures: &[], std_items: &["carriers::Electrical", "values::impl Bit.rise"] },
    Row { section: "D.6", fixtures: &["Capacitor"], std_items: &["carriers::Electrical", "carriers::Bit"] },
    Row { section: "E.1", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise", "values::impl Bit.fall"] },
    Row { section: "E.2", fixtures: &[], std_items: &["carriers::Bit"] },
    Row { section: "E.3", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "E.4", fixtures: &["Tx"], std_items: &["carriers::Bit"] },
    Row { section: "E.5", fixtures: &[], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "E.6", fixtures: &["Stream"], std_items: &["carriers::Bit", "values::impl Bit.rise"] },
    Row { section: "F.1", fixtures: &["SampleHold", "Comparator"], std_items: &["carriers::Electrical", "carriers::Bit"] },
    Row { section: "F.2", fixtures: &["Resistor", "Capacitor", "Vco"], std_items: &["carriers::Electrical", "values::impl Bit.rise"] },
    Row { section: "F.3", fixtures: &["Cell", "Dram1T1C", "MemoryBank", "Capacitor", "RSwitch"], std_items: &["carriers::Bit", "carriers::Electrical"] },
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
    assert_eq!(fences.len(), 16, "Appendices D, E and F hold 16 gallery fences (E.7 and F.4 are fragments until `Resolve` runs)");
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

/// The root file of one gallery: the fence, then the fixtures of its row.
fn assemble(fence: &str, fixtures: &[&str]) -> String {
    let mut text = fence.to_string();
    for model in fixtures {
        text.push('\n');
        text.push_str(&fixture(model));
    }
    text
}

fn fixtures_of(section: &str) -> &'static [&'static str] {
    TABLE.iter().find(|r| r.section == section).unwrap_or_else(|| panic!("a row for {section}")).fixtures
}

/// The gallery built against the embedded library.
fn build(text: &str) -> Design {
    Design::builder().source("top.msl", text).build()
}

/// The gallery built against a library whose prelude is empty.
fn build_against_empty_prelude(text: &str) -> Design {
    Design::builder().std_source("lib.msl", "pub mod prelude {}").source("top.msl", text).build()
}

fn gallery_design(g: &Gallery) -> Design {
    build(&assemble(&g.block.text, fixtures_of(&g.section)))
}

/// The diagnostics of the design, one line each: the fence's file and line, the code, the text of
/// the offending span, and the file it is in.
fn diagnostic_lines(design: &Design, g: &Gallery) -> Vec<String> {
    let files = design.files();
    design
        .diagnostics()
        .iter()
        .map(|d| {
            let file = &files[d.file.0 as usize];
            let span = d.diagnostic.primary.0;
            let source = file.text().get(span.start..span.end).unwrap_or("?");
            format!("{}:{}: {} `{source}` in {}", g.block.file, g.block.line, d.diagnostic.code, file.name())
        })
        .collect()
}

/// The `Field` and `MethodCall` slots of the root crate that stayed `Unresolved`, by name.
fn unresolved_members(design: &Design, g: &Gallery) -> Vec<String> {
    let mut out = Vec::new();
    for decl in design.root_crate().decls() {
        for expr in decl.exprs() {
            if matches!(expr.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }) && expr.resolution() == Resolution::Unresolved {
                out.push(format!("{}:{}: unresolved member `{}`", g.block.file, g.block.line, expr.text()));
            }
        }
    }
    out
}

/// The items of `names` that are not `std` declarations or have no reference in the root crate.
fn unreferenced_std_items(design: &Design, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| match design.std_crate().find(name) {
            Some(item) => {
                let in_root = item.references().iter().any(|r| match r {
                    Reference::Path(p) => p.enclosing().krate().is_root(),
                    Reference::Member(e) => e.enclosing().krate().is_root(),
                    Reference::Pattern(p) => p.enclosing().krate().is_root(),
                });
                !(item.krate().is_std() && in_root)
            }
            None => true,
        })
        .map(|n| n.to_string())
        .collect()
}

/// The gate fails unless it builds exactly 16 fences.
fn require_16(built: usize) -> Result<(), String> {
    if built == 16 { Ok(()) } else { Err(format!("the gate built {built} gallery fences, not 16")) }
}

#[test]
fn every_gallery_builds_with_no_diagnostics() {
    let mut all = Vec::new();
    for g in galleries() {
        all.extend(diagnostic_lines(&gallery_design(&g), &g));
    }
    assert!(all.is_empty(), "{} diagnostics:\n{}", all.len(), all.join("\n"));
}

#[test]
fn every_gallery_passes_check() {
    for g in galleries() {
        let problems = gallery_design(&g).check();
        assert!(problems.is_empty(), "{}:{}: {problems:?}", g.block.file, g.block.line);
    }
}

#[test]
fn no_gallery_has_an_unresolved_member() {
    let mut all = Vec::new();
    for g in galleries() {
        all.extend(unresolved_members(&gallery_design(&g), &g));
    }
    assert!(all.is_empty(), "{}", all.join("\n"));
}

#[test]
fn every_gallery_references_its_std_items() {
    for g in galleries() {
        let row = TABLE.iter().find(|r| r.section == g.section).expect("a row");
        let missing = unreferenced_std_items(&gallery_design(&g), row.std_items);
        assert!(missing.is_empty(), "{} ({}:{}) does not reference the std items {missing:?}", g.section, g.block.file, g.block.line);
    }
}

#[test]
fn the_gate_builds_16_fences() {
    let fences = galleries();
    let built = fences.iter().map(|g| gallery_design(g)).count();
    assert_eq!(require_16(built), Ok(()));
    assert!(require_16(15).is_err() && require_16(17).is_err(), "another number fails the gate");
}

#[test]
fn a_dirty_gallery_is_reported_by_fence() {
    let g = Gallery {
        section: "D.1".to_string(),
        block: Block { file: "appendix_x.md".to_string(), line: 42, class: Class::Valid, text: "module M(p: Electrical) {}\nconst X: f64 = NoSuchName;\n".to_string() },
    };
    let design = build(&g.block.text);
    let lines = diagnostic_lines(&design, &g);
    assert!(lines.iter().any(|l| l.starts_with("appendix_x.md:42: E0161 `NoSuchName`")), "{lines:?}");
    let g2 = Gallery {
        section: "D.1".to_string(),
        block: Block { file: "appendix_y.md".to_string(), line: 7, class: Class::Valid, text: "module M(in c: Bit) { }\ndigital for M { on c.nosuchmethod() { } }\n".to_string() },
    };
    let unresolved = unresolved_members(&build(&g2.block.text), &g2);
    assert!(unresolved.iter().any(|l| l.starts_with("appendix_y.md:7: unresolved member") && l.contains("nosuchmethod")), "{unresolved:?}");
}

#[test]
fn galleries_fail_against_an_empty_prelude() {
    for g in galleries() {
        let design = build_against_empty_prelude(&assemble(&g.block.text, fixtures_of(&g.section)));
        let e0161 = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0161").count();
        assert!(e0161 >= 1, "{} ({}:{}) builds without E0161 against an empty prelude", g.section, g.block.file, g.block.line);
    }
}

#[test]
fn every_gallery_lists_a_std_item() {
    assert_eq!(TABLE.len(), 16);
    for row in &TABLE {
        assert!(!row.std_items.is_empty(), "{} lists no std item", row.section);
    }
}

/// PI-11 AC 3: each of the 16 gallery fences is the formatter's output for itself, so a reader
/// copies it into a file `piperine fmt --check` accepts.
#[test]
fn every_gallery_fence_is_in_the_formatters_form() {
    let fences = galleries();
    assert_eq!(fences.len(), 16, "the galleries hold 16 fences");
    for gallery in fences {
        let text = format!("{}\n", gallery.block.text.trim_end());
        let formatted = Formatter::format(&text).unwrap_or_else(|d| panic!("{} ({}:{}): {d:?}", gallery.section, gallery.block.file, gallery.block.line));
        assert_eq!(formatted, text, "{} ({}:{}) is not in the formatter's form", gallery.section, gallery.block.file, gallery.block.line);
    }
}
