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
    assert_eq!(constructs.len(), 60, "B.1.2 lists 60 constructs (explicit-unknowns removed `resolve`, `Unknown` and `level`; generics phase D removed `m` and `dtemp`): {constructs:?}");
    assert_eq!(constructs.iter().map(|(n, _)| *n).collect::<Vec<_>>(), (1..=60).collect::<Vec<_>>());
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
    assert_eq!(rows.iter().filter(|r| r.first().is_some_and(|c| c.parse::<usize>().is_ok())).count(), 60, "one row per construct, no extra");
}

/// The table rows (`| .. |` lines) of the `## heading` section of a page, as trimmed cells.
fn section_rows(page: &str, heading: &str) -> Vec<Vec<String>> {
    page_section(page, heading).into_iter().filter(|l| l.starts_with('|')).map(|l| l.split('|').map(|c| c.trim().to_string()).skip(1).collect()).collect()
}

fn page_of(name: &str) -> String {
    pom_docs().into_iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("docs/pom/{name}")).1
}

#[test]
fn place_has_no_parenthesized_form_in_the_docs() {
    // B.3.2 note 4: a `place` takes no parentheses.
    let mapping = page_of("mapping.md");
    let row = mapping.lines().find(|l| l.starts_with("| `place` |")).expect("a `place` row in mapping.md");
    assert!(!row.contains("(s)") && !row.contains("Paren"), "the `place` row still has a parenthesized form: {row}");
    let reflection = page_of("reflection.md");
    let writers = reflection.lines().find(|l| l.starts_with("| `writers(self)` |")).expect("the `Signal::writers` row");
    assert!(!writers.contains("(s)"), "`Signal::writers` still names the parenthesized place: {writers}");
}

#[test]
fn every_code_the_pom_reports_is_documented() {
    let mut codes = std::collections::BTreeSet::new();
    for file in rust_files(&repo().join("crates/piperine-lang/src/pom")) {
        let text = std::fs::read_to_string(file).unwrap();
        let bytes = text.as_bytes();
        // Every string literal `"E` followed by four digits and a closing quote: a code, whether it is
        // passed to `Diagnostic::new` directly or through a helper.
        for (at, _) in text.match_indices("\"E") {
            let code = &text[at + 1..(at + 6).min(text.len())];
            if code.len() == 5 && code[1..].bytes().all(|b| b.is_ascii_digit()) && bytes.get(at + 6) == Some(&b'"') {
                codes.insert(code.to_string());
            }
        }
    }
    assert!(codes.len() >= 17 && codes.contains("E0125") && codes.contains("E0263"), "the scrape found only {codes:?}");
    let rows = section_rows(&page_of("resolution.md"), "## Diagnostics");
    let documented: std::collections::BTreeSet<String> = rows.iter().filter_map(|r| r.first()).map(|c| c.trim_matches('`').to_string()).collect();
    let missing: Vec<_> = codes.difference(&documented).collect();
    assert!(missing.is_empty(), "the Diagnostics table of resolution.md has no row for: {missing:?}");
    // POL-22 AC 3: each row names the spec section that defines its code, and
    // that is the section the spec defines it in.
    let header = &rows[0];
    let col = header.iter().position(|c| c == "Defined in").expect("a `Defined in` column");
    let defined = spec_definitions();
    for row in rows.iter().skip(2).filter(|r| r.first().is_some_and(|c| c.starts_with("`E"))) {
        let code = row[0].trim_matches('`');
        let cell = row[col].trim_matches('`');
        let section = cell.strip_prefix('§').unwrap_or_else(|| panic!("{code}: `Defined in` is `{cell}`, not a `§` section"));
        assert_eq!(defined.get(code).map(String::as_str), Some(section), "{code}: the spec defines it in {:?}", defined.get(code));
    }
}

/// The number of the section of `docs/spec` whose table defines each code (the
/// code is the first cell of a row), as in `3.4` for `### 3.4 Files and imports`.
fn spec_definitions() -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let dir = repo().join("docs/spec");
    for entry in std::fs::read_dir(dir).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        let mut section = String::new();
        for line in text.lines() {
            if let Some(h) = line.strip_prefix('#') {
                let title = h.trim_start_matches('#').trim();
                let number = title.split_whitespace().next().unwrap_or("").trim_end_matches('.');
                if number.starts_with(|c: char| c.is_ascii_digit()) {
                    section = number.to_string();
                }
            } else if let Some(rest) = line.strip_prefix("| `E") {
                if let Some(code) = rest.get(..4).filter(|d| d.bytes().all(|b| b.is_ascii_digit())) {
                    out.entry(format!("E{code}")).or_insert_with(|| section.clone());
                }
            }
        }
    }
    out
}

