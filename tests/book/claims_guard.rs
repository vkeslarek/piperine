//! Scope: the Book's claim guard (feature `root-e2e`, RE-03): every sentence of the Book that states
//! a value or an outcome the binary produces (Q1 (a): a code, a number with a unit or a count, a
//! verb of outcome) has a claim row in `tests/book/inventory.tsv`, and every row that says the claim
//! holds, diverges, is unraised or is disputed names a root test that cites it. This target links no
//! workspace crate: it is a repository-policy proof over the Book's text, the shape
//! `ROOT_TARGET_EXEMPTIONS` of `suite_hygiene.rs` registers.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod inventory;
}

use support::book::{Book, Sentence};
use support::inventory::{Coverage, Inventory, Kind, Row, Verdict};

/// Decides whether a sentence of the Book states a value, a record or an outcome. The three lists
/// are data: a sentence is flagged when it holds an error code, a number with a unit or a count,
/// or a verb of outcome. The detector is a heuristic: a flag needs a row (a `prose` row says the
/// flag was a false alarm), and a claim the detector misses can still be given a row by hand.
struct ClaimDetector;

impl ClaimDetector {
    /// What follows a number to make it a quantity: a unit of the Book's examples, an SI prefix
    /// written right after the number (`1k`, `10u`) or a percent sign.
    const UNITS: [&'static str; 24] = ["%", "ns", "us", "ms", "s", "V", "mV", "A", "mA", "uA", "Hz", "kHz", "MHz", "GHz", "ohm", "k", "M", "n", "u", "p", "f", "m", "x", "dB"];

    /// What follows a number to make it a count of things a run produces.
    const COUNTS: [&'static str; 22] = ["records", "record", "rows", "row", "microsteps", "microstep", "edges", "edge", "pulses", "pulse", "steps", "step", "times", "bits", "bit", "lines", "columns", "iterations", "points", "diagnostics", "streams", "files"];

    /// The verbs that say what a run, a tool or a model produces.
    const OUTCOME_VERBS: [&'static str; 18] = ["prints", "printed", "gives", "returns", "emits", "fails", "refuses", "reports", "exits", "produces", "rejects", "stops", "wraps", "settles", "crosses", "answers", "reaches", "arrives"];

    /// The words of `sentence`: runs of letters, digits, `.`, `%` and `_`, with the `.` that ends
    /// a word dropped.
    fn words(sentence: &str) -> Vec<&str> {
        sentence.split(|character: char| !(character.is_alphanumeric() || matches!(character, '.' | '%' | '_'))).map(|word| word.trim_end_matches('.')).filter(|word| !word.is_empty()).collect()
    }

    /// Whether `word` is an error code: `E`, `P` or `W` and four digits.
    fn is_code(word: &str) -> bool {
        word.len() == 5 && word.starts_with(['E', 'P', 'W']) && word[1..].chars().all(|character| character.is_ascii_digit())
    }

    /// Whether `word` is a number (digits and at most one point).
    fn is_number(word: &str) -> bool {
        !word.is_empty() && word.starts_with(|character: char| character.is_ascii_digit()) && word.chars().all(|character| character.is_ascii_digit() || character == '.')
    }

    /// Whether `words` hold a number with a unit (`693.1ns`, `63 %`) or a count (`3 records`).
    fn has_quantity(words: &[&str]) -> bool {
        words.iter().enumerate().any(|(at, word)| {
            let digits = word.trim_end_matches(|character: char| !(character.is_ascii_digit() || character == '.'));
            if !Self::is_number(digits) {
                return false;
            }
            let attached = &word[digits.len()..];
            if !attached.is_empty() {
                return Self::UNITS.contains(&attached);
            }
            words.get(at + 1).is_some_and(|next| Self::UNITS.contains(next) || Self::COUNTS.contains(next))
        })
    }

    /// Whether `sentence` states a value or an outcome.
    fn flags(sentence: &str) -> bool {
        let words = Self::words(sentence);
        words.iter().any(|word| Self::is_code(word)) || Self::has_quantity(&words) || words.iter().any(|word| Self::OUTCOME_VERBS.contains(&word.to_lowercase().as_str()))
    }

    /// The flagged sentences of `sentences`, the first of each distinct text (a sentence the Book
    /// repeats in a chapter is one claim).
    fn flagged(sentences: Vec<Sentence>) -> Vec<Sentence> {
        let mut found: Vec<Sentence> = Vec::new();
        for sentence in sentences.into_iter().filter(|sentence| Self::flags(sentence.text())) {
            if !found.contains(&sentence) {
                found.push(sentence);
            }
        }
        found
    }
}

/// The claim rules, each a method that returns what breaks it, one line per offence, so a failure
/// names the sentence and a test can hold the rule on planted input.
struct ClaimGuard<'a> {
    book: &'a Book,
    inventory: &'a Inventory,
    coverage: &'a Coverage,
}

impl ClaimGuard<'_> {
    /// The verdicts a claim row may carry; the others belong to blocks.
    const CLAIM_VERDICTS: [Verdict; 8] = [Verdict::Holds, Verdict::Diverges, Verdict::Unraised, Verdict::Disputed, Verdict::Binary, Verdict::Refused, Verdict::Prose, Verdict::Pending];

    /// The verdicts whose row names a root test that asserts the claim.
    const ASSERTED_VERDICTS: [Verdict; 4] = [Verdict::Holds, Verdict::Diverges, Verdict::Unraised, Verdict::Disputed];

    /// Every flagged sentence of every chapter has a claim row with its fingerprint. A failure
    /// prints the row to add.
    fn sentences_without_a_row(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for chapter in self.book.chapters() {
            for sentence in ClaimDetector::flagged(self.book.sentences(&chapter)) {
                let expected = Row::first_of_sentence(&chapter, &sentence);
                match self.inventory.row(&expected.id) {
                    None => offences.push(format!("{}: the sentence has no claim row; add\n{}", expected.id, expected.line())),
                    Some(row) if row.fingerprint != expected.fingerprint => offences.push(format!("{}: two sentences share the id's eight digits", expected.id)),
                    Some(_) => {}
                }
            }
        }
        offences
    }

    /// Every claim row is still a sentence of its chapter (any sentence: a row need not have been
    /// flagged) and carries a verdict a claim can have.
    fn rows_that_are_no_sentence(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.claims() {
            let chapter = row.id.split_once('~').map_or("", |(chapter, _)| chapter);
            if !self.book.chapters().iter().any(|known| known == chapter) {
                offences.push(format!("{}: the row names no chapter of the Book; remove it", row.id));
                continue;
            }
            let sentences = self.book.sentences(chapter);
            if !sentences.iter().any(|sentence| sentence.fingerprint().to_string() == row.fingerprint && Row::claim_id(chapter, sentence) == row.id) {
                offences.push(format!("{}: the chapter no longer has this sentence; remove or move the row", row.id));
            }
            if !Self::CLAIM_VERDICTS.contains(&row.verdict) {
                offences.push(format!("{}: the verdict `{}` is a block's, not a claim's", row.id, row.verdict.word()));
            }
        }
        offences
    }

    /// A claim that holds, diverges, is unraised or is disputed names a root test (`piperine/..`)
    /// that exists and cites the claim's id.
    fn asserted_claims_without_a_citing_test(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.claims().into_iter().filter(|row| Self::ASSERTED_VERDICTS.contains(&row.verdict)) {
            if !row.covered_by.starts_with("piperine/") {
                offences.push(format!("{}: a `{}` claim names a root test (`piperine/<target>::<fn>`), not `{}`", row.id, row.verdict.word(), row.covered_by));
                continue;
            }
            match self.coverage.resolve(&row.covered_by) {
                Err(message) => offences.push(format!("{}: {message}", row.id)),
                Ok(test) if !test.body().contains(&row.id) => offences.push(format!("{}: its test `{}` never cites the claim", row.id, row.covered_by)),
                Ok(_) => {}
            }
        }
        offences
    }

    /// A claim only the binary can make names a `piperine-cli` test that exists.
    fn binary_claims_without_a_command_line_test(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.claims().into_iter().filter(|row| row.verdict == Verdict::Binary) {
            if !row.covered_by.starts_with("piperine-cli/") {
                offences.push(format!("{}: a `binary` claim names a `piperine-cli` test, not `{}`", row.id, row.covered_by));
            } else if let Err(message) = self.coverage.resolve(&row.covered_by) {
                offences.push(format!("{}: {message}", row.id));
            }
        }
        offences
    }

    /// A claim about a refusal names either the `refused` block it is about, or, when the Book shows
    /// no block for it (a binding the checker refuses), a root test that cites the claim.
    fn refused_claims_without_a_refusal(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.claims().into_iter().filter(|row| row.verdict == Verdict::Refused) {
            if row.covered_by.starts_with("piperine/") {
                match self.coverage.resolve(&row.covered_by) {
                    Err(message) => offences.push(format!("{}: {message}", row.id)),
                    Ok(test) if !test.body().contains(&row.id) => offences.push(format!("{}: its test `{}` never cites the claim", row.id, row.covered_by)),
                    Ok(_) => {}
                }
                continue;
            }
            let block = self.inventory.row(&row.covered_by).filter(|block| block.kind == Kind::Block);
            if block.is_none_or(|block| block.verdict != Verdict::Refused) {
                offences.push(format!("{}: a `refused` claim names the `refused` block it is about or a root test that cites it, not `{}`", row.id, row.covered_by));
            }
        }
        offences
    }

    /// No claim row is `pending`: every sentence that states a value, a record or an outcome is settled.
    fn pending_claims(&self) -> Vec<String> {
        self.inventory.claims().into_iter().filter(|row| row.verdict == Verdict::Pending).map(|row| format!("{}: the claim is still pending", row.id)).collect()
    }

    /// Fails listing `offences`, one per line, under the rule `rule`.
    fn expect_none(rule: &str, offences: Vec<String>) {
        assert!(offences.is_empty(), "{rule}:\n  {}", offences.join("\n  "));
    }
}

/// The Book, the inventory and the tests as they stand in this repository.
struct Tree {
    book: Book,
    inventory: Inventory,
    coverage: Coverage,
}

impl Tree {
    /// The repository's Book, inventory and tests.
    fn read() -> Tree {
        Tree { book: Book::read(), inventory: Inventory::read(), coverage: Coverage::of_repository() }
    }

