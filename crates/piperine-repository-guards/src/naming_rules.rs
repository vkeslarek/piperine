//! The naming rules (CC-05): a declared name is made of words, or of terms of the domain glossary,
//! and a bare type name is declared once.

use std::collections::{BTreeMap, BTreeSet};

use crate::allowance::Findings;
use crate::item_scan::{ScannedKind, ItemScan, CodeOrigin, ScannedItem};
use crate::source_tree::SourceTree;

/// The terms the domain spells in short form: names a reader already knows (`msl`, `abi`, `dc`).
pub struct Glossary {
    terms: BTreeSet<String>,
}

impl Glossary {
    /// Reads `data/glossary.tsv`: one `term<TAB>meaning` row per line.
    pub fn read() -> Glossary {
        let text = std::fs::read_to_string(SourceTree::guards_directory().join("data/glossary.tsv")).unwrap_or_default();
        Glossary::parse(&text)
    }

    /// Parses glossary text; `#` lines and blank lines are comments.
    pub fn parse(text: &str) -> Glossary {
        let terms = text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')).filter_map(|line| line.split('\t').next()).map(|term| term.trim().to_lowercase()).collect();
        Glossary { terms }
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

    /// Whether every word of `name` is in the glossary or the dictionary, glossary compounds read whole.
    fn is_made_of_words(name: &str, glossary: &Glossary, dictionary: &Dictionary) -> bool {
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
        let glossary = Glossary::parse("msl\tthe language\nac_stim\tan analysis source\n");
        let dictionary = Dictionary::parse("lower\nexpression\nstimulus\n");
        assert!(NamingRules::is_made_of_words("MslLower", &glossary, &dictionary));
        assert!(NamingRules::is_made_of_words("lower_ac_stim", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("lower_expr", &glossary, &dictionary));
        assert!(!NamingRules::is_made_of_words("x", &glossary, &dictionary));
    }
}
