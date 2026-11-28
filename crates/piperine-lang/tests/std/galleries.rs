//! Scope: the gallery gate of the standard library (STD-13, STD-14). The 20 gallery fences of
//! Appendices D, E and F name models that other sections define; the fixtures under
//! `tests/fixtures/gallery/` hold those models, every line copied from `docs/spec/`, and a table
//! in this file says which fixtures each fence is assembled with and which `std` items it must
//! reference. The table is data; each check is a plain function that returns its failures, and the
//! tests assert the list is empty on the real tree and non-empty on text written in the test.


use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::msl::format::Formatter;
use piperine_lang::pom::{Design, ExprKind, Reference, Reflect, Resolution};
use piperine_lang_fixtures::spec_corpus::{BlockClass, SpecBlock, SpecCorpus};

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
    Row { section: "E.7", fixtures: &[], std_items: &["carriers::Bit", "nets::Driven", "nets::Pullup"] },
    Row { section: "F.1", fixtures: &["SampleHold", "Comparator"], std_items: &["carriers::Electrical", "carriers::Bit"] },
    Row { section: "F.2", fixtures: &["Resistor", "Capacitor", "Vco"], std_items: &["carriers::Electrical", "values::impl Bit.rise"] },
    Row { section: "F.3", fixtures: &["Cell", "Dram1T1C", "MemoryBank", "Capacitor", "RSwitch"], std_items: &["carriers::Bit", "carriers::Electrical"] },
    // the three nets of F.4 are three fences of one section: each declares its own nature and modules and places a `Bit` input
    Row { section: "F.4", fixtures: &[], std_items: &["carriers::Bit"] },
    Row { section: "F.5", fixtures: &["Capacitor", "Switch"], std_items: &["carriers::Electrical", "carriers::Bit"] },
];

/// The gallery fixtures on disk and the specification text they are checked against.
struct GalleryText;

impl GalleryText {
    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gallery")
    }

    fn spec_text(file: &str) -> String {
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec").join(file)).expect("spec file")
    }

    /// The nearest heading `## X.n` above each `rust` fence of Appendices D, E and F.
    fn galleries() -> Vec<Gallery> {
        let mut out = Vec::new();
        for block in SpecCorpus::corpus().into_iter().filter(|b| ["appendix_d", "appendix_e", "appendix_f"].iter().any(|p| b.file.starts_with(p))) {
            if block.class != BlockClass::Valid {
                continue;
            }
            let text = GalleryText::spec_text(&block.file);
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
        std::fs::read_to_string(GalleryText::fixture_dir().join(format!("{model}.msl"))).unwrap_or_else(|e| panic!("fixture {model}.msl: {e}"))
    }

    /// The trimmed lines of every `rust` fence of `docs/spec/`, and the code spans of Appendix D's
    /// table "Models referenced from the normative parts".
    fn spec_lines() -> BTreeSet<String> {
        let mut lines: BTreeSet<String> = SpecCorpus::corpus().iter().flat_map(|b| b.text.lines().map(|l| l.trim().to_string()).collect::<Vec<_>>()).collect();
        let d = GalleryText::spec_text("appendix_d_gallery_analog.md");
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
                if !GalleryText::uses(fence, model) || GalleryText::defines(fence, model) {
                    bad.push((row.section.to_string(), model.to_string()));
                }
            }
        }
        bad
    }

    fn fence_texts() -> Vec<(String, String)> {
        GalleryText::galleries().into_iter().map(|g| (g.section, g.block.text)).collect()
    }

    /// The root file of one gallery: the fence, then the fixtures of its row.
    fn assemble(fence: &str, fixtures: &[&str]) -> String {
        let mut text = fence.to_string();
        for model in fixtures {
            text.push('\n');
            text.push_str(&GalleryText::fixture(model));
        }
        text
    }

    fn fixtures_of(section: &str) -> &'static [&'static str] {
        TABLE.iter().find(|r| r.section == section).unwrap_or_else(|| panic!("a row for {section}")).fixtures
    }
}

/// Builds a design of a gallery and reads its diagnostics.
struct GalleryDesign;

impl GalleryDesign {
    /// The gallery built against the embedded library.
    fn build(text: &str) -> Design {
        Design::builder().source("top.msl", text).build()
    }

