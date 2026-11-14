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
