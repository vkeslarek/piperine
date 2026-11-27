//! Scope: the structural guards over the specification pages (`docs/spec/*.md`, `mkdocs.yml`): every
//! error code is defined exactly once, the construct count of Appendix B holds, every method a fence
//! calls is catalogued, the library appendix is in the file map and the navigation, and the removed
//! forms' table carries a code and a replacement per row. The tests that pinned a sentence of a
//! page left with code-clarity T11: the specification is the authority, and the implementation is held
//! to it by the other targets.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[path = "support/spec_tables.rs"]
mod spec_tables;
use spec_tables::{section, table_rows};

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::msl::lexer::{Kw, Lexer, Punct, Token};

/// The pages of the specification and what the guards below read from them.
struct Specification;

impl Specification {
    /// The repository root, two levels above this crate.
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The text of `docs/spec/<file>`.
    fn page(file: &str) -> String {
        let path = Self::root().join("docs/spec").join(file);
        std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    /// Every `docs/spec/*.md` as (file name, text), in name order.
    fn every_page() -> Vec<(String, String)> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(Self::root().join("docs/spec"))
            .expect("docs/spec exists")
            .map(|entry| entry.expect("directory entry").path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
            .collect();
        files.sort();
        files
            .into_iter()
            .map(|path| {
                let name = path.file_name().expect("a file name").to_string_lossy().into_owned();
                let text = std::fs::read_to_string(&path).expect("spec file is UTF-8");
                (name, text)
            })
            .collect()
    }

    /// `text` with every run of whitespace (line wraps included) as one space.
    fn flat(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Appendix B's section B.3.6, the table of removed forms.
    fn removed_forms_section() -> String {
        section(&Self::page("appendix_b_grammar.md"), "### B.3.6")
    }

    /// The value library page, Appendix H.
    fn library_page() -> String {
        Self::page("appendix_h_library.md")
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

    /// The codes defined by a row whose first cell is exactly one code.
    fn defining_rows() -> BTreeMap<String, Vec<String>> {
        let mut defs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, text) in Specification::every_page() {
            for row in table_rows(&text) {
                let first = row[0].trim_matches('`');
                if Specification::codes_in(first) == [first.to_string()] && first.len() == 5 {
                    defs.entry(first.to_string()).or_default().push(name.clone());
                }
            }
        }
        defs
    }

    /// Names called as methods (`. name (`) and names declared (`fn name`) in
    /// every `rust` or `rust,fragment` fence, found by the crate's lexer.
    fn fence_names() -> (BTreeMap<String, String>, BTreeSet<String>) {
        let (mut called, mut declared) = (BTreeMap::new(), BTreeSet::new());
        // A `compile_fail` fence shows what is rejected; it catalogues nothing.
        for block in spec_corpus::corpus().into_iter().filter(|b| b.class != spec_corpus::Class::CompileFail) {
            let (tokens, _) = Lexer::lex(&block.text);
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
}

#[test]
fn every_error_code_is_defined_exactly_once() {
    // POL-08 AC 7.
    let defs = Specification::defining_rows();
    let twice: Vec<_> = defs.iter().filter(|(_, v)| v.len() > 1).collect();
    assert!(twice.is_empty(), "codes defined by more than one row: {twice:?}");
    let mut mentioned = BTreeSet::new();
    for (_, text) in Specification::every_page() {
        mentioned.extend(Specification::codes_in(&text));
    }
    let undefined: Vec<_> = mentioned.iter().filter(|c| !defs.contains_key(*c) && !RETIRED_CODES.contains(&c.as_str())).collect();
    assert!(undefined.is_empty(), "codes mentioned and never defined: {undefined:?}");
    // A retired code (§0.2: numbers are not reused) is named only where its retirement is stated.
    let flat_specs = Specification::every_page().iter().map(|(_, text)| Specification::flat(text)).collect::<Vec<_>>().join(" ");
    for code in RETIRED_CODES {
        assert!(!defs.contains_key(code), "{code} is retired and still has a defining row");
        assert!(flat_specs.contains(&format!("`{code}` is retired")), "the retirement of {code} is stated");
    }
}

/// The codes value semantics retired (Appendix G.71) and `E0701`, retired by generics T55 (`cast`'s `where` is its check): each is named by the Resolution that
/// retires it and by G.71, and by no defining row; `E0456`, retired by analog-operators (user decision
/// 2026-11-23, `simparam(name)` is an `Option<f64>`), is named by §14.4.6.
const RETIRED_CODES: [&str; 8] = ["E0102", "E0331", "E0150", "E0151", "E0906", "E0908", "E0701", "E0456"];

#[test]
fn b1_counts_60_constructs() {
    // POL-08 AC 8, amended by explicit-unknowns (replaces `b1_still_counts_65_constructs`): the
    // literal `Unknown`, the contextual keyword `resolve` and the method `level` leave, so the
    // count and the rows agree at 62 (60 since generics phase D: `m` and `dtemp` left), and the B.1.1 kinds add up to it.
    let sec = section(&Specification::page("appendix_b_grammar.md"), "## B.1 ");
    assert!(sec.contains("**60**"), "B.1 states 60");
    let rows = table_rows(&section(&sec, "### B.1.2"));
    assert_eq!(rows.len(), 60, "B.1.2 holds 60 rows");
    let last: usize = rows.last().unwrap()[0].parse().expect("row numbers are integers");
    assert_eq!(last, 60);
    let kinds: usize = table_rows(&section(&sec, "### B.1.1")).iter().filter_map(|r| r[2].parse::<usize>().ok()).sum();
    assert_eq!(kinds, 60, "the kinds of B.1.1 sum to the count");
    for gone in ["Unknown", "`resolve`", "`level`"] {
        assert!(!rows.iter().any(|r| r[1] == gone), "B.1.2 still lists {gone}");
    }
}

#[test]
fn every_method_called_in_a_spec_fence_is_catalogued() {
    // POL-02 AC 2: a called name is in Appendix H, a B.1 method, or declared
    // by a fence. The scan tokenizes with the crate's lexer.
    let (called, declared) = Specification::fence_names();
    let h = Specification::library_page();
    let mut known = BTreeSet::new();
    for group in ["## H.1", "## H.2", "## H.3", "## H.4", "## H.5", "## H.6", "## H.7"] {
        known.extend(Specification::first_cell_names(&section(&h, group)));
    }
    let b1 = section(&Specification::page("appendix_b_grammar.md"), "### B.1.2");
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
fn appendix_h_is_in_the_file_map_and_nav() {
    // POL-02 AC 4.
    let map = section(&Specification::page("index.md"), "### 0.8");
    assert!(map.contains("appendix_h_library.md"), "§0.8 lists the file");
    let nav = std::fs::read_to_string(Specification::root().join("mkdocs.yml")).expect("mkdocs.yml");
    let line = nav.lines().find(|l| l.contains("spec/appendix_h_library.md")).expect("nav entry");
    assert!(line.contains("H."), "{line}");
}

#[test]
fn b36_rows_carry_a_code_and_a_replacement() {
    // POL-01 AC 3: the code sits in the second cell, the replacement in the
    // third, and no first cell is a code (the parse-phase fixture guard of
    // `msl_errors` scrapes first cells).
    let rows = table_rows(&Specification::removed_forms_section());
    assert_eq!(rows.len(), 21);
    for row in rows {
        assert_eq!(row.len(), 3, "Removed | Reported as | Write instead: {row:?}");
        assert!(!row[0].trim_matches('`').starts_with('E'), "first cell is a code: {row:?}");
        let codes = Specification::codes_in(&row[1]);
        assert_eq!(codes.len(), 1, "one code in the second cell: {row:?}");
        assert!(["E0101", "E0900", "E0903", "E0905"].contains(&codes[0].as_str()), "{row:?}");
        assert!(!row[2].is_empty(), "a replacement: {row:?}");
    }
}
