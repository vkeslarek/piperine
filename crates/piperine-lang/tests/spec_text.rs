//! Scope: the text of the specification itself (`docs/spec/*.md`,
//! `mkdocs.yml`). Each test asserts one wording the msl-polish decisions fix
//! (POL-01 to POL-08, POL-13 AC 1, POL-15 AC 1) or one guard that ties the
//! pages together: every code is defined once, the construct count holds.
//! The helpers are plain functions; no macro builds a table or an assertion.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The text of `docs/spec/<file>`.
fn spec(file: &str) -> String {
    let path = root().join("docs/spec").join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every `docs/spec/*.md` as (file name, text), in name order.
fn all_specs() -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join("docs/spec"))
        .expect("docs/spec exists")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&p).expect("spec file is UTF-8");
            (name, text)
        })
        .collect()
}

fn heading_level(line: &str) -> usize {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if hashes > 0 && line[hashes..].starts_with(' ') { hashes } else { 0 }
}

/// The lines from the heading that starts with `heading` (for example
/// `### B.3.6`) up to the next heading of the same or a higher level.
/// Lines inside a code fence are never taken for headings.
fn section(text: &str, heading: &str) -> String {
    let mut out = String::new();
    let mut level = 0;
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let l = if fenced { 0 } else { heading_level(line) };
        if level == 0 {
            if l > 0 && line.starts_with(heading) {
                level = l;
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        if l > 0 && l <= level {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    assert!(level > 0, "no heading starts with `{heading}`");
    out
}

/// The body rows of every table of `section`: cells trimmed, header and
/// separator rows left out.
fn table_rows(section: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut header_seen = false;
    for line in section.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            header_seen = false;
            continue;
        }
        let cells: Vec<String> = t.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect();
        if cells.iter().all(|c| !c.is_empty() && c.bytes().all(|b| b == b'-' || b == b':')) {
            continue;
        }
        if !header_seen {
            header_seen = true;
            continue;
        }
        rows.push(cells);
    }
    rows
}

/// `E` and four digits, as a code the specification mentions.
fn codes_in(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= b.len() {
        let word_start = i == 0 || !b[i - 1].is_ascii_alphanumeric();
        if word_start
            && b[i] == b'E'
            && b[i + 1..i + 5].iter().all(u8::is_ascii_digit)
            && (i + 5 == b.len() || !b[i + 5].is_ascii_alphanumeric())
        {
            out.push(text[i..i + 5].to_string());
            i += 5;
        } else {
            i += 1;
        }
    }
    out
}

const REMOVED_FORMS: [&str; 17] = [
    "type aliases",
    "associated types",
    "`static` items",
    "trait-bound `where` clauses",
    "`impl Trait` types",
    "qualified paths",
    "restricted visibility",
    "loop labels",
    "`&self` and `&mut self` receivers",
    "`mut` and pattern parameters",
    "half-open range patterns",
    "a leading `::`",
    "inner attributes",
    "the never type",
    "raw-pointer types",
    "`union` items",
    "`extern` blocks",
];

fn b36() -> String {
    section(&spec("appendix_b_grammar.md"), "### B.3.6")
}

#[test]
fn b36_lists_every_removed_form() {
    // POL-01 AC 1: one row per removed form, and the list is stated complete.
    let sec = b36();
    let rows = table_rows(&sec);
    for form in REMOVED_FORMS {
        let hits = rows.iter().filter(|r| r[0].starts_with(form)).count();
        assert_eq!(hits, 1, "B.3.6 must hold exactly one row for `{form}`: {rows:?}");
    }
    assert_eq!(rows.len(), REMOVED_FORMS.len(), "B.3.6 holds no row beyond the 17 forms");
    assert!(sec.contains("complete list"), "B.3.6 must say its list is the complete list");
    for part in ["§1 rule 2", "§3.2", "A.1"] {
        assert!(sec.contains(part), "B.3.6 must name {part} beside the table");
    }
    // The two old rows no parser can reject move to a closing sentence.
    assert!(!rows.iter().any(|r| r[0].contains("struct-update") || r[0].contains("blocking")));
    assert!(sec.contains("§8.4") && sec.contains("§12.2"), "the closing sentence cites §8.4 and §12.2");
}

#[test]
fn b36_names_the_forms_taken_in() {
    // POL-01 AC 2.
    let sec = b36();
    let taken = sec.split("Taken from Rust").nth(1).expect("B.3.6 has a `Taken from Rust` list");
    assert!(taken.contains("function-pointer types"), "{taken}");
    assert!(taken.contains("fn(T, ..) -> U"), "{taken}");
    assert!(taken.contains("negative literal as a const generic argument"), "{taken}");
    assert!(taken.contains("Int< -1 >"), "{taken}");
}

#[test]
fn b36_rows_carry_a_code_and_a_replacement() {
    // POL-01 AC 3: the code sits in the second cell, the replacement in the
    // third, and no first cell is a code (the parse-phase fixture guard of
    // `msl_errors` scrapes first cells).
    let rows = table_rows(&b36());
    assert_eq!(rows.len(), 17);
    for row in rows {
        assert_eq!(row.len(), 3, "Removed | Reported as | Write instead: {row:?}");
        assert!(!row[0].trim_matches('`').starts_with('E'), "first cell is a code: {row:?}");
        let codes = codes_in(&row[1]);
        assert_eq!(codes.len(), 1, "one code in the second cell: {row:?}");
        assert!(["E0101", "E0900", "E0903", "E0905"].contains(&codes[0].as_str()), "{row:?}");
        assert!(!row[2].is_empty(), "a replacement: {row:?}");
    }
}

/// The codes defined by a row whose first cell is exactly one code.
fn defining_rows() -> BTreeMap<String, Vec<String>> {
    let mut defs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, text) in all_specs() {
        for row in table_rows(&text) {
            let first = row[0].trim_matches('`');
            if codes_in(first) == [first.to_string()] && first.len() == 5 {
                defs.entry(first.to_string()).or_default().push(name.clone());
            }
        }
    }
    defs
}