    /// The rules over this tree, with the chapters the registry settles.
    fn guard(&self) -> ClaimGuard<'_> {
        ClaimGuard { book: &self.book, inventory: &self.inventory, coverage: &self.coverage }
    }
}

#[test]
fn every_value_sentence_has_a_claim_row() {
    // RE-03 AC 1 and 2: each sentence that states a value or an outcome has a row
    ClaimGuard::expect_none("every value sentence has a claim row", Tree::read().guard().sentences_without_a_row());
}

#[test]
fn every_claim_row_is_a_sentence_of_its_chapter() {
    // RE-03 AC 2: a row whose sentence the Book changed or lost is found by its id
    ClaimGuard::expect_none("every claim row is a sentence of its chapter", Tree::read().guard().rows_that_are_no_sentence());
}

#[test]
fn holding_claims_name_root_tests_that_cite_them() {
    // RE-03 AC 1: the test of a claim cites the claim, so the sentence is found from the test
    ClaimGuard::expect_none("an asserted claim names a root test that cites it", Tree::read().guard().asserted_claims_without_a_citing_test());
}

#[test]
fn binary_claims_name_command_line_tests() {
    // Q1 (a): a claim only the binary can make is held in `piperine-cli`
    let tree = Tree::read();
    let guard = tree.guard();
    ClaimGuard::expect_none("a binary claim names a command line test", guard.binary_claims_without_a_command_line_test());
    ClaimGuard::expect_none("a refused claim names its refusal", guard.refused_claims_without_a_refusal());
}

