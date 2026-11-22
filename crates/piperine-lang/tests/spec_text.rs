//! Scope: the text of the specification itself (`docs/spec/*.md`,
//! `mkdocs.yml`). Each test asserts one wording the msl-polish decisions fix
//! (POL-01 to POL-08, POL-13 AC 1, POL-15 AC 1) or one guard that ties the
//! pages together: every code is defined once, the construct count holds.
//! The helpers are plain functions; no macro builds a table or an assertion.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[path = "support/spec_tables.rs"]
mod spec_tables;
use spec_tables::{section, table_rows};

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

/// `text` with every run of whitespace (line wraps included) as one space.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

const REMOVED_FORMS: [&str; 21] = [
    "type aliases",
    "associated types",
    "`static` items",
    "trait-bound `where` clauses",
    "`impl Trait` types",
    "qualified paths",
    "restricted visibility",
    "loop labels",
    "references and borrows",
    "pattern parameters",
    "the markers `mut`, `ref` and `move`",
    "half-open range patterns",
    "a leading `::`",
    "inner attributes",
    "the never type",
    "raw-pointer types",
    "`union` items",
    "`extern` blocks",
    "`extern crate` items",
    "named function-pointer parameters",
    "attributes and doc comments on a parameter",
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
    assert_eq!(rows.len(), REMOVED_FORMS.len(), "B.3.6 holds no row beyond the 21 forms");
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
    let taken = sec.split("Two forms that look absent").nth(1).expect("B.3.6 has a `Two forms that look absent` list");
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
    assert_eq!(rows.len(), 21);
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
    let undefined: Vec<_> = mentioned.iter().filter(|c| !defs.contains_key(*c) && !RETIRED_CODES.contains(&c.as_str())).collect();
    assert!(undefined.is_empty(), "codes mentioned and never defined: {undefined:?}");
    // A retired code (§0.2: numbers are not reused) is named only where its retirement is stated.
    let flat_specs = all_specs().iter().map(|(_, text)| flat(text)).collect::<Vec<_>>().join(" ");
    for code in RETIRED_CODES {
        assert!(!defs.contains_key(code), "{code} is retired and still has a defining row");
        assert!(flat_specs.contains(&format!("`{code}` is retired")), "the retirement of {code} is stated");
    }
}

/// The codes value semantics retired (Appendix G.71): each is named by the Resolution that
/// retires it and by G.71, and by no defining row.
const RETIRED_CODES: [&str; 6] = ["E0102", "E0331", "E0150", "E0151", "E0906", "E0908"];