#[test]
fn every_error_code_is_defined_exactly_once() {
    // POL-08 AC 7.
    let defs = defining_rows();
    let twice: Vec<_> = defs.iter().filter(|(_, v)| v.len() > 1).collect();
    assert!(twice.is_empty(), "codes defined by more than one row: {twice:?}");
    let mut mentioned = BTreeSet::new();
    for (_, text) in all_specs() {
        mentioned.extend(codes_in(&text));
    }
    let undefined: Vec<_> = mentioned.iter().filter(|c| !defs.contains_key(*c)).collect();
    assert!(undefined.is_empty(), "codes mentioned and never defined: {undefined:?}");
}

#[test]
fn b1_still_counts_65_constructs() {
    // POL-08 AC 8: the count and the rows agree.
    let sec = section(&spec("appendix_b_grammar.md"), "## B.1 ");
    assert!(sec.contains("**65**"), "B.1 states 65");
    let rows = table_rows(&section(&sec, "### B.1.2"));
    assert_eq!(rows.len(), 65, "B.1.2 holds 65 rows");
    let last: usize = rows.last().unwrap()[0].parse().expect("row numbers are integers");
    assert_eq!(last, 65);
}

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::msl::lexer::{Kw, Punct, Token, lex};

/// Names called as methods (`. name (`) and names declared (`fn name`) in
/// every `rust` or `rust,fragment` fence, found by the crate's lexer.
fn fence_names() -> (BTreeMap<String, String>, BTreeSet<String>) {
    let (mut called, mut declared) = (BTreeMap::new(), BTreeSet::new());
    // A `compile_fail` fence shows what is rejected; it catalogues nothing.
    for block in spec_corpus::corpus().into_iter().filter(|b| b.class != spec_corpus::Class::CompileFail) {
        let (tokens, _) = lex(&block.text);
        for w in tokens.windows(3) {
            if let (Token::Punct(Punct::Dot), Token::Ident(n), Token::Punct(Punct::LParen)) = (&w[0].0, &w[1].0, &w[2].0) {
                called.entry(n.clone()).or_insert_with(|| format!("{}:{}", block.file, block.line));
            }
            if let (Token::Kw(Kw::Fn), Token::Ident(n)) = (&w[0].0, &w[1].0) {
                declared.insert(n.clone());
            }
        }
    }
    (called, declared)
}

/// The names in the first cell of every table row of `section`, without
/// backticks and without a trailing parameter list.
fn first_cell_names(section: &str) -> BTreeSet<String> {
    table_rows(section)
        .into_iter()
        .map(|r| r[0].trim_matches('`').split(['<', '(']).next().unwrap().to_string())
        .collect()
}

fn appendix_h() -> String {
    spec("appendix_h_library.md")
}

#[test]
fn appendix_h_has_every_group() {
    // POL-02 AC 1: the title, the normative mark, nine groups, ten adaptors.
    let h = appendix_h();
    assert!(h.starts_with("# Appendix H. Inherited library"), "title");
    assert!(h.contains("This appendix is normative."));
    for group in [
        "## H.1 Integer types",
        "## H.2 Float types",
        "## H.3 `bool`",
        "## H.4 `Option`",
        "## H.5 Arrays and slices",
        "## H.6 Ranges",
        "## H.7 Iterator adaptors",
        "## H.8 Traits",
        "## H.9 Types",
    ] {
        let sec = section(&h, group);
        assert!(!table_rows(&sec).is_empty(), "{group} holds a table");
    }
    let adaptors = first_cell_names(&section(&h, "## H.7"));
    for a in ["map", "filter", "fold", "sum", "count", "zip", "enumerate", "rev", "any", "all"] {
        assert!(adaptors.contains(a), "H.7 lists `{a}`: {adaptors:?}");
    }
    assert_eq!(adaptors.len(), 10, "exactly the ten adaptors: {adaptors:?}");
}