#[test]
fn no_claim_row_is_pending() {
    // RE-03 AC 2: at close every claim row has a verdict, in every chapter
    ClaimGuard::expect_none("no claim row is pending", Tree::read().guard().pending_claims());
}

#[test]
fn the_detector_flags_a_value_a_code_and_an_outcome() {
    // Q1 (a): a code, a quantity and a verb of outcome are flagged; a rule is not
    for flagged in [
        "The checker gives E0310 on a bare port.",
        "The monitor crosses at 693.1 ns.",
        "The capacitor holds 63 % of the step.",
        "The sweep has 21 rows.",
        "The resistor is 1k.",
        "The command prints one line.",
        "The run stops.",
    ] {
        assert!(ClaimDetector::flags(flagged), "a claim: {flagged}");
    }
    for plain in ["A module has ports and a body.", "Chapter 1", "A port named E010 is a name.", "Each of the 3 modules is a unit.", "Stay with the 2 of them."] {
        assert!(!ClaimDetector::flags(plain), "no claim: {plain}");
    }
    let twice = vec![Sentence::new("It prints E0001."), Sentence::new("Nothing here."), Sentence::new("It prints E0001.")];
    assert_eq!(ClaimDetector::flagged(twice).len(), 1, "a repeated sentence is one claim");
}