    /// The gallery built against a library whose prelude is empty.
    fn build_against_empty_prelude(text: &str) -> Design {
        Design::builder().std_source("lib.msl", "pub mod prelude {}").source("top.msl", text).build()
    }

    fn gallery_design(g: &Gallery) -> Design {
        GalleryDesign::build(&GalleryText::assemble(&g.block.text, GalleryText::fixtures_of(&g.section)))
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

    /// The gate fails unless it builds exactly 20 fences.
    fn require_20(built: usize) -> Result<(), String> {
        if built == 20 { Ok(()) } else { Err(format!("the gate built {built} gallery fences, not 20")) }
    }
}

/// A gallery fence with its section (`D.1` ...): the fences of Appendices D, E and F.
struct Gallery {
    section: String,
    block: SpecBlock,
}

#[test]
fn gallery_fixtures_exist() {
    let mut on_disk: Vec<String> = std::fs::read_dir(GalleryText::fixture_dir())
        .expect("tests/fixtures/gallery exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let mut want: Vec<String> = MODELS.iter().map(|m| format!("{m}.msl")).collect();
    want.sort();
    assert_eq!(on_disk, want, "one fixture file for each of the twelve models, and no other");
    for model in MODELS {
        assert!(GalleryText::defines(&GalleryText::fixture(model), model), "{model}.msl declares {model}");
    }
}

#[test]
fn every_fixture_line_is_spec_text() {
    let known = GalleryText::spec_lines();
    for model in MODELS {
        let foreign = GalleryText::foreign_lines(&GalleryText::fixture(model), &known);
        assert!(foreign.is_empty(), "{model}.msl has lines that are in no `rust` fence and no code span of Appendix D's table: {foreign:?}");
    }
    assert_eq!(GalleryText::foreign_lines("module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}\nmodule NotInTheSpec_x9() {}\n", &known), vec!["module NotInTheSpec_x9() {}".to_string()], "an invented line is named");
}

#[test]
fn the_fixture_table_covers_every_gallery() {
    let fences = GalleryText::galleries();
    assert_eq!(fences.len(), 20, "Appendices D, E and F hold 20 gallery fences (E.7 and the first three nets of F.4 run with `Resolve`; the typed bus of F.4.4 names no library item and stays a fragment, the Book listing digital#20 runs it)");
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
    assert_eq!(GalleryText::table_misuse(&TABLE, &GalleryText::fence_texts()), Vec::<(String, String)>::new());
    let bad = [
        Row { section: "D.3", fixtures: &["Vco"], std_items: &[] },
        Row { section: "D.1", fixtures: &["Tx"], std_items: &[] },
    ];
    assert_eq!(
        GalleryText::table_misuse(&bad, &GalleryText::fence_texts()),
        vec![("D.3".to_string(), "Vco".to_string()), ("D.1".to_string(), "Tx".to_string())],
        "a model the fence defines (Vco in D.3) and a model it does not use (Tx in D.1) are named as pairs"
    );
}

#[test]
fn every_gallery_builds_with_no_diagnostics() {
    let mut all = Vec::new();
    for g in GalleryText::galleries() {
        all.extend(GalleryDesign::diagnostic_lines(&GalleryDesign::gallery_design(&g), &g));
    }
    assert!(all.is_empty(), "{} diagnostics:\n{}", all.len(), all.join("\n"));
}

#[test]
fn every_gallery_passes_check() {
    for g in GalleryText::galleries() {
        let problems = GalleryDesign::gallery_design(&g).check();
        assert!(problems.is_empty(), "{}:{}: {problems:?}", g.block.file, g.block.line);
    }
}

#[test]
fn no_gallery_has_an_unresolved_member() {
    let mut all = Vec::new();
    for g in GalleryText::galleries() {
        all.extend(GalleryDesign::unresolved_members(&GalleryDesign::gallery_design(&g), &g));
    }
    assert!(all.is_empty(), "{}", all.join("\n"));
}

#[test]
fn every_gallery_references_its_std_items() {
    for g in GalleryText::galleries() {
        let row = TABLE.iter().find(|r| r.section == g.section).expect("a row");
        let missing = GalleryDesign::unreferenced_std_items(&GalleryDesign::gallery_design(&g), row.std_items);
        assert!(missing.is_empty(), "{} ({}:{}) does not reference the std items {missing:?}", g.section, g.block.file, g.block.line);
    }
}

#[test]
fn the_gate_builds_20_fences() {
    let fences = GalleryText::galleries();
    let built = fences.iter().map(|g| GalleryDesign::gallery_design(g)).count();
    assert_eq!(GalleryDesign::require_20(built), Ok(()));
    assert!(GalleryDesign::require_20(19).is_err() && GalleryDesign::require_20(21).is_err(), "another number fails the gate");
}

#[test]
fn a_dirty_gallery_is_reported_by_fence() {
    let g = Gallery {
        section: "D.1".to_string(),
        block: SpecBlock { file: "appendix_x.md".to_string(), line: 42, class: BlockClass::Valid, text: "module M(p: Electrical) {}\nconst X: f64 = NoSuchName;\n".to_string() },
    };
    let design = GalleryDesign::build(&g.block.text);
    let lines = GalleryDesign::diagnostic_lines(&design, &g);
    assert!(lines.iter().any(|l| l.starts_with("appendix_x.md:42: E0161 `NoSuchName`")), "{lines:?}");
    let g2 = Gallery {
        section: "D.1".to_string(),
        block: SpecBlock { file: "appendix_y.md".to_string(), line: 7, class: BlockClass::Valid, text: "module M(in c: Bit) { }\ndigital for M { on c.nosuchmethod() { } }\n".to_string() },
    };
    let unresolved = GalleryDesign::unresolved_members(&GalleryDesign::build(&g2.block.text), &g2);
    assert!(unresolved.iter().any(|l| l.starts_with("appendix_y.md:7: unresolved member") && l.contains("nosuchmethod")), "{unresolved:?}");
}

#[test]
fn galleries_fail_against_an_empty_prelude() {
    for g in GalleryText::galleries() {
        let design = GalleryDesign::build_against_empty_prelude(&GalleryText::assemble(&g.block.text, GalleryText::fixtures_of(&g.section)));
        let e0161 = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0161").count();
        assert!(e0161 >= 1, "{} ({}:{}) builds without E0161 against an empty prelude", g.section, g.block.file, g.block.line);
    }
}

#[test]
fn every_gallery_lists_a_std_item() {
    assert_eq!(TABLE.len(), 18);
    for row in &TABLE {
        assert!(!row.std_items.is_empty(), "{} lists no std item", row.section);
    }
}

/// The gallery fences that write a form `composite-values` lifted, with the text of each form: a `for` in
/// a reaction (E.3, phase A), a guarded arm of a `match` and the variants of an enum with a payload (E.4,
/// phases A and B) and the memory `Ram` of E.5 (phase C).
const LIFTED_FENCES: [(&str, &[&str]); 3] = [("E.3", &["for k in 1..N"]), ("E.4", &["_ if !baud_tick", "Tx::Data { n }"]), ("E.5", &["[Bit<u32>; 256]", "mem[addr] <- wdata"])];

/// How the type checker named the forms phases A and B lifted, before CV-01 to CV-22 (`types/unsupported.rs`).
const LIFTED_FORMS: [&str; 16] = [
    "assignment",
    "`while` loop",
    "`loop`",
    "`for` loop",
    "`let mut` binding",
    "a match arm guard",
    "range pattern",
    "struct expression",
    "field access",
    "tuple expression",
    "array expression",
    "index expression",
    "a variant with a payload",
    "`#[repr]`",
    "an array of `Bit",
    "an element or field commit",
];

/// The modules a source text declares.
struct ModuleNames;

impl ModuleNames {
    /// The modules `text` declares, by name.
    fn declared_in(text: &str) -> Vec<String> {
        text.lines()
            .filter_map(|line| line.strip_prefix("module "))
            .filter_map(|rest| rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next())
            .map(str::to_string)
            .collect()
    }
}

#[test]
fn the_fences_composite_values_lifted_are_checked() {
    // CV-37 AC 2: a gallery fence that writes a form of phase A or B is held to the type checker, each of its
    // modules as the top: none of its refusals names a form `composite-values` lifted. (E.3 is stopped by
    // its const generics first, `generics` L30; E.4 writes the guarded arm and the payload enum of section 2.)
    let mut held = Vec::new();
    for g in GalleryText::galleries() {
        let Some((_, forms)) = LIFTED_FENCES.iter().find(|(section, _)| *section == g.section) else { continue };
        for form in *forms {
            assert!(g.block.text.contains(form), "{}: the fence no longer writes `{form}`", g.section);
        }
        let text = GalleryText::assemble(&g.block.text, GalleryText::fixtures_of(&g.section));
        let design = GalleryDesign::build(&text);
        for module in ModuleNames::declared_in(&text) {
            let Err(refused) = piperine_lang::types::Types::check(&design, &module) else { continue };
            for diagnostic in refused {
                let message = diagnostic.diagnostic.message;
                for lifted in LIFTED_FORMS {
                    assert!(!message.contains(lifted), "{} as `{module}`: the checker still refuses {lifted}: {message}", g.section);
                }
            }
        }
        held.push(g.section);
    }
    assert_eq!(held, ["E.3", "E.4", "E.5"], "the three fences are held (E.5 is phase C: the array of carriers `Ram`; §8.2 and §22.1.3 are not gallery fences)");
}

/// The sections of the gallery fences that write a form of `generics` phase A: the nets of E.7 and F.4 (`resolved-nets`) write
/// an `impl Resolve` with its generic `resolve`; the list is the one to update, the day another fence writes one.
const EXPECTED_HELD: [&str; 4] = ["E.7", "F.4", "F.4", "F.4"];

/// What a gallery fence writes of the forms `generics` phase A lifted: a generic function, `impl`, a trait, a generic
/// struct or enum. A fence is held when its text writes one.
const GENERICS_FORMS: [&str; 5] = ["fn ", "impl ", "trait ", "struct ", "enum "];

/// How the type checker named the forms phase A lifted before GN-01 to GN-06 (`types/unsupported.rs`).
const GENERICS_LIFTED: [&str; 6] = ["generic function", "generic struct", "generic enum", "trait", "`impl`", "self parameter"];

/// The `<` that opens a generic list on a line that declares a function, struct or enum, or an `impl` or a trait.
struct GenericDeclarations;

impl GenericDeclarations {
    /// Whether `text` declares an item with a `<..>` list or writes an `impl` or a `trait`.
    fn written_in(text: &str) -> bool {
        text.lines().map(str::trim_start).any(|line| {
            let declares = GENERICS_FORMS.iter().any(|form| line.strip_prefix("pub ").unwrap_or(line).starts_with(form));
            let head = line.split(['{', '(']).next().unwrap_or(line);
            declares && (head.contains('<') || line.starts_with("impl ") || line.starts_with("trait "))
        })
    }
}

#[test]
fn the_fences_generics_lifted_are_checked() {
    // GN-31 AC 2: a gallery fence that writes a form of generics phase A (a generic function, `impl`, a trait, a generic
    // struct or enum) is held to the type checker, each of its modules as the top: none of its refusals names a form
    // `generics` lifted
    let mut held = Vec::new();
    for g in GalleryText::galleries() {
        if !GenericDeclarations::written_in(&g.block.text) {
            continue;
        }
        let text = GalleryText::assemble(&g.block.text, GalleryText::fixtures_of(&g.section));
        let design = GalleryDesign::build(&text);
        for module in ModuleNames::declared_in(&text) {
            let Err(refused) = piperine_lang::types::Types::check(&design, &module) else { continue };
            for diagnostic in refused {
                let message = diagnostic.diagnostic.message;
                for lifted in GENERICS_LIFTED {
                    assert!(!message.contains(lifted), "{} as `{module}`: the checker still refuses {lifted}: {message}", g.section);
                }
            }
        }
        held.push(g.section);
    }
    assert_eq!(held, EXPECTED_HELD, "the fences that write phase A forms are held");
}

/// The refusals of the seven fences that write a generic module: none. E.6's `Slice` (a generic bundle, `flip Stream<T>`) and F.3's
/// `MemoryBank` (the bundle `Cell`, its literal through the template `C: Fn(Cell)`) check clean (`resolved-nets-bundles`, GAPS L25).
const REMAINING_REFUSALS: [&str; 0] = [];

/// What a gallery fence writes of the forms `generics` phase C lifted: a generic module.
struct GenericModules;

impl GenericModules {
    /// Whether `text` declares a module with a `<..>` list: `module Dff<T: Repr>(..)`.
    fn written_in(text: &str) -> bool {
        text.lines().map(str::trim_start).any(|line| {
            let line = line.strip_prefix("pub ").unwrap_or(line);
            line.strip_prefix("module ").is_some_and(|rest| rest.split('(').next().is_some_and(|head| head.contains('<')))
        })
    }
}

#[test]
fn the_fences_generics_phase_c_lifted_are_checked() {
    // GN-31 AC 2: a gallery fence that writes a generic module (and, in it, a generation `for`) is held to the type checker,
    // each of its modules as the top: it checks clean, with no diagnostic at all (the generic "not supported yet" included), but for the bundle (L25) and the bundle literal, each `P0001` by name and pinned exactly
    let mut held = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    for g in GalleryText::galleries() {
        if !GenericModules::written_in(&g.block.text) {
            continue;
        }
        let text = GalleryText::assemble(&g.block.text, GalleryText::fixtures_of(&g.section));
        let design = GalleryDesign::build(&text);
        for module in ModuleNames::declared_in(&text) {
            if let Err(refused) = piperine_lang::types::Types::check(&design, &module) {
                refusals.extend(refused.iter().map(|diagnostic| format!("{} as `{module}`: {}: {}", g.section, diagnostic.diagnostic.code, diagnostic.diagnostic.message)));
            }
        }
        held.push(g.section);
    }
    assert_eq!(held, ["D.1", "E.1", "E.2", "E.3", "E.6", "F.2", "F.3"], "the fences that write a generic module are held");
    assert_eq!(refusals, REMAINING_REFUSALS, "every fence that writes a generic module checks clean");
}

/// PI-11 AC 3: each of the 20 gallery fences is the formatter's output for itself, so a reader
/// copies it into a file `piperine fmt --check` accepts.
#[test]
fn every_gallery_fence_is_in_the_formatters_form() {
    let fences = GalleryText::galleries();
    assert_eq!(fences.len(), 20, "the galleries hold 20 fences");
    for gallery in fences {
        let text = format!("{}\n", gallery.block.text.trim_end());
        let formatted = Formatter::format(&text).unwrap_or_else(|d| panic!("{} ({}:{}): {d:?}", gallery.section, gallery.block.file, gallery.block.line));
        assert_eq!(formatted, text, "{} ({}:{}) is not in the formatter's form", gallery.section, gallery.block.file, gallery.block.line);
    }
}

/// The items of L58's generics group, as the names the fences write them with.
const GENERIC_LIBRARY: [&str; 15] = ["Short", "since", "idt", "idtmod", "zi_nd", "laplace_nd", "noise_table", "Stable", "Setup", "Hold", "MinPulse", "inertial", "cast", "same", "TribufDelayed"];

#[test]
fn the_fences_of_the_generic_library_are_checked() {
    // GN-31 AC 2: every gallery fence that writes an item of the generic library is held to the checker, whole-model, with no
    // diagnostic: none of them is left refused for a form of the `generics` feature (T113 holds the items themselves)
    let fences: Vec<Gallery> = GalleryText::galleries().into_iter().filter(|g| GENERIC_LIBRARY.iter().any(|item| GalleryText::uses(&g.block.text, item))).collect();
    let used: BTreeSet<&str> = GENERIC_LIBRARY.iter().copied().filter(|item| fences.iter().any(|g| GalleryText::uses(&g.block.text, item))).collect();
    assert!(used.contains("idtmod"), "the fences write at least `idtmod` (D.3): {used:?}");
    for g in fences {
        let found = piperine_lang::types::Types::check_model(&GalleryDesign::gallery_design(&g)).err().unwrap_or_default();
        let lines: Vec<String> = found.iter().map(|d| format!("{} {}", d.diagnostic.code, d.diagnostic.message)).collect();
        assert!(lines.is_empty(), "{} ({}:{}) is held to the checker: {lines:?}", g.section, g.block.file, g.block.line);
    }
}
