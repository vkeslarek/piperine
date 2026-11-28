//! Scope: the guard of the specification's error codes (feature `params-and-io`, PI-13). Every
//! code of every error table under `docs/spec/` is raised by a non-test source file under
//! `crates/*/src`, or is listed in `tests/support/code_owners.tsv` with the feature and the
//! construct that will raise it; a listed code that a source now raises is a stale entry and
//! fails naming it. The counting rule is `GAPS.md` section 10's: a code is raised when it is
//! spelled on a line that is not a comment. The helpers are structs with methods; no macro
//! builds a table or an assertion.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use piperine_lang_fixtures::spec_tables::SpecTables;


/// The repository the guard reads: its specification pages and its crates' sources.
struct Repository;

impl Repository {
    /// The repository root, two levels above this crate.
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Every file under `dir` whose extension is `extension`, in name order.
    fn files_under(dir: &Path, extension: &str) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else { return found };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if path.is_dir() {
                found.extend(Self::files_under(&path, extension));
            } else if path.extension().is_some_and(|found_extension| found_extension == extension) {
                found.push(path);
            }
        }
        found
    }
}

/// The codes of the error tables of `docs/spec/*.md`: every table row whose first cell is a
/// code written as `Ennnn` (PI-13 AC 1).
struct CodeTable {
    codes: BTreeSet<String>,
}

impl CodeTable {
    /// Read every page of `docs/spec/`.
    fn from_spec() -> Self {
        let mut codes = BTreeSet::new();
        for page in Repository::files_under(&Repository::root().join("docs/spec"), "md") {
            let text = std::fs::read_to_string(&page).unwrap_or_else(|error| panic!("{}: {error}", page.display()));
            for row in SpecTables::table_rows(&text) {
                if let Some(code) = row.first().and_then(|cell| Self::code_of_cell(cell)) {
                    codes.insert(code);
                }
            }
        }
        Self { codes }
    }

    /// The code a table cell names when the whole cell is one code in backticks.
    fn code_of_cell(cell: &str) -> Option<String> {
        let text = cell.strip_prefix('`')?.strip_suffix('`')?;
        SourceSpellings::is_code(text).then(|| text.to_string())
    }
}

/// The codes spelled on a line that is not a comment in a source file under `crates/*/src`
/// (`GAPS.md` section 10's counting rule), and where each is spelled.
struct SourceSpellings {
    sites: BTreeMap<String, Vec<String>>,
}

impl SourceSpellings {
    /// Walk `src/` of every crate.
    fn from_crates() -> Self {
        let mut sites: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let crates = Repository::root().join("crates");
        let mut members: Vec<PathBuf> = std::fs::read_dir(&crates).expect("crates/ exists").flatten().map(|entry| entry.path()).collect();
        members.sort();
        for member in members {
            for file in Repository::files_under(&member.join("src"), "rs") {
                let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
                for (index, line) in text.lines().enumerate() {
                    if line.trim_start().starts_with("//") {
                        continue;
                    }
                    for code in Self::codes_in(line) {
                        sites.entry(code).or_default().push(format!("{}:{}", file.display(), index + 1));
                    }
                }
            }
        }
        Self { sites }
    }

    /// Whether `text` is `E` and four digits.
    fn is_code(text: &str) -> bool {
        text.len() == 5 && text.starts_with('E') && text[1..].bytes().all(|byte| byte.is_ascii_digit())
    }

    /// Every `Ennnn` or `Pnnnn` spelled in `line` that is not part of a longer word.
    fn codes_in(line: &str) -> Vec<String> {
        let bytes = line.as_bytes();
        let is_word_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
        let mut found = Vec::new();
        for start in 0..bytes.len().saturating_sub(4) {
            let window = &bytes[start..start + 5];
            let is_code = matches!(window[0], b'E' | b'P') && window[1..].iter().all(u8::is_ascii_digit);
            let is_word_start = start == 0 || !is_word_byte(bytes[start - 1]);
            let is_word_end = bytes.get(start + 5).is_none_or(|next| !is_word_byte(*next));
            if is_code && is_word_start && is_word_end {
                found.push(String::from_utf8_lossy(window).into_owned());
            }
        }
        found
    }

    /// Whether some non-comment line spells `code`.
    fn raises(&self, code: &str) -> bool {
        self.sites.contains_key(code)
    }

