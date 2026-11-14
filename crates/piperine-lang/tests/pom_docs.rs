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

/// The names of the productions of Appendix B.3.2: the `name ::=` lines of its code block.
fn b32_productions() -> Vec<String> {
    let text = std::fs::read_to_string(repo().join("docs/spec/appendix_b_grammar.md")).unwrap();
    let start = text.find("### B.3.2").expect("B.3.2 heading");
    let end = text[start..].find("### B.3.3").map_or(text.len(), |i| start + i);
    text[start..end]
        .lines()
        .filter_map(|l| {
            let (name, rest) = l.split_once(char::is_whitespace)?;
            (rest.trim_start().starts_with("::=") && !name.starts_with("//")).then(|| name.to_string())
        })
        .collect()
}

#[test]
fn every_b3_production_is_mapped() {
    let productions = b32_productions();
    assert!(productions.len() >= 30, "the scrape found only {productions:?}");
    for known in ["nature_decl", "module_decl", "instance", "reaction", "gen<X>", "attr_link"] {
        assert!(productions.iter().any(|p| p == known), "{known} is read from B.3.2: {productions:?}");
    }
    let docs = pom_docs();
    let page = &docs.iter().find(|(n, _)| n == "mapping.md").expect("docs/pom/mapping.md").1;
    let mut types = Vec::new();
    for file in rust_files(&repo().join("crates/piperine-lang/src/pom")) {
        let text = std::fs::read_to_string(file).unwrap();
        types.extend(text.lines().filter_map(public_type_name).map(str::to_string));
    }
    let rows: Vec<&str> = page.lines().filter(|l| l.starts_with('|')).collect();
    let mut unmapped = Vec::new();
    for p in &productions {
        let key = format!("`{p}`");
        let names_a_type = |row: &&str| {
            row.split('`').skip(1).step_by(2).any(|code| {
                let ident: String = code.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                types.iter().any(|t| *t == ident)
            })
        };
        // The production sits in the first cell; a type is named in the same row.
        let mapped = rows.iter().any(|r| r.split('|').nth(1).is_some_and(|cell| cell.contains(&key)) && names_a_type(r));
        if !mapped {
            unmapped.push(p.clone());
        }
    }
    assert!(unmapped.is_empty(), "no row of docs/pom/mapping.md maps: {unmapped:?}");
}

/// The rows `(number, construct)` of Appendix B.1.2: the lines `| N | construct | ..` of its table.
fn b12_constructs() -> Vec<(usize, String)> {
    let text = std::fs::read_to_string(repo().join("docs/spec/appendix_b_grammar.md")).unwrap();
    let start = text.find("### B.1.2").expect("B.1.2 heading");
    let end = text[start + 1..].find("\n##").map_or(text.len(), |i| start + 1 + i);
    text[start..end]
        .lines()
        .filter_map(|l| {
            let mut cells = l.split('|').map(str::trim).skip(1);
            let number: usize = cells.next()?.parse().ok()?;
            Some((number, cells.next()?.to_string()))
        })
        .collect()
}

/// The names of every `pub struct|enum|trait` and every `pub fn` under `src/pom`, and in
/// `src/msl/ast.rs` (the literal and operator types the POM re-exports).
fn pom_public_names() -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut files = rust_files(&repo().join("crates/piperine-lang/src/pom"));
    files.push(repo().join("crates/piperine-lang/src/msl/ast.rs"));
    for file in files {
        for line in std::fs::read_to_string(file).unwrap().lines() {
            if let Some(t) = public_type_name(line) {
                names.insert(t.to_string());
            }
            if let Some(rest) = line.trim_start().strip_prefix("pub fn ").or_else(|| line.trim_start().strip_prefix("pub const fn ")) {
                names.insert(rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect());
            }
        }
    }
    names
}

/// The lines of the `## heading` section of a page.
fn page_section<'a>(page: &'a str, heading: &str) -> Vec<&'a str> {
    let mut lines = page.lines().skip_while(|l| l.trim_end() != heading);
    lines.next();
    lines.take_while(|l| !l.starts_with("## ")).collect()
}

#[test]
fn every_b1_construct_is_mapped() {
    let constructs = b12_constructs();
    assert_eq!(constructs.len(), 65, "B.1.2 lists 65 constructs: {constructs:?}");
    assert_eq!(constructs.iter().map(|(n, _)| *n).collect::<Vec<_>>(), (1..=65).collect::<Vec<_>>());
    let docs = pom_docs();
    let page = &docs.iter().find(|(n, _)| n == "mapping.md").expect("docs/pom/mapping.md").1;
    let section = page_section(page, "## Constructs of B.1");
    assert!(!section.is_empty(), "mapping.md has a `## Constructs of B.1` section");
    let rows: Vec<Vec<&str>> = section.iter().filter(|l| l.starts_with('|')).map(|l| l.split('|').map(str::trim).skip(1).collect()).collect();
    let names = pom_public_names();
    let mut unmapped = Vec::new();
    for (number, construct) in &constructs {
        let row = rows.iter().find(|r| r.first().and_then(|c| c.parse::<usize>().ok()) == Some(*number));
        // The row repeats the construct as B.1.2 writes it and names a POM type, method or function.
        let names_pom = |r: &Vec<&str>| {
            r.get(2).is_some_and(|cell| {
                cell.split('`').skip(1).step_by(2).any(|code| code.split(|c: char| !(c.is_alphanumeric() || c == '_')).any(|ident| names.contains(ident)))
            })
        };
        if !row.is_some_and(|r| r.get(1).copied() == Some(construct.as_str()) && names_pom(r)) {
            unmapped.push(format!("{number} {construct}"));
        }
    }
    assert!(unmapped.is_empty(), "no row of the `Constructs of B.1` table maps: {unmapped:?}");
    assert_eq!(rows.iter().filter(|r| r.first().is_some_and(|c| c.parse::<usize>().is_ok())).count(), 65, "one row per construct, no extra");
}

#[test]
fn msl_fences_parse_clean() {
    let mut fences = 0;
    for (file, text) in pom_docs() {
        let mut open: Option<(usize, String)> = None;
        for (i, line) in text.lines().enumerate() {
            match &mut open {
                None => {
                    if line.trim_start() == "```msl" {
                        open = Some((i + 1, String::new()));
                    }
                }
                Some((at, body)) => {
                    if line.trim_start().starts_with("```") {
                        fences += 1;
                        let parsed = piperine_lang::msl::parser::parse(body);
                        assert!(parsed.diagnostics.is_empty(), "{file}:{at}: msl fence does not parse: {:?}", parsed.diagnostics);
                        open = None;
                    } else {
                        body.push_str(line);
                        body.push('\n');
                    }
                }
            }
        }
        assert!(open.is_none(), "{file}: an msl fence is never closed");
    }
    assert!(fences >= 10, "docs/pom holds only {fences} msl fences");
}

#[test]
fn mkdocs_nav_lists_every_pom_page() {
    let yml = std::fs::read_to_string(repo().join("mkdocs.yml")).unwrap();
    let nav = &yml[yml.find("\nnav:").expect("mkdocs.yml has a nav") + 1..];
    assert!(nav.contains("Object model"), "the nav has an `Object model` section");
    let pages = pom_docs();
    assert!(pages.len() >= 5, "docs/pom holds {} pages", pages.len());
    for (file, _) in pages {
        assert!(nav.lines().any(|l| l.trim_end().ends_with(&format!("pom/{file}"))), "mkdocs.yml nav does not list docs/pom/{file}");
    }
}