#[test]
fn b1_counts_62_constructs() {
    // POL-08 AC 8, amended by explicit-unknowns (replaces `b1_still_counts_65_constructs`): the
    // literal `Unknown`, the contextual keyword `resolve` and the method `level` leave, so the
    // count and the rows agree at 62, and the B.1.1 kinds add up to it.
    let sec = section(&spec("appendix_b_grammar.md"), "## B.1 ");
    assert!(sec.contains("**62**"), "B.1 states 62");
    let rows = table_rows(&section(&sec, "### B.1.2"));
    assert_eq!(rows.len(), 62, "B.1.2 holds 62 rows");
    let last: usize = rows.last().unwrap()[0].parse().expect("row numbers are integers");
    assert_eq!(last, 62);
    let kinds: usize = table_rows(&section(&sec, "### B.1.1")).iter().filter_map(|r| r[2].parse::<usize>().ok()).sum();
    assert_eq!(kinds, 62, "the kinds of B.1.1 sum to the count");
    for gone in ["Unknown", "`resolve`", "`level`"] {
        assert!(!rows.iter().any(|r| r[1] == gone), "B.1.2 still lists {gone}");
    }
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
    assert!(h.starts_with("# Appendix H. Value library"), "title");
    assert!(h.contains("This appendix is normative."));
    for group in [
        "## H.1 Integer types",
        "## H.2 Float types",
        "## H.3 `bool`",
        "## H.4 `Option`",
        "## H.5 Arrays",
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

fn s34() -> String {
    section(&spec("part_i_values.md"), "### 3.4 ")
}

#[test]
fn e0162_is_defined_in_3_4() {
    // POL-04 AC 1: the row, its phase and its three conditions.
    let rows = table_rows(&s34());
    let row = rows.iter().find(|r| r[0] == "`E0162`").expect("§3.4 defines E0162");
    assert_eq!(row[2], "elaboration", "{row:?}");
    assert!(row[1].contains("declared twice in one table of one namespace"), "{row:?}");
    assert!(row[1].contains("explicit `use`") && row[1].contains("collides"), "{row:?}");
    assert!(row[1].contains("two glob imports") && row[1].contains("used"), "{row:?}");
    assert_eq!(defining_rows()["E0162"], ["part_i_values.md"], "one defining row");
}

#[test]
fn glob_shadowing_and_e0143_are_stated() {
    // POL-04 AC 2.
    let s = s34().split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(s.contains("An item or an explicit `use` shadows a glob import"), "{s}");
    assert!(s.contains("A hardware module and a unit struct occupy both tables"), "{s}");
    assert!(s.contains("Two `fn` items of one name are `E0143`, not `E0162`"), "{s}");
    let r = spec("part_i_values.md");
    let row = r.lines().find(|l| l.starts_with("| `E0143`")).unwrap();
    assert!(row.contains("a second `fn` of the same name"), "{row}");
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn no_reference_form_is_part_of_the_language() {
    // Value semantics (G.71, V1): §1 rule 2 leaves every reference out, `E0101` reports one at
    // parse and `E0102` is retired. This replaces the POL-06 AC 1 guard of the borrowed argument.
    let part = spec("part_i_values.md");
    let rule2 = one_line(part.split("- **§1 rule 2.**").nth(1).unwrap().split("- **§1 rule 3.**").next().unwrap());
    for needle in ["references and pointers of every kind", "the markers `mut`, `ref` and `move`", "`&` is the bitwise operator only"] {
        assert!(rule2.contains(needle), "{needle}: {rule2}");
    }
    assert!(!part.lines().any(|l| l.starts_with("| `E0102`")), "E0102 has no defining row");
    assert!(one_line(&part).contains("`E0102` is retired"), "the retirement is stated");
    let row = part.lines().find(|l| l.starts_with("| `E0101`")).expect("E0101 row");
    assert!(row.contains("a reference or a pointer in a type, an expression or a pattern"), "{row}");
    assert!(row.ends_with("| parse |"), "{row}");
}

#[test]
fn identifier_pattern_rule_is_stated() {
    // POL-06 AC 2: a rule of Part I, with its Resolution.
    let part = spec("part_i_values.md");
    let rule = one_line(part.split("- **§1 rule 14.**").nth(1).expect("§1 rule 14").split("\n\n").next().unwrap());
    for needle in [
        "plain identifier pattern",
        "with no `@`",
        "a constant, a unit variant or a unit struct",
        "is a path to that item and binds nothing",
    ] {
        assert!(rule.contains(needle), "{needle}: {rule}");
    }
    let after = part.split("- **§1 rule 14.**").nth(1).unwrap();
    assert!(after.split("**Errors**").next().unwrap().contains("> **Resolution.** Rule 14"), "a Resolution note");
}

#[test]
fn behavior_generics_are_repeated_not_positional() {
    // POL-05 AC 1: §10 rule 6, its Resolution and B.3.3 item 14.
    let part = spec("part_iv_behavior.md");
    let s10 = section(&part, "## 10.");
    let flat = one_line(&s10);
    assert!(!flat.contains("positionally"), "§10 no longer says `positionally`");
    let rule6 = one_line(s10.split("- **§10 rule 6**").nth(1).unwrap().split("- **§10 rule 7**").next().unwrap());
    assert!(rule6.contains("repeats the generic names of its module"), "{rule6}");
    assert!(rule6.contains("the same names in the same order"), "{rule6}");
    let note = flat.split("> **Resolution.** §10 rule 6:").nth(1).expect("Resolution of rule 6");
    assert!(note.contains("repeated in order"), "{note}");
    let b33 = one_line(&section(&spec("appendix_b_grammar.md"), "### B.3.3"));
    assert!(b33.contains("`names` repeats the generic names of the module, the same names in the same order"), "{b33}");
}

#[test]
fn items_sit_at_namespace_scope() {
    // POL-05 AC 2: B.2.1 note 3, §8 rule 1 and §3.4.3; E0401 keeps `the same file`.
    let b2 = one_line(&section(&spec("appendix_b_grammar.md"), "### B.2.1"));
    assert!(b2.contains("are items at namespace scope: a file or an inline `mod`"), "{b2}");
    assert!(b2.contains("A block may sit in any namespace of its module's file"), "{b2}");
    let p3 = spec("part_iii_structure.md");
    let rule1 = one_line(p3.split("- **§8 rule 1**").nth(1).unwrap().split("- **§8 rule 2**").next().unwrap());
    assert!(rule1.contains("namespace scope") && rule1.contains("any namespace of the same file"), "{rule1}");
    let r343 = one_line(spec("part_i_values.md").split("- **§3.4.3**").nth(1).unwrap().split("- **§3.4.4**").next().unwrap());
    assert!(r343.contains("namespace scope") && r343.contains("any namespace of"), "{r343}");
    let row = spec("part_iv_behavior.md");
    let row = row.lines().find(|l| l.starts_with("| `E0401`")).unwrap();
    assert!(row.contains("the same file"), "{row}");
}

#[test]
fn e0404_and_e0121_state_their_precedence() {
    // POL-07 AC 1: the E0404 row, §10 rule 8, the E0121 row and §3 rule 6.
    let p4 = spec("part_iv_behavior.md");
    let row = one_line(p4.lines().find(|l| l.starts_with("| `E0404`")).expect("E0404 row"));
    for needle in ["`signal`", "a module body", "`E0121` is the code reported"] {
        assert!(row.contains(needle), "{needle}: {row}");
    }
    assert!(row.contains("| parse |"), "E0404 stays a parse code: {row}");
    let rule8 = one_line(p4.split("- **§10 rule 8**").nth(1).unwrap().split("- **§10 rule 9**").next().unwrap());
    for needle in ["`signal`", "module body", "`E0121`"] {
        assert!(rule8.contains(needle), "{needle}: {rule8}");
    }
    let p1 = spec("part_i_values.md");
    let row = one_line(p1.lines().find(|l| l.starts_with("| `E0121`")).expect("E0121 row"));
    assert!(row.contains("`E0121` is the code reported, never `E0404`"), "{row}");
    assert!(row.contains("| parse |"), "E0121 stays a parse code: {row}");
    let rule6 = one_line(p1.split("- **§3 rule 6.**").nth(1).unwrap().split("- **§3 rule 7.**").next().unwrap());
    assert!(rule6.contains("`E0121`") && rule6.contains("never `E0404`"), "{rule6}");
    let a = spec("appendix_a_lexical.md");
    assert!(a.lines().any(|l| l.starts_with("| `E0907`") && l.contains("A well-formed `contribution` or `commit` in a block of the wrong kind is `E0404`")));
}

#[test]
fn value_blocks_may_hold_contributions_and_commits() {
    // POL-07 AC 2: B.3.4.
    let b34 = one_line(&section(&spec("appendix_b_grammar.md"), "### B.3.4"));
    assert!(b34.contains("a nested block that is used as a value"), "{b34}");
    assert!(b34.contains("let x = { i(a) +> 1.0; 2.0 };"), "{b34}");
    assert!(b34.contains("`contribution` or a `commit`"), "{b34}");
}

#[test]
fn place_has_no_parentheses() {
    // POL-07 AC 3: B.3.3.
    let b33 = one_line(&section(&spec("appendix_b_grammar.md"), "### B.3.3"));
    assert!(b33.contains("A `place` takes no parentheses, in a definition and in a commit alike"), "{b33}");
    assert!(b33.contains("`(q) = 1;`") && b33.contains("`(q) <- 1;`"), "{b33}");
}

/// The line of `text` that starts with the production name `name` followed by `::=`.
fn production<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with(name) && l[name.len()..].trim_start().starts_with("::="))
        .unwrap_or_else(|| panic!("no production `{name}`"));
    let mut out = vec![lines[start]];
    for l in &lines[start + 1..] {
        if l.trim_start().starts_with('|') {
            out.push(l);
        } else {
            break;
        }
    }
    out
}

#[test]
fn attrs_are_allowed_before_statements_and_hardware_items() {
    // POL-08 AC 1: A.4 rule 1, and B.3.2 with its excerpts in §4, §8 and §10.
    let a4 = one_line(&section(&spec("appendix_a_lexical.md"), "## A.4"));
    for needle in [
        "an attribute may precede",
        "every statement",
        "a reaction",
        "a definition",
        "a generation `for` or `if`",
        "a nature field",
        "a contribution",
        "a commit",
    ] {
        assert!(a4.contains(needle), "{needle}: {a4}");
    }
    let sources = [
        ("appendix_b_grammar.md", spec("appendix_b_grammar.md")),
        ("part_ii_carriers.md", spec("part_ii_carriers.md")),
        ("part_iii_structure.md", spec("part_iii_structure.md")),
        ("part_iv_behavior.md", spec("part_iv_behavior.md")),
    ];
    for (file, text) in &sources {
        let has = |name: &str| text.lines().any(|l| l.starts_with(name) && l[name.len()..].trim_start().starts_with("::="));
        let check = |name: &str, needle: &str, at: &str| {
            let p = production(text, name).join(" ");
            let ok = if at == "all" { p.matches(needle).count() >= 1 } else { p.contains(at) };
            assert!(ok && p.contains(needle), "{file}: `{name}` lacks `{needle}`: {p}");
        };
        if has("nature_field") {
            let p = production(text, "nature_field").join("\n");
            assert!(p.lines().next().unwrap().contains("attrs ( 'potential'"), "{file}: {p}");
            assert!(p.contains("| attrs 'value'") && !p.contains("'resolve'"), "{file}: the nature_field production has no `resolve` line (D17): {p}");
        }
        if has("reaction") {
            check("reaction", "attrs 'on'", "all");
            check("contribution", "attrs expr", "all");
        }
        if has("commit") {
            check("commit", "attrs place", "all");
        }
        if has("gen<X>") {
            let p = production(text, "gen<X>").join(" ");
            assert_eq!(p.matches("attrs").count(), 2, "{file}: {p}");
        }
        if has("module_item") {
            let p = production(text, "module_item").join(" ");
            assert!(p.contains("| attrs place '='"), "{file}: {p}");
        }
    }
}

#[test]
fn quantity_and_reaction_docs_are_metadata() {
    // POL-08 AC 2: A.4 and §21.
    let a4 = one_line(&section(&spec("appendix_a_lexical.md"), "## A.4"));
    assert!(a4.contains("the doc comments of a quantity and of a reaction are metadata"), "{a4}");
    assert!(a4.contains("those of a statement are kept in the tree and have no other effect"), "{a4}");
    let s21 = one_line(&section(&spec("part_vi_io.md"), "## 21."));
    assert!(s21.contains("a quantity of a nature, a reaction, a definition and a generation"), "{s21}");
    assert!(s21.contains("the docs of a statement are kept in the tree and have no other effect"), "{s21}");
}

#[test]
fn post_parse_constraints_name_their_phase() {
    // POL-08 AC 3: B.3.3 items 8 to 14; E0125, E0370, E0320, E0140, E0201, E0263 stay `elaboration`.
    let b33 = section(&spec("appendix_b_grammar.md"), "### B.3.3");
    let items: Vec<String> = b33
        .lines()
        .filter(|l| l.len() > 3 && l.as_bytes()[0].is_ascii_digit())
        .map(|l| l.to_string())
        .collect();
    let item = |n: usize| items.iter().find(|l| l.starts_with(&format!("{n}. "))).unwrap_or_else(|| panic!("item {n}")).clone();
    assert!(item(8).contains("(parse, `E0142`)"), "{}", item(8));
    for n in 9..=14 {
        assert!(item(n).contains("(elaboration, at name resolution)"), "{}", item(n));
    }
    assert!(one_line(&b33).contains("item 8 is checked at parse and items 9 to 14 at elaboration, during name resolution"), "{b33}");
    for (file, code) in [
        ("part_i_values.md", "E0125"),
        ("part_iii_structure.md", "E0370"),
        ("part_iii_structure.md", "E0320"),
        ("part_i_values.md", "E0140"),
        ("part_ii_carriers.md", "E0201"),
        ("part_ii_carriers.md", "E0263"),
    ] {
        let t = spec(file);
        let row = t.lines().find(|l| l.starts_with(&format!("| `{code}`"))).unwrap_or_else(|| panic!("{code}"));
        assert!(row.trim_end().ends_with("| elaboration |"), "{row}");
    }
}

#[test]
fn e0125_covers_bodies() {
    // POL-08 AC 4: the E0125 row and §3 rule 2.
    let p1 = spec("part_i_values.md");
    let row = one_line(p1.lines().find(|l| l.starts_with("| `E0125`")).expect("E0125 row"));
    assert!(row.contains("an `extern` function with a body"), "{row}");
    assert!(row.contains("a function without a body that is neither `extern` nor a trait item"), "{row}");
    assert!(row.ends_with("| elaboration |"), "{row}");
    let rule2 = one_line(p1.split("- **§3 rule 2.**").nth(1).unwrap().split("- **§3 rule 3.**").next().unwrap());
    assert!(rule2.contains("a function that is neither `extern` nor a trait item has a body"), "{rule2}");
    assert!(rule2.contains("`E0125`"), "{rule2}");
}

/// The `### G.<n>` entry of Appendix G.
fn g_entry(n: &str) -> String {
    one_line(&section(&spec("appendix_g_scope.md"), &format!("### G.{n} ")))
}

/// The status cell of the summary row of item `G.<n>`.
fn g_summary_status(n: &str) -> String {
    let g = spec("appendix_g_scope.md");
    let rows = table_rows(&section(&g, "## Summary"));
    let row = rows.iter().find(|r| r[0] == format!("G.{n}")).unwrap_or_else(|| panic!("summary row G.{n}"));
    row[2].clone()
}

#[test]
fn raw_identifiers_are_specified_and_g15_is_closed() {
    // POL-08 AC 5: A.1 rule 6 and G.15.
    let a1 = one_line(&section(&spec("appendix_a_lexical.md"), "## A.1"));
    let rule6 = one_line(a1.split(" 6. ").nth(1).unwrap().split(" 7. ").next().unwrap());
    for needle in [
        "`r#name` is an identifier for each of the seven added keywords and for no other word",
        "`r#module`, `r#nature`, `r#signal`, `r#on`, `r#out`, `r#flip` and `r#after`",
        "metadata lists the name without `r#`",
        "`E0900`",
    ] {
        assert!(rule6.contains(needle), "{needle}: {rule6}");
    }
    assert!(!rule6.contains("open item"), "{rule6}");
    assert!(a1.contains("`signal r#out: Electrical;`"), "the Resolution on `out` names the raw form: {a1}");
    assert_eq!(g_summary_status("15"), "Closed");
    let g15 = g_entry("15");
    assert!(g15.contains("**Status: Closed.**") && g15.contains("Appendix A.1"), "{g15}");
    assert!(!g15.contains("In force: an added keyword is not an identifier"), "{g15}");
}

#[test]
fn field_shorthand_is_specified_and_g14_is_closed() {
    // POL-08 AC 6: B.3.2 `instance` and `template`, B.3.3, the §8 excerpt, and G.14.
    for (file, text) in [("appendix_b_grammar.md", spec("appendix_b_grammar.md")), ("part_iii_structure.md", spec("part_iii_structure.md"))] {
        let inst = production(&text, "instance").join(" ");
        assert!(inst.contains("'{' ( IDENT ( ':' expr )? ',' )* '}'"), "{file}: {inst}");
        let tmpl = production(&text, "template").join(" ");
        assert!(tmpl.contains("'{' ( IDENT ( ':' expr )? ',' )* '}'"), "{file}: {tmpl}");
        assert!(!tmpl.contains("IDENT ':' expr"), "{file}: {tmpl}");
    }
    let b33 = one_line(&section(&spec("appendix_b_grammar.md"), "### B.3.3"));
    assert!(b33.contains("`{ r }` means `{ r: r }`"), "{b33}");
    assert!(b33.contains("`R(p, n) { r }`") && b33.contains("`R { r, c: 1p }`"), "{b33}");
    assert!(b33.contains("a field without `:`"), "{b33}");
    assert_eq!(g_summary_status("14"), "Closed");
    let g14 = g_entry("14");
    assert!(g14.contains("**Status: Closed.**") && g14.contains("Appendix B.3"), "{g14}");
    assert!(!g14.contains("In force: the long form only"), "{g14}");
}

#[test]
fn e0909_is_defined_for_out_of_range_literals() {
    // POL-13 AC 1: A.6 defines E0909 (phase parse); A.2 and §1 rule 7 state the rule.
    let a = spec("appendix_a_lexical.md");
    let a6 = section(&a, "## A.6");
    let row = table_rows(&a6)
        .into_iter()
        .find(|r| r[0] == "`E0909`")
        .expect("A.6 has an E0909 row");
    assert_eq!(row.last().map(String::as_str), Some("parse"));
    assert!(row[1].contains("infinite") && row[1].contains("128"), "{}", row[1]);
    let a2 = section(&a, "## A.2");
    assert!(a2.contains("E0909"), "A.2 states the range rule");
    assert!(a2.contains("2^(N-1)"), "A.2 says why `iN` admits 2^(N-1)");
    let one = section(&spec("part_i_values.md"), "## 1.");
    let rule7 = one.lines().find(|l| l.contains("§1 rule 7")).expect("§1 rule 7");
    let at = one.find(rule7).unwrap();
    assert!(one[at..].lines().take(4).any(|l| l.contains("E0909")), "§1 rule 7 states the range rule");
}

#[test]
fn e0910_counts_nesting_and_bounds_chains() {
    // POL-15 AC 1: A.6 defines E0910 as nesting over 256 levels, or more than
    // 4096 chain links on one path; a flat chain is not nesting.
    let a6 = section(&spec("appendix_a_lexical.md"), "## A.6");
    let row = table_rows(&a6)
        .into_iter()
        .find(|r| r[0] == "`E0910`")
        .expect("A.6 has an E0910 row");
    assert_eq!(row.last().map(String::as_str), Some("parse"));
    let text = &row[1];
    assert!(text.contains("256"), "{text}");
    assert!(text.contains("4096"), "{text}");
    for word in ["delimiters", "closures", "return", "break", "type arguments", "binary", "else if", "`as`", "`?`"] {
        assert!(text.contains(word), "the E0910 row names {word}: {text}");
    }
    assert!(!text.contains("operator chains"), "the old wording is gone: {text}");
}

/// What each recorded decision cites as its deciding section: a substring of the `Where` cell.
const DECIDED_IN: [(&str, &str); 16] = [
    ("D1", "B.3.6"),
    ("D2", "Appendix H"),
    ("D3", "§6 rule 11"),
    ("D5", "§3.4.7"),
    ("D6", "§10 rule 6"),
    ("D7", "§1"),
    ("D8", "A.6"),
    ("D9", "A.4"),
    ("D10", "§1 rule 14"),
    ("D11", "§8.5.7"),
    ("D12", "A.2"),
    ("D13", "§3.4.3"),
    ("D14", "§10 rule 8"),
    ("D15", "A.1"),
    ("D16", "B.3.2"),
    ("D17", "B.3.2"),
];

#[test]
fn appendix_g_records_the_decisions() {
    // POL-08 AC 9: one row per decision D1 to D17 except D4, naming the deciding section; G.38 stays Open.
    let g = spec("appendix_g_scope.md");
    let closed = section(&g, "### Closed in this specification");
    let rows = table_rows(&closed);
    for (decision, cited) in DECIDED_IN {
        let row = rows.iter().find(|r| r[0] == decision).unwrap_or_else(|| panic!("no row for {decision} in `Closed in this specification`"));
        assert_eq!(row.len(), 4, "{decision}: Item, Question, Decision, Where: {row:?}");
        assert!(!row[1].is_empty() && !row[2].is_empty(), "{decision}: question and decision are written: {row:?}");
        assert!(row[3].contains(cited), "{decision} cites {cited}: {}", row[3]);
    }
    assert!(rows.iter().all(|r| r[0] != "D4"), "D4 is not decided here: the standard library is G.38");
    assert_eq!(rows.iter().filter(|r| r[0].starts_with('D')).count(), 16);
    // The G-numbered rows are still there.
    for item in ["G.40", "G.41", "G.42", "G.43", "G.44"] {
        assert!(rows.iter().any(|r| r[0] == item), "{item} stays");
    }
    // G.38 was Open here; the gallery gate closes it (see `g38_is_closed`) and it still names the standard-library sources.
    let g38 = one_line(&section(&g, "### G.38"));
    assert!(g38.contains("standard library") && g38.contains("std/*.msl"), "G.38 names the standard-library sources: {g38}");
    assert!(g.lines().any(|l| l.starts_with("| G.40") && l.contains("D1")), "the overview names the decision rows");
}

#[test]
fn appendix_h_renders_every_code_span() {
    // POL-02: a cell holds code spans that open and close on their own; a
    // double backtick or an odd count renders a broken span (and derive names
    // are stated to resolve like bounds, D2).
    let h = appendix_h();
    for (n, line) in h.lines().enumerate() {
        assert!(!line.contains("``"), "line {}: nested backticks: {line}", n + 1);
        assert_eq!(line.matches('`').count() % 2, 0, "line {}: an unclosed code span: {line}", n + 1);
    }
    assert!(section(&h, "## H.8").contains("`E0161`"), "H.8 states that an unknown derive name is E0161");
}

#[test]
fn block_items_follow_the_namespace_rules() {
    // D5: the items of a block are one scope; a repeated name in a table is E0162.
    let s = s34().split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(s.contains("**§3.4.10** The items of a block"), "{s}");
    assert!(s.contains("`E0162` at the later name; this includes two `fn` of one name"), "{s}");
    let r = spec("part_i_values.md");
    let row = r.lines().find(|l| l.starts_with("| `E0162`")).unwrap();
    assert!(row.contains("one table of one namespace or of one block"), "{row}");
}

// ---- msl-packages: §3.5 Crates (PKG-01 AC 1 to 7 and 10) ----

fn s35() -> String {
    section(&spec("part_i_values.md"), "### 3.5 ")
}

fn s35_text() -> String {
    one_line(&s35())
}

#[test]
fn s35_defines_crate_root_and_dependency_name() {
    // PKG-01 AC 1: three bold defining occurrences, and §3.4's lead points at §3.5.
    let s = s35_text();
    assert!(s.contains("A **crate** is one tree of namespace modules rooted at one file"), "{s}");
    assert!(s.contains("The **crate root** is the file the tree starts from"), "{s}");
    assert!(s.contains("A **dependency name** names one other crate inside this crate"), "{s}");
    for term in ["**crate**", "**crate root**", "**dependency name**"] {
        assert_eq!(s.matches(term).count(), 1, "{term} is bold once");
    }
    let lead = one_line(&s34());
    assert!(lead.contains("a package of models is a crate ([§3.5](#35-crates))"), "{lead}");
    assert!(lead.contains("[§3.6](#36-packages)"), "{lead}");
}

#[test]
fn s35_orders_the_first_segment_lookup() {
    // PKG-01 AC 2: the five steps, in order.
    let s = s35_text();
    let steps = [
        "the scopes of the enclosing blocks and declarations",
        "the items and imports of the enclosing namespace",
        "the dependency names of the crate",
        "the prelude",
        "the core names",
    ];
    let mut at = s.find("The first segment of a path is looked up in this order").expect("rule 4");
    for step in steps {
        let hit = s[at..].find(step).unwrap_or_else(|| panic!("`{step}` out of order in {s}"));
        at += hit + step.len();
    }
}

#[test]
fn s35_local_names_win_in_paths_and_clash_in_use() {
    // PKG-01 AC 3.
    let s = s35_text();
    assert!(s.contains("an ordinary path follows rule 4: the item or import wins, without a diagnostic"), "{s}");
    assert!(s.contains("A `use` path whose first segment is that name is ambiguous (`E0163`)"), "{s}");
    assert!(s.contains("There is no leading `::`"), "{s}");
    assert!(s.contains("the dependency wins"), "dependency against a prelude item: {s}");
}

#[test]
fn s35_crate_and_super_are_per_crate() {
    // PKG-01 AC 4.
    let s = s35_text();
    assert!(s.contains("`crate` names the root of the crate that contains the path"), "{s}");
    assert!(s.contains("A crate's own name is not in scope inside it"), "{s}");
    assert!(s.contains("`super` at a crate root does not resolve (`E0161`)"), "{s}");
}

#[test]
fn s35_only_pub_crosses_a_crate() {
    // PKG-01 AC 5.
    let s = s35_text();
    assert!(
        s.contains("an item is visible only when it is `pub` and every namespace on the path from its crate root to it is `pub`"),
        "{s}"
    );
}

#[test]
fn std_is_implicit_and_the_prelude_is_std_prelude() {
    // PKG-01 AC 6.
    let s = s35_text();
    assert!(s.contains("`std` is a dependency of every crate"), "{s}");
    assert!(s.contains("No manifest names it and no dependency may be called `std`"), "{s}");
    assert!(s.contains("The prelude is implied in every file of every crate, the files of `std` included"), "{s}");
    let t = one_line(&s34());
    assert!(t.contains("the `pub` items of `std::prelude`"), "§3.4.2: {t}");
}

#[test]
fn e0161_row_covers_an_unknown_crate() {
    // PKG-01 AC 7: the §3.4 row names a first segment that is no dependency.
    let rows = table_rows(&s34());
    let row = rows.iter().find(|r| r[0] == "`E0161`").expect("§3.4 defines E0161");
    assert!(row[1].contains("a first segment that names no dependency of the crate"), "{row:?}");
    assert_eq!(row[2], "elaboration", "{row:?}");
}

#[test]
fn e0163_is_defined_in_3_5() {
    // PKG-01 AC 10: the defining row, its phase and the two ways out.
    let rows = table_rows(&s35());
    let row = rows.iter().find(|r| r[0] == "`E0163`").expect("§3.5 defines E0163");
    assert_eq!(row[2], "elaboration", "{row:?}");
    assert!(row[1].contains("first segment names both a dependency and an item or an import of the enclosing namespace"), "{row:?}");
    assert_eq!(defining_rows()["E0163"], ["part_i_values.md"], "one defining row");
    let s = s35_text();
    assert!(s.contains("`other = { package = \"<name>\", .. }`"), "{s}");
    assert!(s.contains("`crate::<name>`"), "{s}");
}

// ---- msl-packages: §0.3 and §3.6 Packages, layout and manifest keys (PKG-02 AC 1 to 4) ----

fn s36() -> String {
    section(&spec("part_i_values.md"), "### 3.6 ")
}

fn s36_text() -> String {
    one_line(&s36())
}

#[test]
fn phase_package_is_listed() {
    // PKG-02 AC 1: five phases, `package` first, rule 3 rejects on it.
    let index = spec("index.md");
    let s03 = section(&index, "### 0.3 ");
    let text = one_line(&s03);
    assert!(text.contains("An error is raised in one of five phases"), "{text}");
    let rows = table_rows(&s03);
    assert_eq!(rows[0][0], "**package**", "package is the first phase: {rows:?}");
    assert!(rows[0][1].contains("manifests are read") && rows[0][1].contains("crate graph fixed"), "{rows:?}");
    assert!(rows[0][2].contains("§3.6"), "{rows:?}");
    assert!(text.contains("*package*"), "rule 3 names the phase: {text}");
    assert!(text.contains("reject a model that meets an error condition of phase *package*, *parse* or *elaboration*"), "{text}");
}

#[test]
fn s36_fixes_the_package_layout() {
    // PKG-02 AC 2.
    let s = s36_text();
    assert!(s.contains("A **package** is a directory with a manifest"), "{s}");
    assert!(s.contains("The **manifest** is the file `Piperine.toml` in the package directory"), "{s}");
    assert!(s.contains("The crate root is `src/lib.msl`"), "{s}");
    assert!(s.contains("A source file has the extension `.msl`"), "{s}");
    assert!(s.contains("`mod name;` loads `name.msl` or `name/mod.msl`"), "{s}");
    assert!(s.contains("neither or both is `E0161`"), "{s}");
    assert_eq!(s.matches("**package**").count(), 1);
    assert_eq!(s.matches("**manifest**").count(), 1);
}

#[test]
fn s36_lists_every_manifest_key() {
    // PKG-02 AC 3: one table; each key has a row; any other is `E0171`.
    let sec = s36();
    let rows = table_rows(&sec);
    let key_rows: Vec<&Vec<String>> = rows.iter().filter(|r| r.len() == 4 && r[0].starts_with("`[")).collect();
    let keys: String = key_rows.iter().map(|r| format!("{} {}", r[0], r[1])).collect::<Vec<_>>().join("; ");
    for (table, key) in [
        ("`[package]`", "`name`"),
        ("`[package]`", "`version`"),
        ("`[package]`", "`authors`"),
        ("`[package]`", "`description`"),
        ("`[dependencies]` entry", "`path`"),
        ("`[dependencies]` entry", "`git`"),
        ("`[dependencies]` entry", "`version`"),
        ("`[dependencies]` entry", "`package`"),
    ] {
        assert!(key_rows.iter().any(|r| r[0] == table && r[1].contains(key)), "no row for {table} {key}: {keys}");
    }
    // "Exactly those eleven": nine rows, and eleven keys (`branch`, `tag`, `rev` share one row).
    assert_eq!(key_rows.len(), 9, "{keys}");
    let key_count: usize = key_rows.iter().map(|r| r[1].matches('`').count() / 2).sum();
    assert_eq!(key_count, 11, "{keys}");
    let refs = key_rows.iter().find(|r| r[1].contains("`branch`")).expect("branch, tag, rev row");
    assert!(refs[1].contains("`tag`") && refs[1].contains("`rev`"), "{refs:?}");
    let s = one_line(&sec);
    assert!(s.contains("holds the tables and keys of the table below and no other (`E0171`)"), "{s}");
}

#[test]
fn s36_names_are_identifiers() {
    // PKG-02 AC 4.
    let s = s36_text();
    assert!(
        s.contains("A package name and a dependency name are identifiers (A.1) that are no keyword of the language and not `std`"),
        "{s}"
    );
    assert!(s.contains("The package name is the crate's name"), "{s}");
    assert!(s.contains("A version is `MAJOR.MINOR.PATCH`, three decimal integers"), "{s}");
}

// ---- msl-packages: §3.6 dependencies, requirements, lockfile, errors (PKG-02 AC 5 to 9) ----

#[test]
fn s36_states_the_dependency_sources() {
    // PKG-02 AC 5.
    let s = s36_text();
    assert!(s.contains("A **dependency** is an entry of `[dependencies]`"), "{s}");
    assert!(s.contains("It has exactly one source: `path` or `git`"), "{s}");
    assert!(s.contains("it takes at most one of `branch`, `tag` and `rev`"), "{s}");
    assert!(s.contains("none means the repository's default branch"), "{s}");
    assert!(s.contains("relative to the directory of the manifest that names it"), "{s}");
    assert!(s.contains("`package` gives the located package's own name when it differs from the key"), "{s}");
    assert!(s.contains("Two dependencies that locate the same package"), "{s}");
}

#[test]
fn s36_gives_the_version_requirement_table() {
    // PKG-02 AC 6: the eight forms and the range each admits.
    let rows = table_rows(&s36());
    let expect = [
        ("`1.2.3`", "`>=1.2.3, <2.0.0`"),
        ("`1.2`", "`>=1.2.0, <2.0.0`"),
        ("`1`", "`>=1.0.0, <2.0.0`"),
        ("`0.2.3`", "`>=0.2.3, <0.3.0`"),
        ("`0.2`", "`>=0.2.0, <0.3.0`"),
        ("`0.0.3`", "`>=0.0.3, <0.0.4`"),
        ("`0.0`", "`>=0.0.0, <0.1.0`"),
        ("`0`", "`>=0.0.0, <1.0.0`"),
    ];
    for (req, range) in expect {
        let row = rows.iter().find(|r| r.len() == 2 && r[0] == req).unwrap_or_else(|| panic!("no requirement row {req}"));
        assert_eq!(row[1], range, "{req}");
    }
    assert!(s36_text().contains("`version` is a requirement the located package's version **MUST** satisfy (`E0173`)"));
}

#[test]
fn s36_has_no_search_path_and_no_cycle() {
    // PKG-02 AC 7, and §3.5 rule 10 now carries the code.
    let s = s36_text();
    assert!(s.contains("The graph **MUST NOT** hold a cycle (`E0174`)"), "{s}");
    assert!(s.contains("an implementation **MUST NOT** find a crate through a search path, a registry or an environment variable"), "{s}");
    let c = s35_text();
    assert!(c.contains("The dependencies of the crates form no cycle ([§3.6](#36-packages), `E0174`)"), "{c}");
    let rows = table_rows(&s35());
    let row = rows.iter().find(|r| r[0].starts_with("[§3.6]")).expect("§3.5 keyed row for §3.6");
    assert!(row[1].contains("`E0174`") && row[2] == "package", "{row:?}");
}

#[test]
fn s36_states_the_lockfile_rule() {
    // PKG-02 AC 8.
    let s = s36_text();
    assert!(s.contains("The **lockfile** is `Piperine.lock` beside the root package's manifest"), "{s}");
    assert!(s.contains("the recorded commit **MUST** be used"), "{s}");
    assert!(s.contains("An explicit update resolves again"), "{s}");
    assert!(s.contains("A lockfile inside a dependency is not read"), "{s}");
    assert!(s.contains("`version = 1`"), "{s}");
    for key in ["`name`", "`version`", "`source`", "`commit`"] {
        assert!(s.contains(key), "{key}");
    }
    assert!(s.contains("`git+<url>`") && s.contains("`?branch=<b>`") && s.contains("`?tag=<t>`") && s.contains("`?rev=<r>`"), "{s}");
    assert!(s.contains("40 hexadecimal digits"), "{s}");
}

#[test]
fn s36_defines_the_package_codes() {
    // PKG-02 AC 9: E0170 to E0176, each defined once, in phase `package`, none in `parse`.
    let rows = table_rows(&s36());
    let defs = defining_rows();
    for n in 170..=176 {
        let code = format!("`E0{n}`");
        let row = rows.iter().find(|r| r[0] == code).unwrap_or_else(|| panic!("§3.6 does not define {code}"));
        assert_eq!(row[2], "package", "{row:?}");
        assert_eq!(defs[&format!("E0{n}")], ["part_i_values.md"], "{code} is defined once");
    }
    let table: Vec<_> = rows.iter().filter(|r| r.len() == 3 && r[0].starts_with("`E017")).collect();
    assert_eq!(table.len(), 7);
    assert!(table.iter().all(|r| r[2] != "parse"));
}

#[test]
fn b36_lists_extern_crate() {
    // PKG-01 AC 8: the row, its code and what to write instead.
    let rows = table_rows(&b36());
    let row = rows.iter().find(|r| r[0].starts_with("`extern crate` items")).expect("B.3.6 lists `extern crate` items");
    assert_eq!(codes_in(&row[1]), ["E0900"], "{row:?}");
    assert!(row[2].contains("[dependencies]"), "{row:?}");
    assert_eq!(rows.iter().filter(|r| r[0].starts_with("`extern`")).count(), 1, "the `extern` blocks row is its own");
}

// ---- msl-packages: Appendix G, glossary and file map (PKG-02 AC 10 and 11) ----

/// What each package decision cites as its deciding sections: substrings of the `Where` cell.
const PACKAGE_DECISIONS: [(&str, &[&str]); 6] = [
    ("P1", &["§3.6 rule 2"]),
    ("P2", &["§3.5 rules 4, 5, 11"]),
    ("P3", &["§3.5 rules 3 and 9", "§3.4.2"]),
    ("P4", &["§3.6 rule 3"]),
    ("P5", &["§3.6 rules 7, 13, 14"]),
    ("P6", &["§0.3"]),
];

#[test]
fn appendix_g_records_the_package_decisions() {
    // PKG-02 AC 10: G.0 lists the two exclusions; P1 to P6 are closed rows citing §3.5, §3.6 or §0.3.
    let g = spec("appendix_g_scope.md");
    let g0 = table_rows(&section(&g, "## G.0"));
    let search = g0.iter().find(|r| r[0].contains("source search paths")).expect("G.0 lists source search paths");
    assert!(search[0].contains("`-I`") && search[1].contains("§3.6 rule 12"), "{search:?}");
    let registry = g0.iter().find(|r| r[0].contains("a package registry")).expect("G.0 lists a package registry");
    assert!(registry[1].contains("`path`") && registry[1].contains("`git`") && registry[1].contains("§3.6"), "{registry:?}");
    let closed = section(&g, "### Closed in this specification");
    let rows = table_rows(&closed);
    for (id, cited) in PACKAGE_DECISIONS {
        let row = rows.iter().find(|r| r[0] == id).unwrap_or_else(|| panic!("no row for {id}"));
        assert_eq!(row.len(), 4, "{id}: Item, Question, Decision, Where: {row:?}");
        assert!(!row[1].is_empty() && !row[2].is_empty(), "{id}: {row:?}");
        for c in cited {
            assert!(row[3].contains(c), "{id} cites {c}: {}", row[3]);
        }
    }
    assert_eq!(rows.iter().filter(|r| r[0].starts_with('P')).count(), 6);
    let closing = one_line(&closed);
    assert!(closing.contains("P1 to P6 number the decisions of the crate and package model"), "{closing}");
    // G.38's status is asserted by `g38_is_closed`.
}

#[test]
fn glossary_and_file_map_name_crates_and_packages() {
    // PKG-02 AC 11: a row for each term, linked to its defining section; `prelude` stays at §3.4.
    let index = spec("index.md");
    let rows = table_rows(&section(&index, "### 0.9 "));
    let expect = [
        ("crate", "§3.5", "#35-crates"),
        ("crate root", "§3.5", "#35-crates"),
        ("dependency name", "§3.5", "#35-crates"),
        ("dependency", "§3.6", "#36-packages"),
        ("package", "§3.6", "#36-packages"),
        ("manifest", "§3.6", "#36-packages"),
        ("lockfile", "§3.6", "#36-packages"),
        ("prelude", "§3.4", "#34-files-and-imports"),
    ];
    for (term, sec, anchor) in expect {
        let row = rows
            .iter()
            .find(|r| r[0].split(", ").any(|t| t == term))
            .unwrap_or_else(|| panic!("§0.9 has no row for {term}"));
        assert!(row[1].contains(sec) && row[1].contains(anchor), "{term}: {row:?}");
    }
    let map = table_rows(&section(&index, "### 0.8 "));
    let part_i = map.iter().find(|r| r[0].contains("part_i_values.md")).expect("Part I row");
    assert!(part_i[1].contains("crates") && part_i[1].contains("packages"), "{part_i:?}");
    assert!(part_i[2].contains("§1–3"), "{part_i:?}");
}

#[test]
fn later_commit_is_not_a_time_point() {
    let exec = spec("part_v_execution.md");
    let s = flat(&section(&exec, "### 17.2"));
    assert!(s.contains("at that instant"), "{s}");
    assert!(s.contains("by at least one tick"), "{s}");
}

#[test]
fn time_point_runs_the_procedure() {
    let exec = spec("part_v_execution.md");
    let s = flat(&section(&exec, "## 15."));
    assert!(s.contains("and runs the time-point procedure"), "{s}");
    assert!(s.contains("MAY accept solutions at other times"), "{s}");
}

#[test]
fn extern_c_interface_is_the_device_abi() {
    let structure = spec("part_iii_structure.md");
    let s = flat(&section(&structure, "### 8.5 "));
    assert!(s.contains("piperine_abi_1"), "{s}");
    let g = spec("appendix_g_scope.md");
    let rows = table_rows(&g);
    let row = rows.iter().find(|r| r[0] == "G.65").expect("G.65 row");
    assert!(!row.join(" ").contains("Open"), "{row:?}");
}

#[test]
fn structure_changing_set_is_refused() {
    let host = spec("part_viii_host.md");
    let s = flat(&section(&host, "### 23.1"));
    let start = s.find(" 7. A `set`").expect("rule 7");
    let end = s[start..].find(" 8. ").expect("rule 8") + start;
    let rule = &s[start..end];
    assert!(rule.contains("E0803") && rule.contains("extern"), "{rule}");
    let dc = rule.find("in a `Dc` sweep").expect("rule 7 names the `Dc` sweep");
    assert!(rule[dc..].contains("re-elaborates the instance"), "{rule}");
    let s85 = flat(&section(&spec("part_iii_structure.md"), "### 8.5 "));
    let r10 = &s85[s85.find("**§8.5.10**").expect("§8.5.10")..s85.find("**§8.5.11**").expect("§8.5.11")];
    assert!(r10.contains("E0803") && r10.contains("in a `Dc` sweep that point re-elaborates"), "{r10}");
}


#[test]
fn zero_bytes_are_the_default_value() {
    // EU-17 AC 1 (replaces `zero_bytes_are_the_unknown_value`): §2 rule 11, all-zero bytes are
    // the bytes of `Logic::X`, of `None` and of every derived `Default`.
    let s = flat(&spec("part_i_values.md"));
    assert!(s.contains("all-zero bytes are the bytes of `Logic::X`, of `None`"), "{s}");
}

#[test]
fn continuation_variables_are_core_functions() {
    let behavior = spec("part_iv_behavior.md");
    let s = section(&behavior, "### 14.4");
    let rows = table_rows(&s);
    for name in ["`gmin() -> f64`", "`source_scale() -> f64`"] {
        let row = rows.iter().find(|r| r[0] == name).unwrap_or_else(|| panic!("no row {name}"));
        assert_eq!(row[2], "environment of the solve", "{row:?}");
    }
    assert!(flat(&s).contains("names no session parameter of `simparam`"), "{s}");
}

#[test]
fn records_travel_as_quiet_commits() {
    let c = spec("appendix_c_implementation.md");
    let s = flat(&section(&c, "## C.1 "));
    assert!(s.contains("quiet"), "{s}");
    assert!(s.contains("no microstep"), "{s}");
}

#[test]
fn jump_has_no_solve_of_its_own() {
    let exec = spec("part_v_execution.md");
    let s = flat(&section(&exec, "### 15.2"));
    assert!(s.contains("no solve of its own"), "{s}");
    assert!(!s.contains("h → 0"), "the backward-Euler limit is gone");
    assert!(!s.contains("repeated once"), "the repeated solve is gone");
    let r6 = &s[s.find(" 6. ").expect("rule 6")..s.find(" 7. ").expect("rule 7")];
    assert!(r6.contains("backward Euler"), "{r6}");
    assert!(r6.contains("drops the previous derivative"), "{r6}");
    let landing = &s[s.find("**Before the solve of `t`.**").expect("before-the-solve case")
        ..s.find("**After the solve of `t`.**").expect("after-the-solve case")];
    assert!(landing.contains("is exempt from the truncation-error test"), "{landing}");
}

#[test]
fn reference_rows_describe_the_kept_jump() {
    let c = spec("appendix_c_implementation.md");
    let jumps: Vec<&str> = c.lines().filter(|l| l.starts_with("| the jump")).collect();
    assert_eq!(jumps.len(), 2, "C.1 and C.2 each have a jump row");
    for row in jumps {
        assert!(!row.contains("gap"), "{row}");
    }
    let restart = c.lines().find(|l| l.starts_with("| restart")).expect("restart row");
    assert!(restart.contains("10⁻³ of the last step"), "{restart}");
    assert!(restart.contains("after a host `set` only"), "{restart}");
    let g6 = section(&spec("appendix_g_scope.md"), "### G.6 ");
    for row in table_rows(&g6) {
        assert!(!row[0].contains("jump"), "G.6 lists no jump gap: {row:?}");
    }
}

#[test]
fn residual_test_uses_the_present_mapping() {
    let exec = spec("part_v_execution.md");
    let s = flat(&section(&exec, "### 17.1"));
    let node = s.split(". ").find(|x| x.contains("node's row is tested")).expect("node sentence");
    assert!(node.contains("potential"), "{node}");
    let branch = s.split(". ").find(|x| x.contains("branch row is tested")).expect("branch sentence");
    assert!(branch.contains("flow"), "{branch}");
}

#[test]
fn jump_solve_is_a_transient_step() {
    let exec = spec("part_v_execution.md");
    let row = exec.lines().find(|l| l.starts_with("| **jump solve**")).expect("jump solve row");
    assert!(row.contains("transient step"), "{row}");
    let f5 = flat(&section(&spec("appendix_f_gallery_mixed.md"), "## F.5 "));
    assert!(f5.contains("the right values are those of the first step that reads the jump"), "{f5}");
}

/// The text of one `- **§3 rule N.**` bullet of Part I, as one line.
fn s3_rule(n: u32) -> String {
    let p1 = spec("part_i_values.md");
    let start = format!("- **§3 rule {n}.**");
    let body = p1.split(&start).nth(1).unwrap_or_else(|| panic!("§3 rule {n}"));
    one_line(body.split("\n- **§3 rule").next().unwrap().split("\n\n").next().unwrap())
}

#[test]
fn s3_lists_the_intrinsic_abi() {
    // STD-02 AC 1: two ABI strings, `"intrinsic"` only in the crate `std`.
    let r = s3_rule(10);
    assert!(r.contains("`\"C\"` and `\"intrinsic\"`"), "{r}");
    assert!(r.contains("`\"intrinsic\"` is accepted only in the crate `std`"), "{r}");
    assert!(!r.contains("The only ABI string"), "{r}");
}

#[test]
fn s3_states_the_intrinsic_contract() {
    // STD-02 AC 2: no body, no scalar restriction, no `#[derivative]`, the contract is Part VII's entry.
    let p1 = spec("part_i_values.md");
    let text = one_line(&p1);
    let at = text.find("An `extern \"intrinsic\" fn`").expect("a sentence about `extern \"intrinsic\" fn`");
    let s = &text[at..text[at..].find("\n").map_or(text.len(), |n| at + n).min(at + 600)];
    for part in ["has no body", "not restricted to scalar types", "takes no `#[derivative]`", "its entry in Part VII"] {
        assert!(s.contains(part), "missing `{part}` in: {s}");
    }
}

#[test]
fn e0125_covers_intrinsic_outside_std() {
    // STD-02 AC 3: the E0125 row names the condition; the existing conditions stay.
    let p1 = spec("part_i_values.md");
    let row = one_line(p1.lines().find(|l| l.starts_with("| `E0125`")).expect("E0125 row"));
    assert!(row.contains("the ABI `\"intrinsic\"` outside the crate `std`"), "{row}");
    assert!(row.contains("an `extern` function with a body"), "{row}");
}

/// Part VII from the heading `## 22.` to the first `### Index`: the preamble of §22.
fn s22_preamble() -> String {
    let p7 = spec("part_vii_stdlib.md");
    let start = p7.find("## 22. Standard library").expect("§22 heading");
    let end = p7.find("### Index").expect("§22 Index");
    p7[start..end].to_string()
}

#[test]
fn s22_says_std_is_the_normative_source() {
    // STD-01 AC 1.
    let p = one_line(&s22_preamble());
    assert!(p.contains("The **standard library** is the crate `std`"), "{p}");
    assert!(p.contains("its source files are normative"), "{p}");
    assert!(p.contains("each `rust` fence of this part is an excerpt of one of them"), "{p}");
}

#[test]
fn s22_maps_sections_to_namespaces() {
    // STD-01 AC 2: the table "Sources".
    let pre = s22_preamble();
    let at = pre.find("Sources").expect("a table named Sources");
    let end = pre.find("Implemented by the engine").unwrap_or(pre.len());
    assert!(at < end, "Sources comes before Implemented by the engine");
    let rows = table_rows(&pre[at..end]);
    let expect = [
        ("§22.1", "std::values", "values.msl"),
        ("§22.2", "std::analog", "analog.msl"),
        ("§22.3", "std::nets", "nets.msl"),
        ("§22.4", "std::convert", "convert.msl"),
        ("§22.5", "std::monitor", "monitor.msl"),
        ("§22.6", "std::io", "io.msl"),
        ("§1", "std::constants", "constants.msl"),
        ("§4", "std::carriers", "carriers.msl"),
    ];
    assert_eq!(rows.len(), expect.len(), "{rows:?}");
    for (sec, ns, file) in expect {
        let row = rows.iter().find(|r| r[0].starts_with(sec)).unwrap_or_else(|| panic!("no row for {sec}: {rows:?}"));
        assert!(row[1].contains(ns) && row[2].contains(file), "{sec}: {row:?}");
    }
    assert!(rows.iter().find(|r| r[0].starts_with("§4")).unwrap()[0].contains("§11"), "{rows:?}");
}

#[test]
fn s22_drops_the_implied_pub_resolution() {
    // STD-01 AC 3.
    let p = one_line(&s22_preamble());
    assert!(!p.contains("as if written with `pub`"), "{p}");
    assert!(!p.contains("Listings that omit `pub`"), "{p}");
    // The permission to implement natively stays.
    assert!(p.contains("**MAY** implement an item natively"), "{p}");
}

#[test]
fn s22_lists_the_engine_items() {
    // STD-02 AC 4: exactly five rows, each with an entry and a reason.
    let pre = s22_preamble();
    let at = pre.find("Implemented by the engine").expect("a table named Implemented by the engine");
    let rows = table_rows(&pre[at..]);
    let names: Vec<&str> = rows.iter().map(|r| r[0].trim_matches('`')).collect();
    assert_eq!(names, ["matches_bits", "logic", "table_model", "parse_table", "parse_hex"], "{rows:?}");
    for r in &rows {
        assert_eq!(r.len(), 3, "Item | Entry | Reason: {r:?}");
        assert!(r[1].starts_with('§') && !r[2].is_empty(), "{r:?}");
    }
}

#[test]
fn s22_rule_4_names_extern_intrinsic() {
    // STD-02 AC 5.
    let pre = s22_preamble();
    let rule4 = one_line(pre.split("\n4. ").nth(1).expect("rule 4").split("\n5. ").next().unwrap());
    assert!(rule4.contains("signature and a contract"), "{rule4}");
    assert!(rule4.contains("declared `extern \"intrinsic\"`"), "{rule4}");
}

#[test]
fn analysis_kinds_list_transient() {
    // STD-01 AC 7: §11.1.1 and row 21 of Appendix B.1 list the five kinds.
    let kinds = "`Dc`, `Transient`, `Ac`, `Noise`, `Static`";
    let p4 = one_line(&spec("part_iv_behavior.md"));
    let at = p4.find("- **§11.1.1**").expect("§11.1.1 bullet");
    assert!(p4[at..at + 400].contains(kinds), "{}", &p4[at..at + 400]);
    let b = spec("appendix_b_grammar.md");
    let row = b.lines().find(|l| l.starts_with("| 21 |")).expect("row 21");
    assert!(row.contains(kinds), "{row}");
}

#[test]
fn no_analysis_tran_remains() {
    // STD-01 AC 8.
    for (name, text) in all_specs() {
        assert!(!text.contains("analysis(Tran)"), "{name} still writes analysis(Tran)");
    }
}

#[test]
fn tran_in_fences_is_the_module() {
    // STD-01 AC 9: in a `rust` or `rust,fragment` fence the token `Tran` is a declaration or
    // a placement of the module, so `(` follows (`<` for its generic declaration, explicit-unknowns
    // D20); the analysis kind is `Transient`.
    for (name, text) in all_specs() {
        let mut fenced = false;
        for (n, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("```") {
                fenced = !fenced && (t == "```rust" || t.starts_with("```rust,"));
                continue;
            }
            if !fenced {
                continue;
            }
            let b = line.as_bytes();
            let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
            let mut i = 0;
            while let Some(k) = line[i..].find("Tran") {
                let s = i + k;
                let e = s + 4;
                let alone = (s == 0 || !word(b[s - 1])) && (e == b.len() || !word(b[e]));
                assert!(!alone || b.get(e) == Some(&b'(') || b.get(e) == Some(&b'<'), "{name}:{}: `Tran` not followed by `(` or `<`: {line}", n + 1);
                i = e;
            }
        }
    }
}

const STDLIB_DECISIONS: [(&str, &[&str]); 5] = [
    ("S1", &["§3 rule 10", "§22 rule 4"]),
    ("S2", &["§22"]),
    ("S3", &["§11.1.1"]),
    ("S4", &["§22"]),
    ("S5", &["§1 rule 3"]),
];

#[test]
fn appendix_g_records_the_stdlib_decisions() {
    // STD-01 AC 10: S1 to S5 are closed rows of the decision table, each citing its sections.
    let g = spec("appendix_g_scope.md");
    let closed = section(&g, "### Closed in this specification");
    let rows = table_rows(&closed);
    for (id, cited) in STDLIB_DECISIONS {
        let row = rows.iter().find(|r| r[0] == id).unwrap_or_else(|| panic!("no row for {id}"));
        assert_eq!(row.len(), 4, "{id}: Item, Question, Decision, Where: {row:?}");
        assert!(!row[1].is_empty() && !row[2].is_empty(), "{id}: {row:?}");
        for c in cited {
            assert!(row[3].contains(c), "{id} cites {c}: {}", row[3]);
        }
    }
    assert_eq!(rows.iter().filter(|r| r[0].starts_with('S')).count(), 5);
    let s = |id: &str| rows.iter().find(|r| r[0] == id).unwrap()[2].clone();
    assert!(s("S1").contains("extern \"intrinsic\""), "{}", s("S1"));
    assert!(s("S3").contains("`Transient`"), "{}", s("S3"));
    assert!(s("S5").contains("1.0 / 0.0"), "{}", s("S5"));
    let closing = one_line(&closed);
    assert!(closing.contains("S1 to S5 number the decisions of the standard library"), "{closing}");
    // G.38's status is asserted by `g38_is_closed`.
}

#[test]
fn s1_constants_are_std_items() {
    // STD-01 AC 6: the six constants are `pub const` items of `std::constants`.
    let p1 = spec("part_i_values.md");
    let rule3 = one_line(p1.split("- **§1 rule 3.**").nth(1).unwrap().split("- **§1 rule 4.**").next().unwrap());
    assert!(rule3.contains("`pub const` items of `std::constants`"), "{rule3}");
    let at = p1.find("**Prelude constants**").expect("table caption");
    let block = one_line(&p1[at..p1[at..].find("\n- **§1 rule").map_or(p1.len(), |n| at + n)]);
    assert!(block.contains("six constants are `pub const` items of `std::constants`"), "{block}");
    assert!(block.contains("re-exported by the prelude"), "{block}");
}

#[test]
fn g64_names_the_normal_generator() {
    // STD-02 AC 9: the generator of `normal` is the one written in `std::analog`.
    let g = one_line(&section(&spec("appendix_g_scope.md"), "### G.64"));
    assert!(g.contains("the generator of `normal` is the one written in `std::analog`"), "{g}");
    assert!(g.contains("SplitMix64"), "{g}");
}

#[test]
fn e0707_is_replaced_by_e0317() {
    // STD-02 AC 7: a complex root without its conjugate and a zero denominator are a false `where`.
    let p7 = spec("part_vii_stdlib.md");
    for (file, text) in all_specs() {
        assert!(!text.contains("E0707"), "{file} still names E0707");
    }
    // The rule (one list item) that names each condition also gives `E0317`.
    let rule_with = |heading: &str, needle: &str| -> String {
        let rules: Vec<String> = section(&p7, heading).lines().filter(|l| l.starts_with(|c: char| c.is_ascii_digit())).map(one_line).collect();
        rules.into_iter().find(|r| r.contains(needle)).unwrap_or_else(|| panic!("{heading} has a rule naming {needle}"))
    };
    for heading in ["#### 22.2.13", "#### 22.2.14"] {
        let rule = rule_with(heading, "appear with its conjugate");
        assert!(rule.contains("`E0317`"), "{heading}: the conjugate rule gives E0317: {rule}");
    }
    let zero = rule_with("#### 22.2.14", "`den[D - 1] != 0.0`");
    assert!(zero.contains("`E0317`") && zero.contains("zero"), "the zero denominator gives E0317: {zero}");
}

#[test]
fn laplace_states_the_proper_rule() {
    // STD-02 AC 8: `laplace_nd` requires `D >= 1` and `N <= D`; breaking it is E0304.
    let s = one_line(&section(&spec("part_vii_stdlib.md"), "#### 22.2.14"));
    assert!(s.contains("`D >= 1`") && s.contains("`N <= D`") && s.contains("`E0304`"), "{s}");
    // F3: the leading denominator coefficient must be non-zero (`den: [1.0, 0.0]` is `E0317`).
    assert!(s.contains("`den[D - 1] != 0.0`") && s.contains("[1.0, 0.0]") && s.contains("`E0317`"), "{s}");
}

#[test]
fn g38_is_closed() {
    // STD-01 AC 4: G.38 is Closed and names the two excerpt guards and the gallery gate.
    let g = spec("appendix_g_scope.md");
    let g38 = one_line(&section(&g, "### G.38"));
    assert!(g38.contains("**Status: Closed.**"), "{g38}");
    for target in ["std_excerpts", "std_galleries"] {
        assert!(g38.contains(target), "G.38 names {target}: {g38}");
    }
    assert!(g.lines().any(|l| l.starts_with("| G.38 |") && l.trim_end().ends_with("| Closed |")), "the overview lists G.38 as Closed");
}

#[test]
fn appendix_c_says_parsed_and_resolved() {
    // STD-01 AC 5: the library and the galleries are parsed and resolved; nothing is executed.
    let c = one_line(&spec("appendix_c_implementation.md"));
    assert!(!c.contains("No part of this specification has been compiled"), "the old claim is gone");
    assert!(c.contains("parsed and resolved") && c.contains("nothing is executed"), "{c}");
}

#[test]
fn the_manifest_rule_admits_the_run_table() {
    // CLI-02 AC 4 (msl-cli T3): rule 3 of the manifest section allows a `[run]` table whose
    // content the language does not define and whose copy in a dependency is not read, and the
    // key table has its row; E0171 still covers a value of the wrong type
    let s = section(&spec("part_i_values.md"), "### 3.6 ");
    let rule3 = one_line(s.split("- **§3.6.3**").nth(1).expect("rule 3.6.3").split("- **§3.6.4**").next().unwrap());
    assert!(rule3.contains("`[run]` table is allowed"), "{rule3}");
    assert!(rule3.contains("belongs to the host") && rule3.contains("not defined by the language"), "{rule3}");
    assert!(rule3.contains("dependency's manifest is not read"), "{rule3}");
    let row = s.lines().find(|l| l.starts_with("| `[run]` |")).expect("the key table has a `[run]` row");
    assert!(row.contains("table") && row.contains("not read in a dependency"), "{row}");
    let e0171 = s.lines().find(|l| l.starts_with("| `E0171` |")).expect("E0171 row");
    assert!(e0171.contains("a value of the wrong type"), "{e0171}");
}

#[test]
fn part_viii_defines_a_bench() {
    // CLI-21 AC 1 (msl-cli T41): Part VIII defines a bench, a module without ports written in
    // MSL that places the design with its sources, stimulus and monitors, chosen as the top by
    // the host; the term is in the part's terms and in the index's term table
    let part = spec("part_viii_host.md");
    let terms = part.lines().find(|l| l.starts_with("**Terms defined here:**")).expect("the part's terms");
    assert!(terms.split([',', ' ']).any(|t| t == "bench"), "{terms}");
    let text = one_line(&section(&part, "## 23."));
    let at = text.find("A **bench** is").unwrap_or_else(|| panic!("no defining sentence: {text}"));
    let definition = text[at..].split_once(". ").map_or(&text[at..], |(s, _)| s);
    for part in ["module without ports", "written in MSL", "sources", "stimulus", "monitors", "top"] {
        assert!(definition.contains(part), "the definition names {part}: {definition}");
    }
    let index = spec("index.md");
    let row = index.lines().find(|l| l.starts_with("| bench |")).expect("the index's term table has `bench`");
    assert!(row.contains("part_viii_host.md"), "{row}");
}

/// Principle `n.` of the index's opening list, on one line.
fn index_principle(n: u32) -> String {
    let index = spec("index.md");
    let start = format!("{n}. ");
    let next = format!("{}. ", n + 1);
    let mut out = String::new();
    let mut inside = false;
    for line in index.lines() {
        if line.starts_with(&next) || (inside && line.trim().is_empty()) {
            break;
        }
        if line.starts_with(&start) {
            inside = true;
        }
        if inside {
            out.push_str(line);
            out.push(' ');
        }
    }
    one_line(&out)
}

#[test]
fn the_index_says_benches_are_msl_and_sweeps_are_host() {
    // CLI-21 AC 2: principle 3 names benches as MSL, and sweeps, Monte Carlo, optimization and
    // regression as the host's
    let p3 = index_principle(3);
    assert!(p3.contains("bench") && p3.contains("written in MSL"), "{p3}");
    let host = p3.split("written in MSL").nth(1).unwrap_or_default();
    for work in ["Sweeps", "Monte Carlo", "optimization", "regression", "live in the host"] {
        assert!(host.contains(work), "principle 3 gives {work} to the host: {p3}");
    }
}

#[test]
fn the_testbench_table_splits_stimulus_and_host_work() {
    // CLI-21 AC 3: the model column holds stimulus written as reactions and timers; the host
    // column keeps `force` and `release`, constrained random stimulus, sweeps and Monte Carlo
    let s = section(&spec("part_viii_host.md"), "### 23.1 ");
    let rows: Vec<Vec<String>> = s
        .lines()
        .filter(|l| l.starts_with('|') && !l.starts_with("| ---"))
        .skip(1)
        .map(|l| l.trim_matches('|').split(" | ").map(|c| c.trim().to_string()).collect())
        .collect();
    assert!(!rows.is_empty(), "the testbench table: {s}");
    let model = rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>().join(" / ");
    let host = rows.iter().map(|r| r[1].as_str()).collect::<Vec<_>>().join(" / ");
    assert!(model.contains("stimulus written as reactions and timers"), "model column: {model}");
    for kept in ["`force` and `release`", "constrained random stimulus", "sweeps", "Monte Carlo"] {
        assert!(host.contains(kept), "host column keeps {kept}: {host}");
    }
    assert!(!host.contains("stimulus sequences with waits"), "sequences written as reactions are the model's: {host}");
}

#[test]
fn no_spec_text_says_stimulus_is_host_only() {
    // CLI-21 AC 4: no file of docs/spec/ states that stimulus or a testbench lives only in the
    // host
    for (name, text) in all_specs() {
        let text = flat(&text);
        for claim in ["the testbench lives in the host", "stimulus sequences, sweeps and optimization live in the host", "belongs to the testbench, in the host"] {
            assert!(!text.contains(claim), "{name} still says \"{claim}\"");
        }
    }
}

#[test]
fn part_v_states_the_solver_hygiene_rules() {
    // HYG-17 AC 2 (Q1, Q3, Q5, Q6): the decided truncation basis, the floor, the restart step and
    // the later start are written in Part V
    // the Resolution blocks are quoted: drop the `> ` a wrapped line starts with
    let text = flat(&spec("part_v_execution.md")).replace(" > ", " ");
    for fact in [
        // Q5: the restart step after a landing or a host `set`
        "The step that restarts the integrator after a landing or a host `set` is at most a tenth of the smaller of the last accepted step and the time from `t` to the next scheduled time point",
        // Q6: a later start from an operating point
        "starts from an operating point at `(t_start, 0)`",
        "A start time that is not finite or is negative fails before the operating point",
        // Q1: the basis of rule 1 is the `ddt` states, with the charge tolerance
        "Where rule 1 is measured: on the `ddt` states of §15.2 rule 1",
        "which is at least the session's charge tolerance `chgtol`",
        // Q3 (amended 2026-11-18, ngspice's floor rule): E0508 is a Newton failure at the floor
        "Newton does not converge there, an attempt already at that step having failed",
        "a step that fails the truncation error (§17.2 rule 1) at it is accepted and counted, as ngspice does, never failed",
    ] {
        assert!(text.contains(fact), "Part V lacks \"{fact}\"");
    }
    assert!(!text.contains("on node potentials and branch-flow unknowns against their natures' `abstol`, and on a `ddt` state"), "the node basis of rule 1 is gone");
}

#[test]
fn appendix_c_states_the_solver_hygiene_numerics() {
    // HYG-17 AC 2 (Q1 to Q5): the convergence, truncation, step-control and restart rows of C.2,
    // the defaults of C.2.1 and item 4 of C.2.2 state the decided numerics
    let text = flat(&spec("appendix_c_implementation.md"));
    for fact in [
        // Q4
        "no damping by default (the plan's `NoDamping`); `NodeDamping`, ngspice's `niiter.c` rule",
        "is opt-in through the plan's `NewtonStrategy`, which the DC and transient drivers both use",
        // Q1, Q2
        "error by Milne's device over the `ddt` states (§15.2 rule 1)",
        "`reltol·max(|q|, chgtol)`",
        "with no component skipped for the asymmetry of its two differences",
        // Q3
        "is retaken there once; Newton failing at an attempt already at `dt_min` ends the run with `E0508`",
        "| step at the floor | one retake at `dt_min` after a larger attempt failed; Newton failing at the floor, `E0508`; the error test failing at the floor, accepted and counted | §17.2 |",
        "The step floor follows ngspice's `dctran.c` exactly (\"Timestep too small\")",
        "An attempt at `dt_min` that converges but fails the truncation error is accepted and counted (`floor_accepts`)",
        // F4: the stop snap
        "| step to the stop | a step whose end lies within two ticks short of the stop ends at the stop",
        "the smallest `CAP` of the last accepted solution still bounds it",
        // Q5
        "at most `0.1·min(last step, gap to the next scheduled time point)`, first order",
        "| restart step after a landing or a host `set` | at most 0.1 of the smaller of the last step and the gap to the next scheduled time point;",
    ] {
        assert!(text.contains(fact), "Appendix C lacks \"{fact}\"");
    }
    for gone in ["an update much larger than the last is damped to the midpoint", "accepted with a warning", "Accepting an error rejection at `dt_min` is reported as a warning"] {
        assert!(!text.contains(gone), "Appendix C still says \"{gone}\"");
    }
}

#[test]
fn appendix_c_holds_no_unknown_control_rows() {
    // EU-17 (replaces `appendix_c_states_the_loop_bound_under_unknown_control` and
    // `appendix_c_lists_the_loop_bound_among_the_defaults`): with no merge there is no loop bound
    // under unknown control, so neither C.2 nor C.2.1 has the row.
    let text = spec("appendix_c_implementation.md");
    let c2 = table_rows(&section(&text, "## C.2 "));
    assert!(!c2.iter().any(|r| r[0].contains("unknown control")), "C.2 still has the row: {c2:?}");
    let c21 = table_rows(&section(&text, "### C.2.1 "));
    assert!(!c21.iter().any(|r| r[0].contains("unknown control")), "C.2.1 still has the row: {c21:?}");
    assert!(c21.iter().any(|r| r[0] == "relaxation passes per loop"), "the relaxation default stays");
    assert!(!flat(&text).contains("Known flags"), "C.1 note 1 is gone");
}

#[test]
fn section_1_rule_11_states_the_loop_and_depth_bounds() {
    // Q5 (CV-07 AC 4, CV-36 AC 2): a loop of more than 2^24 iterations and a call nested more than
    // 1 024 deep are panics (§1 rule 11), and C.2.1 lists both bounds among the defaults.
    let rule = flat(&spec("part_i_values.md"));
    for fact in ["One loop that runs more than 2^24 iterations in one evaluation", "a call nested more than 1 024 deep, are panics of this kind"] {
        assert!(rule.contains(fact), "§1 rule 11 lacks \"{fact}\"");
    }
    let rows = table_rows(&section(&spec("appendix_c_implementation.md"), "### C.2.1 "));
    for expected in [["iterations of one loop in one evaluation", "2^24 (16 777 216); the next iteration is a panic", "§1 rule 11"], ["call depth", "1 024; the next nested call is a panic", "§1 rule 11"]] {
        assert!(rows.iter().any(|r| r[..] == expected), "C.2.1 lacks {expected:?}: {rows:?}");
    }
}

// ---- value semantics: G.71 and V1 to V7 ----

/// What each value-semantics decision cites as its deciding sections: substrings of the `Where` cell.
const VALUE_DECISIONS: [(&str, &[&str]); 7] = [
    ("V1", &["§1 rule 2", "§1.4.6"]),
    ("V2", &["§1.5.12", "A.1"]),
    ("V3", &["§1.4.6", "H.8 rule 1"]),
    ("V4", &["§1.2.5", "§1.5.9", "§4 rule 8"]),
    ("V5", &["§1.2.9"]),
    ("V6", &["§3.1.10"]),
    ("V7", &["§1.4.7"]),
];

#[test]
fn appendix_g_records_the_value_decisions() {
    // V1 to V7 are closed rows of the decision table; G.71 is the item, with status Implementation.
    let g = spec("appendix_g_scope.md");
    let closed = section(&g, "### Closed in this specification");
    let rows = table_rows(&closed);
    for (id, cited) in VALUE_DECISIONS {
        let row = rows.iter().find(|r| r[0] == id).unwrap_or_else(|| panic!("no row for {id}"));
        assert_eq!(row.len(), 4, "{id}: Item, Question, Decision, Where: {row:?}");
        assert!(!row[1].is_empty() && !row[2].is_empty(), "{id}: {row:?}");
        for c in cited {
            assert!(row[3].contains(c), "{id} cites {c}: {}", row[3]);
        }
    }
    assert_eq!(rows.iter().filter(|r| r[0].starts_with('V')).count(), 7);
    assert!(one_line(&closed).contains("V1 to V7 number the decisions of value semantics"));
}

#[test]
fn g71_is_closed_and_names_no_implementation_gap() {
    // VS-10 AC 2: the front end meets the amendment, so G.71 is Closed in its entry and its summary row, the retired
    // codes are named, and no sentence says the reference implementation does not meet it yet
    assert_eq!(g_summary_status("71"), "Closed");
    let g71 = g_entry("71");
    for needle in ["**Status: Closed.**", "Two codes are retired: `E0102` (§1) and `E0331` (§8.2)", "Provisional note, for review:"] {
        assert!(g71.contains(needle), "{needle}: {g71}");
    }
    for stale in ["does not yet meet", "still accepts", "are written in the spelling above", "`value-semantics` brings"] {
        assert!(!g71.contains(stale), "G.71 still says `{stale}`: {g71}");
    }
}

#[test]
fn every_value_copies_and_hardware_has_identity() {
    // V1, V3, V7: §1.4.6 and §1.4.7 state the split, and the Resolution names what was removed.
    let part = one_line(&spec("part_i_values.md"));
    for needle in [
        "- **§1.4.6** A value is **copied** wherever it is used.",
        "there is no trait and no derive that grants it, and no type that lacks it",
        "- **§1.4.7** Hardware is not a value.",
        "a carrier changes only by a definition, a commit (`<-`) or a contribution (`+>`)",
        "the traits `Clone` and `Copy`, their derives and the method `clone()`",
    ] {
        assert!(part.contains(needle), "{needle}");
    }
    let h8 = section(&appendix_h(), "## H.8");
    let traits = first_cell_names(&h8);
    assert!(!traits.contains("Clone") && !traits.contains("Copy"), "H.8 lists neither: {traits:?}");
    assert!(one_line(&h8).contains("it does not resolve (`E0161`)"), "H.8 rule 1 gives the general rule");
    assert!(spec("part_i_values.md").contains("pub trait Repr {"), "`Repr` has no supertrait");
}

#[test]
fn every_local_is_a_place_and_no_parameter_is() {
    // V2, V6: §1.5.12, §1.7.7 and §3.1.10; `mut`, `ref` and `move` are reserved words.
    let part = one_line(&spec("part_i_values.md"));
    for needle in [
        "A **place** is a local, a field of a place, or an element of a place.",
        "Every local is a place: no marker makes it one.",
        "a parameter of a function or of a closure, `self` included, and a name a closure captures are not places",
        "- **§1.7.7** Every local may be assigned again, with a value of its type",
        "- **§3.1.10** On a value type, `self` is a copy of the receiver",
        "There is no other receiver form and no shorthand for that assignment.",
    ] {
        assert!(part.contains(needle), "{needle}");
    }
    let a = spec("appendix_a_lexical.md");
    let reserved = a.lines().find(|l| l.starts_with("| reserved words |")).expect("the reserved-words row");
    let value = a.lines().find(|l| l.starts_with("| value keywords | `as`")).expect("the value-keywords row");
    for word in ["`move`", "`mut`", "`ref`"] {
        assert!(reserved.contains(word), "{word} is reserved: {reserved}");
        assert!(!value.contains(word), "{word} is no value keyword: {value}");
    }
    let iii = spec("part_iii_structure.md");
    assert!(!iii.lines().any(|l| l.starts_with("| `E0331`")), "E0331 has no defining row");
    assert!(one_line(&iii).contains("`E0331` is retired"), "the retirement is stated");
}

#[test]
fn there_is_no_slice_and_no_borrow_in_the_grammar() {
    // V1, V4, V5: the productions of B.3.7 and §1.2, the rules that stand in place, `resolve`.
    let b = spec("appendix_b_grammar.md");
    let i = spec("part_i_values.md");
    for text in [&b, &i] {
        assert!(!text.contains("STATIC_LIFETIME"), "no lifetime in a production");
        assert!(!text.contains("| '&' pat1"), "no `&p` pattern");
        assert!(!text.contains("'ref'? 'mut'?"), "no `ref` or `mut` binding");
        assert!(!text.contains("| '[' type ']'"), "no slice type");
    }
    assert!(b.contains("unary_expr     ::= ( '-' | '!' ) unary_expr | postfix_expr"), "no prefix `&` or `*`");
    assert!(b.contains("closure        ::= ( '||' |"), "no `move` before a closure");
    let part = one_line(&i);
    for needle in [
        "there is no slice type",
        "- **§1.2.6** There is no reference type.",
        "is the array of the elements `a` to `b - 1`, of type `[T; b - a]`; its bounds are const expressions",
        "- **§1.2.9** `str` is the type of a string: a sequence of Unicode scalar values. It is a value type without `Repr`.",
        "- **§1.6.9** A name a pattern binds holds a copy of the part it matched",
    ] {
        assert!(part.contains(needle), "{needle}");
    }
    let ii = one_line(&spec("part_ii_carriers.md"));
    assert!(ii.contains("fn resolve<const N: usize>(drives: [X; N]) -> V;"), "§4 rule 8 takes an array");
    let h5 = section(&appendix_h(), "## H.5");
    assert!(!h5.contains('&'), "H.5 lends nothing: {h5}");
}

#[test]
fn section_22_no_longer_says_std_is_in_the_earlier_spelling() {
    // VS-08 AC 3: `std` and the fences of Part VII that are excerpts of it are written in the amended spelling,
    // so §22 holds no Resolution about the earlier one, and the two spellings of `MeasuredDiode` agree.
    let part = one_line(&spec("part_vii_stdlib.md"));
    assert!(!part.contains("still written in the spelling of earlier revisions"), "§22 keeps no Resolution about the earlier spelling");
    for removed in ["&str", "&[T]", "let mut ", "Clone, Copy"] {
        assert!(!part.contains(removed), "Part VII still writes `{removed}`");
    }
    assert!(part.contains("pub fn strongest<const N: usize>(d: [Driven; N]) -> Driven") && part.contains("pub fn sum<const N: usize>(d: [f64; N]) -> f64"), "§22.3 takes the drives as an array under a const parameter");
    assert!(one_line(&spec("part_vii_stdlib.md")).contains(r#"table_model(v(a, k), IV, "3E")"#));
    assert!(one_line(&spec("part_vi_io.md")).contains(r#"table_model(v(a, k), IV, "3E")"#), "Part VI and Part VII agree");
}

#[test]
fn the_commit_of_an_event_read_as_a_bool_is_stated() {
    // UE-02 AC 5, design D2: §13 rule 2 reads an event as a `bool` in the value of a commit, so
    // the spec's own `Comparator` (`q <- up`) is well typed; rule 8's "never committed" is an
    // `Event` where a type other than `bool` is expected.
    let behavior = flat(&spec("part_iv_behavior.md"));
    for fact in [
        "§13 rule 2 reads an event as a `bool` in the value of a commit too",
        "`on up || dn { q <- up; }`, the `Comparator` of this section, commits whether `up` fired in this",
        "an `Event` committed where a type other than `bool` is expected (`E0441`)",
        "an `Event` committed where a type other than `bool` is expected, or carried (§13 rule 8)",
    ] {
        assert!(behavior.contains(fact), "§13 lacks \"{fact}\"");
    }
}

#[test]
fn the_host_drives_a_tops_node_ports_e0810_e0811() {
    // PI-07 AC 8, D10: §8.1.14 and §23.1 rule 1 state the host source, and §23.1's errors table
    // carries E0810 (host) and E0811 (elaboration)
    let structure = flat(&spec("part_iii_structure.md"));
    for fact in ["The host drives each node port with a host source, a potential source to `gnd`", "(`E0810` when a run has none)", "a value input of the top is refused (`E0811`)"] {
        assert!(structure.contains(fact), "§8.1.14 lacks \"{fact}\"");
    }
    let host = spec("part_viii_host.md");
    let flat_host = flat(&host);
    for fact in [
        "It drives each node port of the top with one **host source**",
        "a DC value, an optional AC stimulus (magnitude and phase) and a piecewise-linear waveform",
        "A run needs a host source for every node port of the top (`E0810`)",
        "A value input of the top has no host source and is refused at elaboration (`E0811`)",
    ] {
        assert!(flat_host.contains(fact), "§23.1 lacks \"{fact}\"");
    }
    let quoted = flat_host.replace("> ", "");
    assert!(quoted.contains("a host change of its value at a time is a jump there"), "the Resolution lacks the jump");
    let rows = table_rows(&section(&host, "### 23.1 "));
    for (code, phase) in [("`E0810`", "host"), ("`E0811`", "elaboration")] {
        let row = rows.iter().find(|r| r[0] == code).unwrap_or_else(|| panic!("§23.1's errors table lacks {code}"));
        assert_eq!(row[2], phase, "{code}'s phase");
    }
}

#[test]
fn appendix_c2_states_the_spline_end_conditions() {
    // PI-10, D18, G.64: Appendix C.2 documents the end conditions of the `table_model` splines
    // (§22.2.15 rule 8) and the derivative at a sample
    let text = spec("appendix_c_implementation.md");
    let rows = table_rows(&section(&text, "## C.2 "));
    let row = rows.iter().find(|r| r[0].starts_with("`table_model` splines")).expect("C.2's table has a row for the table_model splines");
    let algorithm = flat(&row[1]);
    for fact in [
        "Control `3`: a cubic spline with the not-a-knot end conditions",
        "Control `2`: a quadratic spline whose first segment is the parabola through the first three samples",
        "The derivative at a sample is the slope of the segment on its right; at the last sample, the slope of the segment on its left",
    ] {
        assert!(algorithm.contains(fact), "C.2's row lacks \"{fact}\": {algorithm}");
    }
}

#[test]
fn the_host_reads_composites_and_sets_a_leaf() {
    // composite-values CV-21, CV-22, CV-18 (Q2 (c), D13): section 23.1 states how a host reads a composite
    // value and sets a composite param
    let host = spec("part_viii_host.md");
    let text = flat(&section(&host, "### 23.1 ").replace("> ", ""));
    for fact in [
        "one column per dotted leaf in CSV and a plot",
        "An enum with no payload reads as the name of its variant, not its tag",
        "A printed path that names a leaf (`Top.st.n`, `Top.v[2]`) prints that leaf alone",
        "A composite param is set by the path of one leaf (`Top.f.num[2]`, `Top.p.x`, `Top.t.0`), before the run",
        "a false one is `E0317`, a path that names no leaf `E0801`",
    ] {
        assert!(text.contains(fact), "§23.1 lacks \"{fact}\"");
    }
}

#[test]
fn the_analog_step_floor_past_2_52_ticks_is_a_recorded_deviation() {
    let c = spec("appendix_c_implementation.md");
    let row = c.lines().find(|l| l.starts_with("| analog step floor past 2^52 ticks")).expect("step floor row");
    assert!(row.contains("256 `f64` spacings"), "{row}");
    assert!(row.contains("transient-performance T25"), "{row}");
}

// ---- explicit unknowns: EU-17 ----

#[test]
fn section_3_3_states_the_explicit_model() {
    // EU-17 AC 1 (replaces the assertions that quoted the known-flag §3.3): R1 to R6 as numbered
    // rules, and the sentence that no primitive numeric type implements `Default`.
    let s = flat(&section(&spec("part_i_values.md"), "### 3.3 "));
    assert!(s.starts_with("### 3.3 Uncertainty and absence"), "{s}");
    for rule in ["**§3.3.1 A value is a value.**", "**§3.3.2 Uncertainty is a type.**", "**§3.3.3 Absence is a type.**", "**§3.3.4 A carrier, a drive and a param start with a value.**", "**§3.3.5 Control takes a `bool`.**", "**§3.3.6 `analog` reads values.**"] {
        assert!(s.contains(rule), "§3.3 lacks {rule}");
    }
    assert!(s.contains("**`bool`, `UInt<N>`, `Int<N>`, `Float<N>` and `str` do not implement `Default`.**"), "R4: {s}");
    assert!(s.contains("differs from Rust on purpose"), "R4 says it differs from Rust");
    assert!(s.contains("`E0187`") && s.contains("`E0180`"), "R4 and R5 name their codes");
    assert!(s.contains("`a == b` on two `Logic` values is a `Logic`"), "R5");
    assert!(s.contains("converting a `Logic` to a binary value gives an `Option`"), "R6 conversions");
}

#[test]
fn a_nature_takes_trait_impls() {
    // EU-11 AC 1, D17: §3.1.9 accepts `impl Trait for` a nature, `self` is a handle, E0134 is a
    // bundle's, and a trait is a capability (§3.7.13).
    let i = spec("part_i_values.md");
    let s31 = flat(&section(&i, "### 3.1 "));
    assert!(s31.contains("`impl Trait for Type` is accepted for a value type and for a nature"), "{s31}");
    assert!(s31.contains("`self`, where a function has one, is a handle"), "{s31}");
    assert!(s31.contains("`impl Trait for` a bundle is `E0134`"), "{s31}");
    let e0134 = table_rows(&section(&i, "### 3.1 ")).into_iter().find(|r| r[0] == "`E0134`").expect("E0134 row");
    assert_eq!(e0134[1], "`impl Trait for` a bundle", "{e0134:?}");
    let s37 = flat(&section(&i, "### 3.7 "));
    assert!(s37.contains("**§3.7.13** A trait is a **capability**"), "{s37}");
    assert!(s37.contains("`W: Nature + Resolve<Driven>`"), "{s37}");
}

#[test]
fn appendix_h_holds_default_from_and_the_operator_traits() {
    // EU-17 AC 6: the `Default` rule of D4, `From`, the parameters of `PartialEq`, H.10.
    let h = appendix_h();
    let h8 = flat(&section(&h, "## H.8"));
    assert!(h8.contains("**`bool`, `UInt<N>`, `Int<N>`, `Float<N>` and `str` do not implement `Default`**"), "{h8}");
    assert!(h8.contains("`Option<T>` (`None`, for every `T`)"), "{h8}");
    assert!(h8.contains("`PartialEq<Rhs = Self, Out = bool>`") && h8.contains("`trait From<T> { fn from(x: T) -> Self; }`"), "{h8}");
    assert!(!h.contains("all-or-nothing"), "H rule 5 (all-or-nothing methods) is removed");
    let rows = table_rows(&section(&h, "## H.10"));
    let traits: Vec<&str> = rows.iter().map(|r| r[1].as_str()).collect();
    for t in ["`BitAnd<Rhs = Self, Out = Self>`", "`BitOr<Rhs = Self, Out = Self>`", "`BitXor<Rhs = Self, Out = Self>`", "`Not<Out = Self>`", "`Shl<Rhs = usize, Out = Self>`, `Shr<Rhs = usize, Out = Self>`", "`PartialEq<Rhs = Self, Out = bool>`"] {
        assert!(traits.contains(&t), "H.10 lacks {t}: {traits:?}");
    }
    let s = flat(&section(&h, "## H.10"));
    for rule in ["the one `impl` whose type is the left operand's", "at most one `impl` exists (`E0187`)", "`a &= b` is `a = a & b`", "have no trait"] {
        assert!(s.contains(rule), "H.10 lacks \"{rule}\"");
    }
}

// ---- explicit unknowns: EU-11, resolved nets as a capability ----

/// Part II section 4 rule 8 and section 5.2, flattened.
fn part_ii_rules() -> (String, String) {
    let ii = spec("part_ii_carriers.md");
    (flat(&section(&ii, "## 4. ")), flat(&section(&ii, "### 5.2 ")))
}

#[test]
fn resolve_is_a_trait_of_two_parameters() {
    // EU-11 AC 2, D18: `Resolve<X: Repr, V: Repr = X>`, one function, `V` is the `value` type
    // (E0203), no `resolve` field and no `resolve` keyword.
    let (nat, _) = part_ii_rules();
    assert!(nat.contains("pub trait Resolve<X: Repr, V: Repr = X> { fn resolve<const N: usize>(drives: [X; N]) -> V; }"), "{nat}");
    assert!(nat.contains("`V` **MUST** be the nature's `value` type"), "{nat}");
    assert!(nat.contains("There is no `resolve` field and no keyword `resolve`"), "{nat}");
    let e0203 = table_rows(&section(&spec("part_ii_carriers.md"), "## 4. ")).into_iter().find(|r| r[0] == "`E0203`").expect("E0203 row");
    assert!(e0203[1].contains("`impl Resolve<X, V>`") && e0203[1].contains("`value` type"), "{e0203:?}");
}

#[test]
fn many_drives_need_the_resolve_capability() {
    // EU-11 AC 3, D21: a carrier takes more than one drive only when its nature implements
    // `Resolve`; a second drive of any other is E0220, naming the nature and `Resolve`.
    let (_, drives) = part_ii_rules();
    assert!(drives.contains("A **one-driver carrier** is a carrier of a discrete nature that does not implement `Resolve`."), "{drives}");
    assert!(drives.contains("A second drive is `E0220`, and its diagnostic names the nature and `Resolve`."), "{drives}");
    assert!(drives.contains("takes any number of them, zero included"), "{drives}");
    let e0220 = table_rows(&section(&spec("part_ii_carriers.md"), "### 5.2 ")).into_iter().find(|r| r[0] == "`E0220`").expect("E0220 row");
    assert!(e0220[1].contains("names the nature and `Resolve`"), "{e0220:?}");
}

#[test]
fn a_drive_picks_its_impl_by_its_type() {
    // EU-11 AC 4, D19: E0225 for two driven types, E0180 for a type no `impl` accepts.
    let (_, drives) = part_ii_rules();
    assert!(drives.contains("A drive picks its `impl` of `Resolve` by the type of the value it drives."), "{drives}");
    assert!(drives.contains("two driven types on one carrier are `E0225`") && drives.contains("no `impl` accepts is `E0180`"), "{drives}");
    let rows = table_rows(&section(&spec("part_ii_carriers.md"), "### 5.2 "));
    assert!(rows.iter().any(|r| r[0] == "`E0225`"), "E0225 has its defining row in 5.2");
}

#[test]
fn resolve_receives_plain_values_for_every_count() {
    // EU-11 AC 5 and 7, D14: plain values in instance-path order, every count from zero, and a
    // drive that relaxation has not produced is absent, not a value.
    let (nat, _) = part_ii_rules();
    assert!(nat.contains("receives as an array of plain values in the order of their instance paths"), "{nat}");
    assert!(nat.contains("with `N` the number of drives a call receives, zero included"), "{nat}");
    assert!(nat.contains("It receives every present drive as a plain value of type `X`"), "{nat}");
    assert!(nat.contains("absence is not a value of the type"), "{nat}");
}

#[test]
fn a_drive_without_a_value_starts_at_its_default() {
    // EU-11 AC 6, EU-04 AC 2: a drive with no initial value starts at `Default::default()` of the
    // driven type and is a drive from `(0, 0)`; a resolved carrier needs no initial value.
    let (_, drives) = part_ii_rules();
    assert!(drives.contains("A drive with no initial value starts at `Default::default()` of the driven type"), "{drives}");
    assert!(drives.contains("It is a drive from `(0, 0)` on"), "{drives}");
    assert!(drives.contains("needs no initial value of its own"), "{drives}");
    let ii = flat(&section(&spec("part_ii_carriers.md"), "### 5.1 "));
    assert!(ii.contains("starts at `Default::default()` of its value type"), "{ii}");
    assert!(drives.contains("a type without it is `E0187` (§5.1 errors)"), "{drives}");
}

#[test]
fn a_relaxing_drive_starts_absent() {
    // EU-11 AC 7, D14: every drive of a loop starts each relaxation (each microstep it runs) absent from what `resolve`
    // receives, the passes of one relaxation build on the previous pass, and absence is not a value of the type.
    let s = flat(&section(&spec("part_iii_structure.md"), "### 8.2 "));
    assert!(s.contains("every drive of the loop starts each relaxation absent from what `resolve` receives (a relaxation is one run of the loop in one microstep); absence is not a value of the type"), "{s}");
    assert!(s.contains("each from the values the previous pass left within this relaxation"), "{s}");
    assert!(!s.contains("starts each pass absent"), "the old step 1 is gone: {s}");
}

#[test]
fn a_loop_that_does_not_settle_is_e0511() {
    // EU-11 AC 8, Q3: the run stops with E0511 naming the loop's drives; the row replaces the
    // "is not an error" row of the 8.2 table, and Part V lists the code among the run codes.
    let iii = spec("part_iii_structure.md");
    let s = flat(&section(&iii, "### 8.2 "));
    assert!(s.contains("the run stops with `E0511`, which names the loop's drives"), "{s}");
    assert!(!s.contains("is not an error: its levels are unknown"), "the old row is gone");
    let row = table_rows(&section(&iii, "### 8.2 ")).into_iter().find(|r| r[0] == "`E0511`").expect("E0511 row in 8.2");
    assert_eq!(row[2], "run", "{row:?}");
}

#[test]
fn part_iv_reads_values_and_the_first_commit_is_a_change() {
    // EU-13 AC 2 and 5, EU-14: §13 rule 6 says the initial value's commit at `(0, 0)` is a change
    // of every carrier and that `rise()` does not fire for a carrier that starts high; Part IV
    // holds none of the removed words.
    let iv = spec("part_iv_behavior.md");
    let s = flat(&iv);
    assert!(s.contains("that commit is a change of every carrier. So `changed()` fires there for every carrier. `rise()` and `fall()` do not"), "{s}");
    assert!(s.contains("When the guard is false the reaction does not run"), "§13 rule 1");
    for gone in ["known_or", "is_known", "`level(", ".level(", "`Unknown`", "flag-total", "under unknown control", "known flag", "`E0150`"] {
        assert!(!s.contains(gone), "Part IV still says {gone}");
    }
}

#[test]
fn records_and_the_host_hold_values_only() {
    // EU-15 AC 1 to 3, EU-04 AC 6: a record is its declared fields, a `require` is a pass or a
    // fail, a `fatal` ends the run, a top-level `in` port follows 3.3.4 (§19, §23.1 rule 9,
    // §23.2 rules 3 and 4).
    let vi = flat(&spec("part_vi_io.md"));
    assert!(vi.contains("A record is its declared fields and that context, and nothing else"), "{vi}");
    assert!(vi.contains("A `require` record is a pass or a fail: its `pass` field is a `bool`."), "{vi}");
    assert!(vi.contains("| `E0604` | A field name is repeated in one `emit!`. |"), "{vi}");
    let viii = flat(&spec("part_viii_host.md"));
    assert!(viii.contains("else `Default::default()` of its type. When none of the three exists the analysis is refused with `E0187` naming the port."), "{viii}");
    assert!(viii.contains("3. A `require` record is a pass or a fail (§19): its `pass` field is a `bool`, and there is no third verdict. 4. A `fatal` record ends the run (§19)."), "{viii}");
    for (name, text) in [("part_v_execution.md", flat(&spec("part_v_execution.md"))), ("part_vi_io.md", vi), ("part_viii_host.md", viii)] {
        for gone in ["known_or", "`maybe`", "maybe =", "`Unknown`", "flag-total", "unknown control", "known flag"] {
            assert!(!text.contains(gone), "{name} still says {gone}");
        }
    }
    let v = flat(&spec("part_v_execution.md"));
    assert!(v.contains("| §8.2.3 | A loop of resolved drives still moves after its bounded passes (`E0511`)"), "Part V lists E0511 among the run codes");
}

// ---- explicit unknowns: EU-12, Part VII ----

#[test]
fn section_22_1_holds_the_four_state_tables() {
    // EU-08, EU-09, EU-17 AC 2: the four-state tables of `&`, `|`, `^`, `!` are printed in 22.1.2,
    // beside the `impl`s that implement them; `Known` and `merge` are gone.
    let s22 = spec("part_vii_stdlib.md");
    let s = section(&s22, "### 22.1 ");
    let rows = table_rows(&section(&s, "#### 22.1.2"));
    let find = |first: &str| rows.iter().filter(|r| r[0] == first).cloned().collect::<Vec<_>>();
    // `&`: 0 & anything is 0; 1 & 1 is 1; 1 & X and X & X are X.
    assert!(rows.contains(&vec!["**0**".into(), "0".into(), "0".into(), "0".into()]), "{rows:?}");
    assert!(rows.contains(&vec!["**1**".into(), "0".into(), "1".into(), "X".into()]), "{rows:?}");
    assert!(rows.contains(&vec!["**X**".into(), "0".into(), "X".into(), "X".into()]), "{rows:?}");
    // `|`: 1 | anything is 1; `^`: any X operand gives X; `!`: X and Z give X.
    assert!(rows.contains(&vec!["**1**".into(), "1".into(), "1".into(), "1".into()]), "{rows:?}");
    assert!(rows.contains(&vec!["**X**".into(), "X".into(), "X".into(), "X".into()]), "{rows:?}");
    assert!(rows.contains(&vec!["X, Z".into(), "X".into()]), "{rows:?}");
    assert_eq!(find("**0**").len(), 3, "one row for 0 in each of the three binary tables");
    let flat_s = flat(&s);
    for gone in ["#### 22.1.1 `Known`", "#### 22.1.2 `merge`", "known_or", "is_known"] {
        assert!(!flat_s.contains(gone), "22.1 still holds {gone}");
    }
    assert!(flat_s.contains("#### 22.1.1 `Logic` and `LogicState`"), "22.1.1 declares Logic");
    assert!(flat_s.contains("`pub binary: UInt<N>`") || flat_s.contains("pub binary: UInt<N>"), "D1: two planes");
    assert!(flat_s.contains("`a == b` on two `Logic<N>` is a `Logic<1>`: X when any level of either operand is X or Z"), "EU-09 AC 1");
    assert!(flat_s.contains("`#[derive(PartialEq)]` on a struct with a `Logic` field is `E0187`"), "EU-09 AC 4");
    assert!(flat_s.contains("`+`, `-` and `<` on one are `E0180`"), "EU-08 AC 7");
}

#[test]
fn strongest_resolves_by_strength() {
    // EU-12 AC 1, Q5 = B: five strengths, `Driven`, and the table of `Driven::strongest`.
    let s = section(&spec("part_vii_stdlib.md"), "### 22.3 ");
    let f = flat(&s);
    assert!(s.contains("### 22.3 Strengths and ready-made net modules"), "retitled");
    assert!(f.contains("The order is `HighZ < Weak < Pull < Strong < Supply`."), "{f}");
    assert!(f.contains("pub struct Driven { pub v: Logic, pub s: Strength, }") || f.contains("pub v: Logic,"), "Driven");
    let rows = table_rows(&section(&s, "#### 22.3.2"));
    let drives: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(drives, ["none, or all `HighZ`", "one strongest drive", "several of the greatest strength, same level", "several of the greatest strength, levels differ"], "{rows:?}");
    assert_eq!(rows[0][1], "`Driven::Z`");
    assert_eq!(rows[3][1], "X, that strength");
    assert!(f.contains("A conflict is a value") && f.contains("`E0511`"), "a conflict is a value, an instability an error");
}

#[test]
fn the_library_declares_no_net_nature() {
    // EU-12 AC 2, D20: no `Wire`, `Tri`, `SumNet`, `tri`, `Logic::wired`; the eight modules are
    // generic over `W: Nature + Resolve<Driven>`.
    let part = spec("part_vii_stdlib.md");
    let s = section(&part, "### 22.3 ");
    assert!(!s.contains("pub nature"), "22.3 declares no nature");
    for gone in ["Wire", "SumNet", "Tri<", "`tri`", "wired"] {
        assert!(!part.contains(gone), "Part VII still says {gone}");
    }
    let bound = "<W: Nature + Resolve<Driven>>";
    let all = flat(&part);
    for module in ["Tribuf", "Pullup", "Pulldown", "Keeper", "Tran", "Tgate", "Bidir", "TribufDelayed"] {
        assert!(all.contains(&format!("pub module {module}{bound}")), "{module} is generic over a Resolve<Driven> nature");
    }
    // The strength rationale is at most fifteen lines and names only citations that were confirmed.
    let why = section(&s, "#### 22.3.3");
    assert!(why.lines().filter(|l| !l.trim().is_empty()).count() <= 15 + 2, "{why}");
    for confirmed in ["IEEE 1364-2005", "§9.22.3", "IEEE 1800 §6.6.7", "IEEE 1164", "ngspice manual (§8.6.1"] {
        assert!(why.contains(confirmed), "the rationale lacks {confirmed}");
    }
    for dropped in ["Logisim", "Digital (H. Neemann)", "IEEE 1666", "§28", "§7.9"] {
        assert!(!why.contains(dropped), "the rationale kept an unconfirmed citation: {dropped}");
    }
}

/// The words the explicit model removed (EU-17 AC 3).
const REMOVED_WORDS: [&str; 8] = ["known flag", "`Unknown`", "`level(", "known_or", "is_known", "flag-total", "under unknown control", "maybe = true"];

/// The text of `docs/spec` outside Appendix G's entry G.72, which records what was removed.
fn spec_without_g72() -> Vec<(String, String)> {
    all_specs()
        .into_iter()
        .map(|(name, text)| {
            if name != "appendix_g_scope.md" {
                return (name, text);
            }
            let a = text.find("### G.72 ").expect("G.72 exists");
            let b = a + text[a..].find("## Closed items").expect("G.72 ends at the closed items");
            (name, format!("{}{}", &text[..a], &text[b..]))
        })
        .collect()
}

#[test]
fn the_removed_words_are_absent_from_the_specification() {
    // EU-17 AC 3: outside G.72, none of the removed words is anywhere in docs/spec.
    let mut found = Vec::new();
    for (name, text) in spec_without_g72() {
        let flat_text = flat(&text);
        for word in REMOVED_WORDS {
            if flat_text.contains(word) {
                found.push(format!("{name}: {word}"));
            }
        }
        // the library net `Wire`, not the tail of a longer name (`AndWire`)
        if flat_text.match_indices("Wire").any(|(at, _)| !flat_text[..at].ends_with(|c: char| c.is_ascii_alphabetic())) {
            found.push(format!("{name}: Wire"));
        }
    }
    assert!(found.is_empty(), "the specification still says: {found:#?}");
}

#[test]
fn appendix_g_records_the_explicit_model() {
    // EU-17 AC 4: G.72 lists what was removed; since phase A of `explicit-unknowns` it says the
    // implementation meets it in every layer and names `Logic<N>` as phase B (T40);
    // G.29, G.46 and G.57 are closed by it, and no longer have an open or flagged entry.
    let g = spec("appendix_g_scope.md");
    let g72 = flat(&section(&g, "### G.72"));
    for removed in ["the known flag of every level of every value, and the literal `Unknown`", "`level`, `known`, `known_or`, `is_known`, `high`, `merge`, and `Known`", "the merge of effects under unknown control", "the gate of an `analog` read", "`maybe` on a record", "the `resolve` field of a nature, and the library nets `Wire`, `Tri` and `SumNet`"] {
        assert!(g72.contains(removed), "G.72 does not list: {removed}");
    }
    assert!(!g72.contains("does not yet meet this item"), "G.72 says the implementation still falls short: {g72}");
    assert!(g72.contains("The reference implementation meets this item in every layer") && g72.contains("`Logic<N>`") && g72.contains("phase B of `explicit-unknowns`"), "{g72}");
    let closed = flat(&section(&g, "### Other closed items"));
    for item in ["G.28, G.29, G.45, G.46, G.55, G.57"] {
        assert!(closed.contains(item), "the closed table lacks {item}");
    }
    assert!(!g.contains("### G.29 "), "G.29 has no open entry");
    assert!(table_rows(&section(&g, "## Flagged resolutions")).iter().all(|r| !r[0].starts_with("G.45") && !r[0].starts_with("G.46")), "G.45 and G.46 left the flagged table");
}

#[test]
fn the_gallery_works_four_nets() {
    // EU-12 AC 3: F.4 holds four worked nets, each with its nature, its `impl` and its module
    // code: a plain `bool` into a wired-AND nature, an open-drain line with a pull-up, a
    // tri-state bus with two drivers and a keeper, and a conflict of equal strengths; plus the
    // typed bus of D12 as a fifth listing.
    let f = spec("appendix_f_gallery_mixed.md");
    let s = section(&f, "## F.4");
    for heading in ["### F.4.1 A plain `bool` into a wired-AND net", "### F.4.2 An open-drain line with a pull-up", "### F.4.3 A tri-state bus with two drivers and a keeper", "### F.4.4 A conflict of equal strengths"] {
        assert!(s.contains(heading), "F.4 lacks {heading}");
    }
    let flat_s = flat(&s);
    for needle in ["pub nature AndWire { value: bool }", "impl Resolve<bool> for AndWire", "pub nature Sda { value: Driven }", "impl Resolve<Driven> for Sda", "pub nature TriBus { value: Driven }", "impl Resolve<Driven> for TriBus", "let rp = Pullup(sda);", "let k = Keeper(line);", "pub nature DataBus { value: Data }", "impl Resolve<Option<u8>, Data> for DataBus", "`Driven { v: Logic::X, s: Strength::Strong }`", "`E0511`"] {
        assert!(flat_s.contains(needle), "F.4 lacks {needle}");
    }
    assert!(!flat_s.contains(": Wire;") && !flat_s.contains(": Wire,"), "no library Wire");
}

#[test]
fn the_host_fails_loud_and_names_the_absent_cell() {
    // EU-15 (D15, finding 34): a host that cannot read a cell fails and never fills it; the two cells with no value of
    // their own are absent, not unknown, and each format spells that one cell
    let part = spec("part_viii_host.md");
    let s = flat(&part.replace("\n> ", "\n"));
    assert!(s.contains("a host that cannot read one fails with the cause"), "{s}");
    assert!(s.contains("It never fills the cell in."), "{s}");
    assert!(s.contains("are **absent**, which is not an unknown value"), "{s}");
    assert!(s.contains("the payload of a `None`, a field of another variant"), "{s}");
    assert!(s.contains("a param no placement has given a value"), "{s}");
    assert!(s.contains("an empty field in a text table and in CSV, `null` in JSON Lines, and nothing in a plot"), "{s}");
}
