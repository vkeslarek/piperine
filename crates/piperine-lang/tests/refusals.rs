//! Scope: the refusal owner of the front end (feature `code-clarity`, CC-07). `P0001` is worded in one
//! place, `Refusal::diagnostic`; the helpers are structs with methods.

use std::path::{Path, PathBuf};

use piperine_lang::msl::Span;
use piperine_lang::refusal::forms::{AnalogForm, ConstantForm, DigitalForm, GenericsForm, PatternForm, StructureForm, ValueForm};
use piperine_lang::refusal::{Refusal, RefusalOwner, RefusalRow, RefusedForm};

/// The sources of this crate, read as text.
struct Sources;

impl Sources {
    /// Every `.rs` file under `src/`, in name order.
    fn all() -> Vec<PathBuf> {
        let mut found = Vec::new();
        Self::collect(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        found.sort();
        found
    }

    fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
}

#[test]
fn p0001_is_worded_in_one_place() {
    let spelling = ["\"P000", "1\""].concat();
    let spelled: Vec<PathBuf> = Sources::all().into_iter().filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&spelling))).collect();
    assert_eq!(spelled.len(), 1, "the literal {spelling} is written in {spelled:?}; `Refusal::diagnostic` is its one place");
    assert!(spelled[0].ends_with("refusal.rs"), "the literal {spelling} is written in {spelled:?}, not in refusal.rs");
}

#[test]
fn p0001_does_not_name_a_milestone() {
    let diagnostic = Refusal::of(ValueForm::Assignment).diagnostic(Span::default());
    assert_eq!(diagnostic.message, "assignment is not supported yet");
    assert_eq!(diagnostic.primary.1, "not supported yet");
    let milestone = ["M1", " subset"].concat();
    let naming: Vec<PathBuf> = Sources::all().into_iter().filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&milestone))).collect();
    assert!(naming.is_empty(), "these sources still name the milestone `{milestone}`: {naming:?}");
}

/// The constructions of each refused form in the sources outside `refusal/`.
struct FormSites;

impl FormSites {
    /// The families of forms, as the enums are named.
    const FAMILIES: [&'static str; 7] = ["Value", "Pattern", "Analog", "Digital", "Structure", "Generics", "Constant"];

    /// Each form written in `text` (`ValueForm::Method`), in order of appearance.
    fn written_in(text: &str) -> Vec<String> {
        let mut written = Vec::new();
        for family in Self::FAMILIES {
            let prefix = format!("{family}Form::");
            for (start, _) in text.match_indices(&prefix) {
                let name: String = text[start + prefix.len()..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                written.push(format!("{prefix}{name}"));
            }
        }
        written
    }
}

#[test]
fn no_two_sites_build_the_same_refused_form() {
    let mut sites: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for path in Sources::all().into_iter().filter(|path| !path.components().any(|part| part.as_os_str() == "refusal")) {
        let text = std::fs::read_to_string(&path).expect("a source of this crate is readable");
        for form in FormSites::written_in(&text) {
            *sites.entry(form).or_default() += 1;
        }
    }
    let repeated: Vec<String> = sites.into_iter().filter(|(_, count)| *count > 1).map(|(form, count)| format!("{form} x{count}")).collect();
    assert!(repeated.is_empty(), "these forms are built at more than one site; give each one method of `RefusedForm` (refusal/repeated.rs): {repeated:?}");
}

#[test]
fn the_catch_all_refusal_files_are_gone() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(!source.join("types/unsupported.rs").exists(), "types/unsupported.rs is gone: its names live in refusal/syntax.rs");
    assert!(!source.join("elab/unlowered.rs").exists(), "elab/unlowered.rs is gone: its forms are rows of the refusal table");
}

/// The refusal table, read as one list of rows.
struct RefusalTable;

impl RefusalTable {
    /// Every form of every family, each paired with the row its family gives it, in declaration order.
    fn entries() -> Vec<(RefusedForm, &'static RefusalRow)> {
        let mut entries = Vec::new();
        entries.extend(ValueForm::ALL.iter().map(|form| (RefusedForm::Value(*form), form.row())));
        entries.extend(PatternForm::ALL.iter().map(|form| (RefusedForm::Pattern(*form), form.row())));
        entries.extend(AnalogForm::ALL.iter().map(|form| (RefusedForm::Analog(*form), form.row())));
        entries.extend(DigitalForm::ALL.iter().map(|form| (RefusedForm::Digital(*form), form.row())));
        entries.extend(StructureForm::ALL.iter().map(|form| (RefusedForm::Structure(*form), form.row())));
        entries.extend(GenericsForm::ALL.iter().map(|form| (RefusedForm::Generics(*form), form.row())));
        entries.extend(ConstantForm::ALL.iter().map(|form| (RefusedForm::Constant(*form), form.row())));
        entries
    }

    /// The number of variants the table's enums declare, read from `src/refusal/forms.rs`.
    fn declared_variants() -> usize {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/refusal/forms.rs")).expect("the table source is readable");
        let mut inside = false;
        let mut count = 0;
        for line in text.lines() {
            if line.starts_with("pub enum ") {
                inside = true;
            } else if line == "}" {
                inside = false;
            } else if inside && line.starts_with("    ") && line.ends_with(',') && !line.trim_start().starts_with("//") {
                count += 1;
            }
        }
        count
    }

    /// The repository's `GAPS.md`.
    fn gaps() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../GAPS.md")).expect("GAPS.md is readable")
    }
}

#[test]
fn every_refused_form_has_a_row() {
    let entries = RefusalTable::entries();
    assert_eq!(entries.len(), RefusalTable::declared_variants(), "a form of `refusal/forms.rs` is missing from its family's `ALL`");
    for (form, row) in &entries {
        assert_eq!(&row.form, form, "the row of {form:?} words another form: `ROWS` follows the declaration order of the enum");
        assert!(!row.text.is_empty(), "the row of {form:?} has no words");
    }
}

#[test]
fn every_gap_owner_names_a_gaps_row() {
    let gaps = RefusalTable::gaps();
    for (_, row) in RefusalTable::entries() {
        if let RefusalOwner::Gap(id) = row.owner {
            assert!(gaps.lines().any(|line| line.starts_with(&format!("| {id} |"))), "the form {:?} names `{id}` as its owner and GAPS.md has no such row", row.form);
        }
    }
}

#[test]
fn the_unswept_count_only_shrinks() {
    let swept = RefusalTable::entries().iter().filter(|(_, row)| row.owner == RefusalOwner::Unswept).count();
    let recorded: usize = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/unswept_count.txt")).expect("the count is recorded").trim().parse().expect("the count is a number");
    assert!(swept <= recorded, "{swept} forms are `Unswept`, the record holds {recorded}: a new form needs an owner (`Gap(id)`), the count never grows");
    assert_eq!(swept, recorded, "{swept} forms are `Unswept`: write {swept} in tests/support/unswept_count.txt (the count only shrinks)");
}
