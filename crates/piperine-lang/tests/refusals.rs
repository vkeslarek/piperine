//! Scope: the refusal owner of the front end (feature `code-clarity`, CC-07). `P0001` is worded in one
//! place, `Refusal::diagnostic`; the helpers are structs with methods.

use std::path::{Path, PathBuf};

use piperine_lang::msl::Span;
use piperine_lang::refusal::Refusal;
use piperine_lang::refusal::forms::ValueForm;

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