    /// Where `code` is spelled, for a message.
    fn sites_of(&self, code: &str) -> &[String] {
        self.sites.get(code).map_or(&[][..], Vec::as_slice)
    }
}

/// One allow-list row: a code no source raises yet, the feature that will, and the construct.
struct CodeOwner {
    code: String,
    feature: String,
    construct: String,
}

/// The allow-list `tests/support/code_owners.tsv`: code, owner feature, construct.
struct CodeOwners {
    entries: Vec<CodeOwner>,
}

impl CodeOwners {
    /// Read the allow-list; `#` lines are comments.
    fn load() -> Self {
        let path = Repository::root().join("crates/piperine-lang/tests/support/code_owners.tsv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let entries = text
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(|line| {
                let mut cells = line.split('\t').map(str::trim);
                CodeOwner { code: cells.next().unwrap_or_default().to_string(), feature: cells.next().unwrap_or_default().to_string(), construct: cells.next().unwrap_or_default().to_string() }
            })
            .collect();
        Self { entries }
    }

    /// Whether `code` is listed.
    fn lists(&self, code: &str) -> bool {
        self.entries.iter().any(|entry| entry.code == code)
    }
}

#[test]
fn every_spec_code_is_raised_or_owned() {
    // PI-13 AC 1: a code of the spec that no source raises and no entry lists is a gap nobody owns.
    let table = CodeTable::from_spec();
    let spellings = SourceSpellings::from_crates();
    let owners = CodeOwners::load();
    let unowned: Vec<&String> = table.codes.iter().filter(|code| !spellings.raises(code) && !owners.lists(code)).collect();
    assert!(unowned.is_empty(), "codes of docs/spec raised by no source and listed in no entry of code_owners.tsv: {unowned:?}");
}

#[test]
fn no_owned_code_is_raised() {
    // PI-13 AC 3: an entry whose code a source now raises is stale.
    let spellings = SourceSpellings::from_crates();
    let owners = CodeOwners::load();
    let stale: Vec<String> = owners.entries.iter().filter(|entry| spellings.raises(&entry.code)).map(|entry| format!("{} (raised at {:?})", entry.code, spellings.sites_of(&entry.code))).collect();
    assert!(stale.is_empty(), "entries of code_owners.tsv for codes a source raises; remove the rows: {stale:?}");
}

#[test]
fn every_owned_code_names_its_owner_and_construct() {
    // PI-13 AC 2: each entry says who raises the code and for which construct; each names a code of the spec once.
    let table = CodeTable::from_spec();
    let owners = CodeOwners::load();
    let mut seen = BTreeSet::new();
    for entry in &owners.entries {
        assert!(!entry.feature.is_empty(), "{} names no owner feature", entry.code);
        assert!(!entry.construct.is_empty(), "{} names no construct", entry.code);
        assert!(table.codes.contains(&entry.code), "{} is no code of docs/spec", entry.code);
        assert!(seen.insert(entry.code.clone()), "{} is listed twice", entry.code);
    }
}

#[test]
fn no_source_spells_p0002() {
    // PI-06 (X5): every mismatch carries the code its spec row names, so the provisional
    // `P0002` is spelled outside a comment in no source file.
    let spellings = SourceSpellings::from_crates();
    assert!(!spellings.raises("P0002"), "sources that still spell P0002: {:?}", spellings.sites_of("P0002"));
}

#[test]
fn this_features_codes_are_all_raised() {
    // PI-13 AC 4, PI-12 AC 9 (params-and-io): the codes this feature owns (spec.md's table, with `E0810` and `E0811`; `E0906` is retired by explicit-unknowns, MD-78, and is no code of a table now) are raised by a
    // source and listed in no entry of `code_owners.tsv`.
    const CODES: [&str; 26] = [
        "E0105", "E0122", "E0180", "E0182", "E0188", "E0204", "E0205", "E0244", "E0315", "E0333", "E0354", "E0361", "E0610", "E0612", "E0613", "E0614", "E0703", "E0704", "E0705", "E0706", "E0904",
        "E0810", "E0811", "E0141", "E0802", "E0803",
    ];
    let spellings = SourceSpellings::from_crates();
    let owners = CodeOwners::load();
    for code in CODES {
        assert!(spellings.raises(code), "{code} is no code a source raises");
        assert!(!owners.lists(code), "{code} is still in code_owners.tsv");
    }
}