/// A planted Book, the tests its claims name and the inventory of it: the rules are held on these.
struct Planted;

impl Planted {
    /// A chapter `demo` of four claims and one rule.
    fn book() -> Book {
        Book::from_html(
            "<section id=\"demo\" class=\"chapter\"><p>It prints E0001. It takes 5 ns. A rule holds. It fails with E0002. \
             It exits with E0003.</p><pre data-lang=\"msl\">module A() {}</pre></section>"
                .to_string(),
        )
    }

    /// The tests the planted rows name, as a tree on disk; the first cites the claim `cited`.
    fn tests(cited: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a temporary tree");
        let write = |path: &str, text: &str| {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            std::fs::write(path, text).expect("a planted file");
        };
        write("Cargo.toml", "[package]\nname = \"demo\"\n\n[[test]]\nname = \"book_demo\"\npath = \"tests/book/demo.rs\"\n");
        write("tests/book/demo.rs", &format!("#[test]\nfn cites() {{\n    // {cited}\n    let x = 1;\n}}\n\n#[test]\nfn forgets() {{\n    let x = 1;\n}}\n"));
        write("crates/piperine-cli/tests/cli.rs", "#[test]\nfn prints() {\n}\n");
        root
    }

    /// The first rows of `book`: one pending claim per flagged sentence.
    fn rows(book: &Book) -> Vec<Row> {
        ClaimDetector::flagged(book.sentences("demo")).iter().map(|sentence| Row::first_of_sentence("demo", sentence)).collect()
    }

    /// The inventory that holds `rows`.
    fn inventory(rows: &[Row]) -> Inventory {
        Inventory::parse(&rows.iter().map(Row::line).collect::<Vec<_>>().join("\n"))
    }
}

