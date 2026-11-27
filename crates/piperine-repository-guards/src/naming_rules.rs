//! The naming rules (CC-05): a declared name is made of words, or of terms of the domain glossary,
//! and a bare type name is declared once.

use std::collections::{BTreeMap, BTreeSet};

use crate::allowance::Findings;
use crate::item_scan::{ScannedKind, ItemScan, CodeOrigin, ScannedItem};
use crate::source_tree::SourceTree;

/// The terms the domain spells in short form: names a reader already knows (`msl`, `abi`, `dc`).
pub struct Glossary {
    terms: BTreeSet<String>,
    /// The declared names in which a term means something other than its row says (`diode_rev`
    /// is reverse bias, not `rev`'s reversed order): the term does not clear them.
    other_meanings: BTreeSet<String>,
}

impl Glossary {
    /// Reads `data/glossary.tsv`: one `term<TAB>meaning` row per line, with an optional third
    /// column listing, comma-separated, the names in which the term means something else.
    /// Panics when the file cannot be read.
    pub fn read() -> Glossary {
        let path = SourceTree::guards_directory().join("data/glossary.tsv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        Glossary::parse("data/glossary.tsv", &text)
    }

    /// Parses glossary text read from `origin`; `#` lines and blank lines are comments. Panics,
    /// naming `origin` and the line, on a row that is not 2 or 3 tab-separated cells or whose term
    /// is empty.
    pub fn parse(origin: &str, text: &str) -> Glossary {
        let mut terms = BTreeSet::new();
        let mut other_meanings = BTreeSet::new();
        for (index, line) in text.lines().enumerate().filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#')) {
            let cells: Vec<&str> = line.split('\t').collect();
            if !(2..=3).contains(&cells.len()) || cells[0].trim().is_empty() {
                panic!("{origin}:{}: `{line}` is not `term<TAB>meaning[<TAB>names]`", index + 1);
            }
            terms.insert(cells[0].trim().to_lowercase());
            if let Some(names) = cells.get(2) {
                other_meanings.extend(names.split(',').map(str::trim).filter(|name| !name.is_empty()).map(str::to_string));
            }
        }
        Glossary { terms, other_meanings }
    }

    /// Whether the name is one in which a term means something other than its row says.
    pub fn means_otherwise_in(&self, name: &str) -> bool {
        self.other_meanings.contains(name)
    }

    /// The names of the third column: those in which a term means something else.
    pub fn other_meaning_names(&self) -> &BTreeSet<String> {
        &self.other_meanings
    }

    /// Whether the glossary holds the term.
    pub fn holds(&self, term: &str) -> bool {
        self.terms.contains(term)
    }

    /// The terms made of several words (`ac_stim`), which are checked against a whole name.
    fn compounds(&self) -> impl Iterator<Item = &String> {
        self.terms.iter().filter(|term| term.contains('_'))
    }
}

/// The words of the language: every word a name may be made of besides the glossary.
pub struct Dictionary {
    words: BTreeSet<String>,
}

impl Dictionary {
    /// Reads `data/dictionary.txt`: one lower-case word per line.
    pub fn read() -> Dictionary {
        let text = std::fs::read_to_string(SourceTree::guards_directory().join("data/dictionary.txt")).unwrap_or_default();
        Dictionary::parse(&text)
    }

    /// Parses dictionary text.
    pub fn parse(text: &str) -> Dictionary {
        Dictionary { words: text.lines().map(|line| line.trim().to_lowercase()).filter(|line| !line.is_empty()).collect() }
    }

    /// Whether the dictionary holds the word.
    pub fn holds(&self, word: &str) -> bool {
        self.words.contains(word)
    }
}

/// Splits a declared name into its lower-case word parts: on `_`, on case changes (an acronym run
/// is one word: `ABIError` is `abi`, `error`) and around digits, which are dropped.
pub struct NameWords;

impl NameWords {
    /// The word parts of `name`.
    pub fn of(name: &str) -> Vec<String> {
        let mut words = Vec::new();
        for piece in name.split('_') {
            let characters: Vec<char> = piece.chars().collect();
            let mut current = String::new();
            for (at, &character) in characters.iter().enumerate() {
                let lower_to_upper = character.is_uppercase() && at > 0 && characters[at - 1].is_lowercase();
                let acronym_end = character.is_uppercase() && at > 0 && characters[at - 1].is_uppercase() && characters.get(at + 1).is_some_and(|next| next.is_lowercase());
                if !character.is_alphabetic() || lower_to_upper || acronym_end {
                    if !current.is_empty() {
                        words.push(current.to_lowercase());
                        current.clear();
                    }
                }
                if character.is_alphabetic() {
                    current.push(character);
                }
            }
            if !current.is_empty() {
                words.push(current.to_lowercase());
            }
        }
        words
    }
}

/// The rules over what things are called.
pub struct NamingRules;