/// The `(name, kind)` of every `row(..)` of `BUILTINS` in `src/pom/builtin.rs`.
fn builtin_rows() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(repo().join("crates/piperine-lang/src/pom/builtin.rs")).unwrap();
    text.lines()
        .filter_map(|l| {
            let rest = l.trim_start().strip_prefix("row(\"")?;
            let (name, rest) = rest.split_once('"')?;
            let kind = rest.split("BuiltinKind::").nth(1)?.split(|c: char| !c.is_alphanumeric()).next()?;
            Some((name.to_string(), kind.to_string()))
        })
        .collect()
}

#[test]
fn every_builtin_row_is_documented() {
    let rows = builtin_rows();
    assert!(rows.len() >= 59 && rows.iter().any(|(n, k)| n == "ddt" && k == "Module"), "the scrape found only {rows:?}");
    let table = section_rows(&page_of("resolution.md"), "## The built-in table");
    let mut missing = Vec::new();
    for (name, kind) in &rows {
        let named = table.iter().any(|r| r.first().is_some_and(|c| c.as_str() == format!("`{kind}`")) && r.get(1).is_some_and(|c| c.contains(&format!("`{name}`"))));
        if !named {
            missing.push(format!("{kind} {name}"));
        }
    }
    assert!(missing.is_empty(), "the built-in table of resolution.md has no entry for: {missing:?}");
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

#[test]
fn docs_describe_crates_and_no_prelude_sources() {
    // PKG-09 AC 1: the scope order names the dependency names and the `std` prelude, and no page
    // talks about prelude sources.
    for (name, text) in pom_docs() {
        let lower = text.to_lowercase();
        assert!(!lower.contains("prelude source"), "{name} mentions prelude sources");
        assert!(!text.contains(".prelude("), "{name} mentions `.prelude(`");
    }
    let resolution = page_of("resolution.md");
    let order = page_section(&resolution, "## Scope order").join("\n");
    assert!(order.contains("dependency names"), "the scope order does not name the dependency names");
    assert!(order.contains("`std::prelude`"), "the scope order does not name the `std` prelude");
    let dependencies = order.find("dependency names").unwrap();
    let prelude = order.find("`std::prelude`").unwrap();
    assert!(dependencies < prelude, "dependency names come before the prelude in the scope order");
    assert!(resolution.contains("### Crates and the standard library"), "resolution.md lacks the crates section");
    assert!(!resolution.contains("### Imports and the prelude"), "resolution.md still has the section this replaced");
    assert!(page_of("index.md").contains(".crates("), "the build line of the overview does not show `.crates(graph)`");
    assert!(page_of("model.md").lines().any(|l| l.trim_end() == "## Crates"), "model.md lacks a `## Crates` storage section");
}

#[test]
fn mapping_has_a_crate_row() {
    // PKG-09 AC 2: a row maps a crate to `CrateData`, `Crate` and the lookups that reach it.
    let mapping = page_of("mapping.md");
    let row = mapping.lines().find(|l| l.starts_with("| `crate` |")).expect("a `crate` row in mapping.md");
    for needed in ["`CrateData`", "`Crate`", "design.krate(", "design.crates()", "Crate::dependency"] {
        assert!(row.contains(needed), "the `crate` row lacks {needed}: {row}");
    }
}

#[test]
fn resolution_doc_describes_intrinsics() {
    // STD-17 AC 2: the page names `extern "intrinsic"` and `is_intrinsic` where it describes E0125.
    let page = page_of("resolution.md");
    let row = page.lines().find(|l| l.starts_with("| `E0125` |")).expect("the E0125 row");
    assert!(row.contains("extern \"intrinsic\"") && row.contains("`std`"), "{row}");
    let shape = page.lines().find(|l| l.contains("is_intrinsic")).expect("a line naming `is_intrinsic`");
    assert!(shape.contains("extern \"intrinsic\""), "{shape}");
}

#[test]
fn resolution_doc_describes_the_supertrait_rule() {
    // STD-17 AC 3: `Self::NAME` in a trait resolves through the supertraits.
    let page = page_of("resolution.md");
    let line = page.lines().find(|l| l.contains("Self::NAME") && l.contains("supertrait")).expect("a line on `Self::NAME` and supertraits");
    assert!(line.contains("trait"), "{line}");
}

#[test]
fn members_doc_lists_the_blanket_step() {
    // STD-17 AC 4: the Members table ends the carrier row with the blanket implementation.
    let page = page_of("resolution.md");
    let row = page.lines().find(|l| l.starts_with("| a carrier of a nature")).expect("the carrier row of the Members table");
    assert!(row.contains("blanket") && row.contains("impl<T: Repr>"), "{row}");
    assert!(row.rfind("blanket") > row.rfind("value type"), "the blanket step comes last: {row}");
    let rule = page.lines().find(|l| l.contains("blanket") && l.contains("Repr") && !l.starts_with('|')).expect("a sentence on the blanket step");
    assert!(rule.contains("not searched") || rule.contains("other bound"), "{rule}");
}

#[test]
fn the_builtin_table_doc_has_no_const_row() {
    // STD-17 AC 1: the table has no `Const` row and the page says where the constants went.
    let page = page_of("resolution.md");
    let table = section_rows(&page, "## The built-in table");
    assert!(!table.is_empty(), "the built-in table is found");
    assert!(table.iter().all(|r| r.first().is_none_or(|c| c.as_str() != "`Const`")), "no `Const` row");
    let sentence = page.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(sentence.contains("`std::constants`") && sentence.contains("six constants"), "the page says the six constants are items of `std::constants`");
    let model = page_of("model.md");
    let kinds = section_rows(&model, "### `BuiltinKind`");
    assert!(kinds.iter().all(|r| r.first().is_none_or(|c| c.as_str() != "`Const`")), "BuiltinKind has no `Const` variant row");
}

/// One statement of `docs/pom/` that describes a form the implementation still has, although the
/// specification no longer has it (explicit-unknowns, D7): the page, the text that says it, the source file
/// and the text that keeps it alive, and the task that removes both.
struct DocAheadOfTheCode {
    /// The page of `docs/pom/` that holds the statement.
    page: &'static str,
    /// The text of the statement, as written on the page.
    statement: &'static str,
    /// The source file, relative to the repository root, that still has the form.
    source: &'static str,
    /// The text of the source that is the form.
    form: &'static str,
    /// The task that removes the form from the code and rewrites the page.
    owner: &'static str,
}

impl DocAheadOfTheCode {
    /// The statements that wait for their task. When a task removes a form the entry fails here, naming it.
    const WAITING: [DocAheadOfTheCode; 0] = [];

    /// The offences of this statement: the page no longer says it, or the source no longer has the form.
    /// Either way the entry is stale and its owner has delivered.
    fn offences(&self) -> Vec<String> {
        let mut found = Vec::new();
        if !page_of(self.page).contains(self.statement) {
            found.push(format!("docs/pom/{} no longer says {}: drop the entry (owner {})", self.page, self.statement, self.owner));
        }
        let source = std::fs::read_to_string(repo().join(self.source)).unwrap_or_else(|e| panic!("{}: {e}", self.source));
        if !source.contains(self.form) {
            found.push(format!("{} no longer has `{}`: rewrite docs/pom/{} and drop the entry (owner {})", self.source, self.form, self.page, self.owner));
        }
        found
    }
}

#[test]
fn the_pom_docs_that_wait_for_the_code_stay_honest() {
    // docs/pom describes the implementation, which still has the `resolve` field
    // that the specification removed; each statement is owned by the task that removes its form,
    // and a stale entry (the form gone or the page rewritten) fails here instead of hiding a divergence.
    let found: Vec<String> = DocAheadOfTheCode::WAITING.iter().flat_map(DocAheadOfTheCode::offences).collect();
    assert!(found.is_empty(), "{found:#?}");
    let note = page_of("mapping.md");
    assert!(note.contains("`resolved-nets` T16"), "the mapping note names the owners of the forms it keeps");
}