#[test]
fn the_claim_guard_names_a_sentence_without_a_row() {
    // RE-03 AC 2: each rule on a planted tree names the claim it breaks
    let book = Planted::book();
    let first = Planted::rows(&book);
    assert_eq!(first.len(), 4, "the planted chapter has four claims: {first:?}");
    let first_id = first[0].id.clone();
    let tests = Planted::tests(&first_id);
    let coverage = Coverage::at(tests.path());
    let judged = |rows: &[Row], rule: &dyn Fn(&ClaimGuard) -> Vec<String>| {
        let inventory = Planted::inventory(rows);
        rule(&ClaimGuard { book: &book, inventory: &inventory, coverage: &coverage })
    };

    let found = judged(&first, &|guard| guard.sentences_without_a_row());
    assert!(found.is_empty(), "a complete planted inventory breaks no rule: {found:?}");
    let found = judged(&first[1..], &|guard| guard.sentences_without_a_row());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with(&format!("{first_id}: the sentence has no claim row; add\nclaim\t{first_id}\t")), "the offence names the claim and prints its row: {}", found[0]);

    let mut lost = first.clone();
    lost.push(Row { id: "demo~00000000".to_string(), fingerprint: "0000000000000000".to_string(), ..first[0].clone() });
    lost.push(Row { id: "nowhere~00000000".to_string(), ..first[0].clone() });
    lost[1].verdict = Verdict::Run;
    let found = judged(&lost, &|guard| guard.rows_that_are_no_sentence());
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found.iter().any(|line| line.starts_with("demo~00000000: the chapter no longer has this sentence")), "{found:?}");
    assert!(found.iter().any(|line| line.starts_with("nowhere~00000000: the row names no chapter")), "{found:?}");
    assert!(found.iter().any(|line| line.contains("`run` is a block's")), "{found:?}");

    let mut asserted = first.clone();
    asserted[0].verdict = Verdict::Holds;
    asserted[0].covered_by = "piperine/book_demo::cites".to_string();
    asserted[1].verdict = Verdict::Diverges;
    asserted[1].covered_by = "piperine/book_demo::forgets".to_string();
    asserted[2].verdict = Verdict::Unraised;
    asserted[2].covered_by = "piperine-cli/cli::prints".to_string();
    asserted[3].verdict = Verdict::Disputed;
    asserted[3].covered_by = "piperine/book_demo::no_such_test".to_string();
    let found = judged(&asserted, &|guard| guard.asserted_claims_without_a_citing_test());
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].contains("never cites the claim") && found[0].starts_with(&asserted[1].id), "{found:?}");
    assert!(found[1].contains("names a root test") && found[1].starts_with(&asserted[2].id), "{found:?}");
    assert!(found[2].contains("no top-level `fn no_such_test(`") && found[2].starts_with(&asserted[3].id), "{found:?}");

    let mut binary = first.clone();
    binary[0].verdict = Verdict::Binary;
    binary[0].covered_by = "piperine-cli/cli::prints".to_string();
    binary[1].verdict = Verdict::Binary;
    binary[1].covered_by = "piperine/book_demo::cites".to_string();
    binary[2].verdict = Verdict::Binary;
    binary[2].covered_by = "piperine-cli/cli::no_such_test".to_string();
    let found = judged(&binary, &|guard| guard.binary_claims_without_a_command_line_test());
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with(&binary[1].id) && found[1].starts_with(&binary[2].id), "{found:?}");

    let mut refused = first.clone();
    refused[0].verdict = Verdict::Refused;
    refused[0].covered_by = "demo#1".to_string();
    refused[1].verdict = Verdict::Refused;
    refused[1].covered_by = "demo#2".to_string();
    let block = Row { kind: Kind::Block, id: "demo#1".to_string(), verdict: Verdict::Refused, ..first[0].clone() };
    refused.push(block);
    let found = judged(&refused, &|guard| guard.refused_claims_without_a_refusal());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with(&refused[1].id), "{found:?}");

    let mut by_test = first.clone();
    by_test[0].verdict = Verdict::Refused;
    by_test[0].covered_by = "piperine/book_demo::cites".to_string();
    by_test[1].verdict = Verdict::Refused;
    by_test[1].covered_by = "piperine/book_demo::forgets".to_string();
    let found = judged(&by_test, &|guard| guard.refused_claims_without_a_refusal());
    assert_eq!(found.len(), 1, "a block-less refusal names a root test that cites it: {found:?}");
    assert!(found[0].starts_with(&by_test[1].id) && found[0].contains("never cites the claim"), "{found:?}");

    let found = judged(&first, &|guard| guard.pending_claims());
    assert_eq!(found.len(), 4, "{found:?}");
    assert!(found[0].starts_with(&first_id) && found[0].ends_with("the claim is still pending"), "{found:?}");
}
