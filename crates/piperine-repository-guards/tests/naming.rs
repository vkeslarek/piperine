//! The naming guards (CC-05, CC-10 AC 4): every declared name is made of words of the dictionary or
//! terms of the domain glossary, and no bare type name is declared twice in product code. Ratchets
//! over `allowances/<crate>/<guard>.tsv`.

use piperine_repository_guards::{Dictionary, Glossary, ItemScan, NamingRules, SourceTree};

#[test]
fn every_declared_name_is_words_or_glossary() {
    let scan = ItemScan::of(&SourceTree::of_workspace());
    let glossary = Glossary::read();
    let left_behind = NamingRules::undeclared_other_meanings(&scan, &glossary);
    assert!(left_behind.is_empty(), "data/glossary.tsv names, in a third column, names no item declares (delete them):\n{}", left_behind.join("\n"));
    let findings = NamingRules::undeclared_words(&scan, &glossary, &Dictionary::read());
    findings.judge("undeclared_words").assert_clean("undeclared_words");
}

#[test]
fn no_bare_type_name_is_declared_twice() {
    let scan = ItemScan::of(&SourceTree::of_workspace());
    NamingRules::repeated_type_names(&scan).judge("repeated_type_names").assert_clean("repeated_type_names");
}