impl NamingRules {
    /// The item kinds whose names are checked.
    const NAMED: [ScannedKind; 8] = [ScannedKind::Type, ScannedKind::Trait, ScannedKind::Field, ScannedKind::Variant, ScannedKind::Method, ScannedKind::FreeFunction, ScannedKind::NestedFunction, ScannedKind::Module];

    /// Every declared name is made of words of the dictionary or terms of the glossary; the name of a
    /// method of a trait `impl` is the trait's, and is left out.
    pub fn undeclared_words(scan: &ItemScan, glossary: &Glossary, dictionary: &Dictionary) -> Findings {
        let mut findings = Findings::new();
        for item in scan.items().iter().filter(|item| Self::NAMED.contains(&item.kind) && !item.is_documentation_exempt()) {
            if !Self::is_made_of_words(&item.name, glossary, dictionary) {
                findings.add(&item.crate_name, format!("{}::{}", item.file.display(), item.name));
            }
        }
        findings
    }

    /// The third-column names of the glossary that no scanned item declares: an entry left behind
    /// once its name was renamed.
    pub fn undeclared_other_meanings(scan: &ItemScan, glossary: &Glossary) -> Vec<String> {
        let declared: BTreeSet<&str> = scan.items().iter().map(|item| item.name.as_str()).collect();
        glossary.other_meaning_names().iter().filter(|name| !declared.contains(name.as_str())).cloned().collect()
    }

    /// Whether every word of `name` is in the glossary or the dictionary, glossary compounds read
    /// whole, and the name is not one in which a glossary term means something else.
    fn is_made_of_words(name: &str, glossary: &Glossary, dictionary: &Dictionary) -> bool {
        if glossary.means_otherwise_in(name) {
            return false;
        }
        let mut remaining = NameWords::of(name).join("_");
        for compound in glossary.compounds() {
            remaining = remaining.replace(compound.as_str(), "_");
        }
        remaining.split('_').filter(|word| !word.is_empty()).all(|word| glossary.holds(word) || dictionary.holds(word))
    }

    /// No type name is declared twice in product code, in one crate or across crates.
    pub fn repeated_type_names(scan: &ItemScan) -> Findings {
        let types: Vec<&ScannedItem> = scan.items().iter().filter(|item| matches!(item.kind, ScannedKind::Type | ScannedKind::Trait) && item.origin == CodeOrigin::Product).collect();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for item in &types {
            *counts.entry(item.name.as_str()).or_default() += 1;
        }
        let mut findings = Findings::new();
        for item in types.iter().filter(|item| counts[item.name.as_str()] > 1) {
            findings.add(&item.crate_name, format!("{}::{}", item.file.display(), item.name));
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_on_underscores_case_acronyms_and_digits() {
        assert_eq!(NameWords::of("lower_expr_unchecked"), vec!["lower", "expr", "unchecked"]);
        assert_eq!(NameWords::of("ABIError"), vec!["abi", "error"]);
        assert_eq!(NameWords::of("TrBdf2"), vec!["tr", "bdf"]);
        assert_eq!(NameWords::of("atan2"), vec!["atan"]);
        assert_eq!(NameWords::of("HostCx"), vec!["host", "cx"]);
    }

    #[test]
    fn a_name_of_words_and_glossary_terms_passes_and_an_abbreviation_fails() {
        let glossary = Glossary::parse("glossary", "msl\tthe language\nac_stim\tan analysis source\n");
        let dictionary = Dictionary::parse("lower\nexpression\nstimulus\n");
        assert!(NamingRules::is_made_of_words("MslLower", &glossary, &dictionary));
        assert!(NamingRules::is_made_of_words("lower_ac_stim", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("lower_expr", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("x", &glossary, &dictionary));
    }

    #[test]
    #[should_panic(expected = "glossary.tsv:3: `ir the representation` is not")]
    fn a_glossary_row_without_a_tab_is_refused_with_its_line() {
        Glossary::parse("glossary.tsv", "# terms\nmsl\tthe language\nir the representation\n");
    }

    #[test]
    #[should_panic(expected = "glossary.tsv:1:")]
    fn a_glossary_row_of_four_cells_is_refused() {
        Glossary::parse("glossary.tsv", "rev\treversed order\tdiode_rev\textra\n");
    }

    #[test]
    fn a_third_column_name_no_item_declares_is_reported() {
        let glossary = Glossary::parse("glossary", "rev\treversed order\tdiode_rev, renamed_away\n");
        let scan = ItemScan::of(&SourceTree::of_sources(&[("crates/demo/src/lib.rs", "fn diode_rev() {}\n")]));
        assert_eq!(NamingRules::undeclared_other_meanings(&scan, &glossary), vec!["renamed_away".to_string()]);
    }

    #[test]
    fn a_name_in_which_a_term_means_something_else_fails() {
        let glossary = Glossary::parse("glossary", "rev\treversed order\tdiode_rev, other_rev\n");
        let dictionary = Dictionary::parse("diode\nlower\n");
        assert!(NamingRules::is_made_of_words("lower_rev", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("diode_rev", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("other_rev", &glossary, &dictionary));
    }
}
