//! Scope: the reference documentation of the object model (POM-13): `docs/pom/`
//! names every public type and every reflection method, maps every production of
//! Appendix B.3.2, holds only MSL examples that parse, and is listed in the
//! `mkdocs.yml` navigation. The executed `rust` examples are doctests of
//! `piperine_lang::pom::guide`, not tests of this target.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `docs/pom/*.md` as (file name, text), sorted by name.
fn pom_docs() -> Vec<(String, String)> {
    let dir = repo().join("docs/pom");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} cannot be read: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files.iter().map(|p| (p.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(p).unwrap())).collect()
}

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// The name declared by a line `pub struct|enum|trait Name ..` (not `pub(crate)`), if the line is one.
fn public_type_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("pub ")?;
    let rest = ["struct ", "enum ", "trait "].iter().find_map(|k| rest.strip_prefix(k))?;
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

#[test]
fn every_public_pom_type_has_a_heading() {
    let docs = pom_docs();
    let headings: Vec<&str> = docs.iter().flat_map(|(_, t)| t.lines()).filter(|l| l.starts_with("### `")).collect();
    let mut checked = 0;
    let mut missing = Vec::new();
    for file in rust_files(&repo().join("crates/piperine-lang/src/pom")) {
        let text = std::fs::read_to_string(&file).unwrap();
        for name in text.lines().filter_map(public_type_name) {
            checked += 1;
            if !headings.iter().any(|h| h.starts_with(&format!("### `{name}`"))) {
                missing.push(format!("{name} ({})", file.file_name().unwrap().to_string_lossy()));
            }
        }
    }
    assert!(checked > 100, "the scrape found only {checked} public types");
    assert!(missing.is_empty(), "no `### `Name`` heading in docs/pom/ for: {missing:?}");
}

/// The `pub fn` methods of the handles under `src/pom/reflect/`, as (owner type, method): every
/// `pub fn` of an `impl X` block, and every `fn` of `pub trait X`.
fn reflect_methods() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for file in rust_files(&repo().join("crates/piperine-lang/src/pom/reflect")) {
        let text = std::fs::read_to_string(&file).unwrap();
        let mut owner: Option<(String, bool)> = None; // (type, inside a trait)
        for line in text.lines() {
            if !line.starts_with(' ') && !line.is_empty() {
                owner = None;
                let words: Vec<&str> = line.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
                if line.starts_with("impl") && !line.contains(" for ") {
                    // `impl<'d> Name<'d> {` or `impl Name {`
                    if let Some(name) = words.iter().skip(1).find(|w| w.chars().next().is_some_and(char::is_uppercase)) {
                        owner = Some((name.to_string(), false));
                    }
                } else if let Some(rest) = line.strip_prefix("pub trait ") {
                    owner = Some((rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap().to_string(), true));
                }
            } else if let Some((ty, in_trait)) = &owner {
                let t = line.trim_start();
                let rest = if *in_trait { t.strip_prefix("fn ").or_else(|| t.strip_prefix("pub fn ")) } else { t.strip_prefix("pub fn ") };
                if let (Some(rest), true) = (rest, line.starts_with("    ") && !line.starts_with("     ")) {
                    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                    out.push((ty.clone(), name));
                }
            }
        }
    }
    out
}

/// The text of the `### `Name`` section of `reflection.md` (up to the next `### ` heading).
fn section<'a>(page: &'a str, name: &str) -> &'a str {
    let head = format!("### `{name}`");
    let Some(start) = page.lines().scan(0usize, |at, l| { let s = *at; *at += l.len() + 1; Some((s, l)) }).find(|(_, l)| l.starts_with(&head)).map(|(s, _)| s) else {
        return "";
    };
    let rest = &page[start..];
    let end = rest[1..].find("\n### ").map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

#[test]
fn every_reflection_method_is_documented() {
    let docs = pom_docs();
    let page = &docs.iter().find(|(n, _)| n == "reflection.md").expect("docs/pom/reflection.md").1;
    let methods = reflect_methods();
    assert!(methods.len() > 200, "the scrape found only {} methods", methods.len());
    let mut missing = Vec::new();
    for (owner, name) in &methods {
        if !page.contains(&format!("`{name}(")) || !section(page, owner).contains(&format!("`{name}(")) {
            missing.push(format!("{owner}::{name}"));
        }
    }
    assert!(missing.is_empty(), "not mentioned as `name(` under its `### `Type`` heading of reflection.md: {missing:?}");
}