#[test]
fn every_method_called_in_a_spec_fence_is_catalogued() {
    // POL-02 AC 2: a called name is in Appendix H, a B.1 method, or declared
    // by a fence. The scan tokenizes with the crate's lexer.
    let (called, declared) = fence_names();
    let h = appendix_h();
    let mut known = BTreeSet::new();
    for group in ["## H.1", "## H.2", "## H.3", "## H.4", "## H.5", "## H.6", "## H.7"] {
        known.extend(first_cell_names(&section(&h, group)));
    }
    let b1 = section(&spec("appendix_b_grammar.md"), "### B.1.2");
    for row in table_rows(&b1) {
        if row[2].contains("method") {
            known.insert(row[1].trim_matches('`').to_string());
        }
    }
    let missing: Vec<_> = called.iter().filter(|(n, _)| !known.contains(*n) && !declared.contains(*n)).collect();
    assert!(missing.is_empty(), "methods called in a fence and listed nowhere: {missing:?}");
    assert!(called.len() >= 20, "the scan found the fences' methods: {called:?}");
}

#[test]
fn catalogue_is_closed_by_e0161() {
    // POL-02 AC 3: §0.4, Appendix H and the pointers from Part I and A.1.
    let h = appendix_h();
    assert!(h.contains("does not resolve (`E0161`)") && h.contains("The catalogue is closed."));
    let s04 = section(&spec("index.md"), "### 0.4");
    assert!(s04.contains("Appendix H") && s04.contains("closed") && s04.contains("`E0161`"), "{s04}");
    let rule1 = spec("part_i_values.md");
    let rule1 = rule1.split("- **§1 rule 1.**").nth(1).unwrap().split("- **§1 rule 2.**").next().unwrap();
    assert!(rule1.contains("Appendix H"), "§1 rule 1 points at Appendix H: {rule1}");
    let a1 = spec("appendix_a_lexical.md");
    let derive = a1.lines().find(|l| l.starts_with("| `#[derive(")).expect("the derive row of A.1");
    assert!(derive.contains("Appendix H"), "{derive}");
    assert!(!derive.contains("other built-in derives"), "the open-ended list is gone: {derive}");
}

#[test]
fn appendix_h_is_in_the_file_map_and_nav() {
    // POL-02 AC 4.
    let map = section(&spec("index.md"), "### 0.8");
    assert!(map.contains("appendix_h_library.md"), "§0.8 lists the file");
    let nav = std::fs::read_to_string(root().join("mkdocs.yml")).expect("mkdocs.yml");
    let line = nav.lines().find(|l| l.contains("spec/appendix_h_library.md")).expect("nav entry");
    assert!(line.contains("H."), "{line}");
}

#[test]
fn gnd_and_the_implicit_params_are_core_names() {
    // POL-03 AC 1: §6 rule 11, A.1 rule 8 and §3.4.2 say `core name` and
    // name all five; no sentence calls `gnd` a prelude name.
    let five = ["`gnd`", "`Nature`", "`m`", "`dtemp`", "`str`"];
    let carriers = spec("part_ii_carriers.md");
    let rule11 = carriers.split("- **§6 rule 11.**").nth(1).unwrap().split("\n\n").next().unwrap();
    assert!(rule11.contains("`gnd` is a core name"), "{rule11}");
    let after = carriers.split("- **§6 rule 11.**").nth(1).unwrap();
    let note = after.split("> **Resolution.**").nth(1).unwrap().split("\n\n").next().unwrap();
    for n in five {
        assert!(note.contains(n), "the Resolution of §6 names {n}: {note}");
    }
    assert!(note.contains("no declaration in the language can define them"), "{note}");
    let a1 = spec("appendix_a_lexical.md");
    let rule8 = a1.split("\n8. ").nth(1).unwrap().split("\n\n").next().unwrap();
    assert!(rule8.contains("core names"), "{rule8}");
    for n in five {
        assert!(rule8.contains(n), "A.1 rule 8 names {n}: {rule8}");
    }
    let values = spec("part_i_values.md");
    let r342 = values.split("- **§3.4.2**").nth(1).unwrap().split("- **§3.4.3**").next().unwrap();
    assert!(r342.contains("core names"), "{r342}");
    for n in five {
        assert!(r342.contains(n), "§3.4.2 names {n}: {r342}");
    }
    for (file, text) in all_specs() {
        for sentence in text.split(". ") {
            let flat = sentence.replace('\n', " ");
            assert!(
                !(flat.contains("`gnd`") && flat.contains("prelude name")),
                "{file} calls gnd a prelude name: {flat}"
            );
        }
    }
}
