//! Scope: the Book's inventory guard (feature `root-e2e`, RE-01 and RE-02): the Book,
//! `docs/book/content.html`, is read into chapters, blocks and sentences, and every block has a
//! row of `tests/book/inventory.tsv` naming the test that holds it. This target links no workspace
//! crate: it is a repository-policy proof over the Book's text, the shape `ROOT_TARGET_EXEMPTIONS`
//! of `suite_hygiene.rs` registers.

use std::collections::BTreeMap;

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod gaps;
    pub mod inventory;
}

use support::book::{BlockId, Book, Fingerprint};
use support::gaps::Gaps;
use support::inventory::{Coverage, Inventory, Kind, Row, Verdict};

/// The chapters whose rows a chapter task has settled: none of their rows may be `pending`. A chapter
/// task adds its chapter here; the closing task replaces the registry with the strict rules.
const SETTLED_CHAPTERS: [&str; 6] = ["errors", "welcome", "from-rust", "values", "natures", "modules"];

/// The inventory rules, each a method that returns what breaks it, one line per offence, so a
/// failure names the block and a test can hold the rule on planted input.
struct Guard<'a> {
    book: &'a Book,
    inventory: &'a Inventory,
    coverage: &'a Coverage,
    gaps: &'a Gaps,
    settled: &'a [&'a str],
}

impl Guard<'_> {
    /// The verdicts whose rows `GAPS.md` section 11 lists: the Book's claim the binary does not meet.
    const LISTED_VERDICTS: [Verdict; 4] = [Verdict::Refused, Verdict::Diverges, Verdict::Unraised, Verdict::Disputed];

    /// The row of `block`'s chapter and position, as it would be written.
    fn corrected(&self, id: &BlockId) -> String {
        Row::first_of(self.book.block(id)).line()
    }

    /// Every block of the Book has one block row with its fingerprint; a row names a block that
    /// the Book has. A failure prints the corrected row, ready to paste.
    fn rows_against_blocks(&self) -> Vec<String> {
        let mut offences = Vec::new();
        let rows = self.inventory.blocks();
        for block in self.book.blocks() {
            let id = block.id().to_string();
            let found: Vec<&&Row> = rows.iter().filter(|row| row.id == id).collect();
            match found.as_slice() {
                [] => offences.push(format!("{id}: the block has no row; add\n{}", self.corrected(block.id()))),
                [row] if row.fingerprint != block.fingerprint().to_string() => offences.push(format!("{id}: the block changed or moved (fingerprint {} in its row, {} in the Book); the row is now\n{}", row.fingerprint, block.fingerprint(), self.corrected(block.id()))),
                [_] => {}
                _ => offences.push(format!("{id}: the block has {} rows", found.len())),
            }
        }
        for row in rows {
            if BlockId::parse(&row.id).is_none_or(|id| !self.book.blocks().iter().any(|block| *block.id() == id)) {
                offences.push(format!("{}: a row names a block the Book does not have; remove it", row.id));
            }
        }
        offences
    }

    /// Every `covered_by` names a test that exists: a function of a target, or, for a fragment, a
    /// block row that runs or is refused.
    fn tests_that_do_not_exist(&self) -> Vec<String> {
        let mut offences = Vec::new();
        // a `refused` claim names the block it is about, which `book_claims` holds
        let named_tests = |row: &&Row| row.covered_by != "-" && !(row.kind == Kind::Claim && row.verdict == Verdict::Refused);
        for row in self.inventory.rows().iter().filter(named_tests) {
            if row.verdict == Verdict::Fragment {
                let whole = self.inventory.row(&row.covered_by);
                if whole.is_none_or(|whole| !matches!(whole.verdict, Verdict::Run | Verdict::Refused)) {
                    offences.push(format!("{}: a fragment names `{}`, which is not a `run` or `refused` block row", row.id, row.covered_by));
                }
            } else if let Err(message) = self.coverage.resolve(&row.covered_by) {
                offences.push(format!("{}: {message}", row.id));
            }
        }
        offences
    }

    /// Every block row's verdict fits the block's language and attributes.
    fn verdicts_that_do_not_fit(&self) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.blocks() {
            let Some(block) = BlockId::parse(&row.id).and_then(|id| self.book.blocks().iter().find(|block| *block.id() == id)) else { continue };
            if !row.verdict.fits(block) {
                offences.push(format!("{}: the verdict `{}` does not fit a `{}` block", row.id, row.verdict.word(), block.language()));
            }
        }
        offences
    }

    /// The test of a `run` row names the row and runs the model.
    fn run_tests_that_do_not_run(&self) -> Vec<String> {
        self.rows_whose_test_lacks(Verdict::Run, ".run(", "run the model with `.run(`")
    }

    /// The test of a `refused` row names the row and asserts the refusal.
    fn refusals_that_are_not_expected(&self) -> Vec<String> {
        self.rows_whose_test_lacks(Verdict::Refused, "Refusal::expect(", "assert the refusal with `Refusal::expect(`")
    }

    /// The rows of `verdict` whose covering test does not name the row's id or does not contain
    /// `call`; `what` says what the test must do.
    fn rows_whose_test_lacks(&self, verdict: Verdict, call: &str, what: &str) -> Vec<String> {
        let mut offences = Vec::new();
        for row in self.inventory.blocks().into_iter().filter(|row| row.verdict == verdict) {
            let Ok(test) = self.coverage.resolve(&row.covered_by) else { continue };
            if !test.body().contains(&row.id) {
                offences.push(format!("{}: its test `{}` never names the row", row.id, row.covered_by));
            }
            if !test.body().contains(call) {
                offences.push(format!("{}: its test `{}` does not {what}", row.id, row.covered_by));
            }
        }
        offences
    }

    /// The blocks one `cli-file` path joins into a model are held by one test: their `run` and
    /// `refused` rows name the same one.
    fn models_split_across_tests(&self) -> Vec<String> {
        let mut by_file: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
        for block in self.book.blocks() {
            let (Some(path), Some(row)) = (block.file(), self.inventory.row(&block.id().to_string())) else { continue };
            if matches!(row.verdict, Verdict::Run | Verdict::Refused) {
                by_file.entry(path).or_default().push(row);
            }
        }
        by_file
            .into_iter()
            .filter(|(_, rows)| rows.iter().any(|row| row.covered_by != rows[0].covered_by))
            .map(|(path, rows)| format!("{path}: the blocks {} join one model but name different tests", rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>().join(", ")))
            .collect()
    }

    /// `GAPS.md` section 11 and the inventory agree: every block or claim row that is `refused`,
    /// `diverges`, `unraised` or `disputed` under a gap has one row of the section with the same gap
    /// and an owner, and the section has no other row. A `refused` row with no gap is a refusal the
    /// Book teaches (a faulty model shown to be refused) and has no row; any other verdict needs its
    /// gap. The gap is an entry of the file.
    fn section_eleven_disagreements(&self) -> Vec<String> {
        let mut offences: Vec<String> = self.gaps.malformed_rows().into_iter().map(|line| format!("section 11 has a line that is not a four-cell row: {line}")).collect();
        let section = self.gaps.rows();
        let listed: Vec<&Row> = self.inventory.rows().iter().filter(|row| Self::LISTED_VERDICTS.contains(&row.verdict) && (row.gap != "-" || row.verdict != Verdict::Refused)).collect();
        for row in &listed {
            if row.gap == "-" {
                offences.push(format!("{}: a `{}` row names its gap; the row has none", row.id, row.verdict.word()));
                continue;
            }
            match section.iter().filter(|line| line.book == row.id).collect::<Vec<_>>().as_slice() {
                [] => offences.push(format!("{}: section 11 has no row; add\n| {} | {} | <owner> | <what the binary does today> |", row.id, row.id, row.gap)),
                [line] => {
                    if line.gap != row.gap {
                        offences.push(format!("{}: the row says gap {}, section 11 says {}", row.id, row.gap, line.gap));
                    }
                    if line.owner.is_empty() || line.today.is_empty() {
                        offences.push(format!("{}: section 11 gives no owner or no `Today`", row.id));
                    }
                }
                _ => offences.push(format!("{}: section 11 has the row twice", row.id)),
            }
            if !self.gaps.has_entry(&row.gap) {
                offences.push(format!("{}: the gap {} is no entry of GAPS.md", row.id, row.gap));
            }
        }
        for line in &section {
            if !listed.iter().any(|row| row.id == line.book) {
                offences.push(format!("{}: section 11 lists a row the inventory does not hold as refused, diverging, unraised or disputed; remove it", line.book));
            }
        }
        offences
    }

    /// Fails listing `offences`, one per line, under the rule `rule`.
    fn expect_none(rule: &str, offences: Vec<String>) {
        assert!(offences.is_empty(), "{rule}:\n  {}", offences.join("\n  "));
    }

    /// A settled chapter has no `pending` block row.
    fn pending_in_settled_chapters(&self) -> Vec<String> {
        let in_settled = |row: &&Row| self.settled.iter().any(|chapter| BlockId::parse(&row.id).is_some_and(|id| id.chapter == *chapter));
        self.inventory.blocks().into_iter().filter(|row| row.verdict == Verdict::Pending).filter(in_settled).map(|row| format!("{}: the chapter is settled and the row is still pending", row.id)).collect()
    }
}

/// The Book, the inventory and the tests as they stand in this repository.
struct Tree {
    book: Book,
    inventory: Inventory,
    coverage: Coverage,
    gaps: Gaps,
}

impl Tree {
    /// The repository's Book, inventory and tests.
    fn read() -> Tree {
        Tree { book: Book::read(), inventory: Inventory::read(), coverage: Coverage::of_repository(), gaps: Gaps::read() }
    }

    /// The rules over this tree, with the chapters the registry settles.
    fn guard(&self) -> Guard<'_> {
        Guard { book: &self.book, inventory: &self.inventory, coverage: &self.coverage, gaps: &self.gaps, settled: &SETTLED_CHAPTERS }
    }
}

#[test]
fn the_book_reads_every_block() {
    // RE-01 AC 1: the Book has 18 chapters and 214 code blocks (a mermaid diagram is not one), 99 of
    // them `msl`; every block has a distinct id that reads back, and every chapter has prose
    let book = Book::read();
    assert_eq!(book.chapters().len(), 18, "the chapters of the Book: {:?}", book.chapters());
    assert_eq!(book.blocks().len(), 214, "the code blocks of the Book");
    assert_eq!(book.blocks().iter().filter(|block| block.language() == "msl").count(), 99, "the `msl` blocks of the Book");
    let mut seen = Vec::new();
    for block in book.blocks() {
        let id = block.id().to_string();
        assert_eq!(BlockId::parse(&id).as_ref(), Some(block.id()), "{id} reads back");
        assert!(!seen.contains(&id), "{id} names two blocks");
        seen.push(id);
    }
    for chapter in book.chapters() {
        assert!(!book.sentences(&chapter).is_empty(), "the chapter `{chapter}` has no prose");
        let indices: Vec<usize> = book.blocks().iter().filter(|block| block.id().chapter == chapter).map(|block| block.id().index).collect();
        assert_eq!(indices, (1..=indices.len()).collect::<Vec<_>>(), "the blocks of `{chapter}` are numbered from 1 with no gap");
    }
    let welcome = book.sentences("welcome");
    assert_eq!(welcome[0].text(), "Chapter 1", "the first element of a chapter is its number");
    let goal = "By the end of this chapter you have created a project, run an RC filter from the command line, read its results, and made a model tell you what it is doing.";
    assert!(welcome.iter().any(|sentence| sentence.text() == goal), "the goal of the first chapter is one sentence");
    assert_eq!(Fingerprint::of("a").to_string(), "af63dc4c8601ec8c", "FNV-1a 64 of one byte");
    assert_eq!(welcome[0].fingerprint().to_string().len(), 16, "a fingerprint is sixteen hex digits");
    assert_eq!(book.blocks()[0].fingerprint(), Fingerprint::of(book.blocks()[0].text()), "a block's fingerprint is its text's");
}

#[test]
fn a_marked_file_joins_its_blocks() {
    // RE-02 AC 4: `clipper.msl` is the two blocks the Book marks with it, joined in order, each
    // followed by a newline
    let book = Book::read();
    let marked: Vec<_> = book.blocks().iter().filter(|block| block.file() == Some("clipper.msl")).collect();
    assert_eq!(marked.len(), 2, "the Book marks two blocks `clipper.msl`");
    let file = book.file("clipper.msl", marked[1].id());
    assert_eq!(file, format!("{}\n{}\n", marked[0].text(), marked[1].text()));
    assert!(file.starts_with("module Resistor("), "the file opens with the Book's first block");
    assert_eq!(book.file("clipper.msl", marked[0].id()), format!("{}\n", marked[0].text()), "at the first block the file is the first block");
}

#[test]
fn every_block_of_the_book_has_one_row() {
    // RE-01 AC 1 and 3: one row per block, with the block's fingerprint, none for a block the Book lost
    Guard::expect_none("every block has one row", Tree::read().guard().rows_against_blocks());
}

#[test]
fn every_row_names_an_existing_test() {
    // RE-02 AC 1: a row's `covered_by` is a function of a target that exists, or a block that runs
    Guard::expect_none("every row names an existing test", Tree::read().guard().tests_that_do_not_exist());
}

#[test]
fn a_verdict_fits_its_block() {
    // RE-01 AC 2: a verdict says what a block of that language and kind can be
    Guard::expect_none("every verdict fits its block", Tree::read().guard().verdicts_that_do_not_fit());
}

#[test]
fn run_rows_name_tests_that_run_their_model() {
    // RE-02 AC 1: the test of a `run` row names the row and runs the model
    Guard::expect_none("a `run` row's test runs its model", Tree::read().guard().run_tests_that_do_not_run());
}

#[test]
fn refused_rows_name_tests_that_expect_their_refusal() {
    // RE-02 AC 3: the test of a `refused` row names the row and asserts the refusal
    Guard::expect_none("a `refused` row's test expects the refusal", Tree::read().guard().refusals_that_are_not_expected());
}

#[test]
fn a_model_across_blocks_names_one_test() {
    // spec edge 1: the blocks one `cli-file` path joins are one model, one test
    Guard::expect_none("a model across blocks is one test", Tree::read().guard().models_split_across_tests());
}

#[test]
fn refused_rows_and_gaps_section_11_agree() {
    // RE-02 AC 2: every block or claim the binary refuses, does not meet, never raises or the spec
    // disputes is listed in `GAPS.md` section 11 under its gap, and the section lists nothing else
    Guard::expect_none("section 11 of GAPS.md and the inventory agree", Tree::read().guard().section_eleven_disagreements());
}

#[test]
fn settled_chapters_have_no_pending_row() {
    // RE-01 AC 4: a chapter its task settled keeps no row pending
    Guard::expect_none("settled chapters have no pending row", Tree::read().guard().pending_in_settled_chapters());
}

/// A planted Book, the tests it names and the inventory of it: the rules are held on these.
struct Planted;

impl Planted {
    /// A chapter `demo` of seven blocks: a model, a listing, a model to refuse, two blocks of one
    /// file, a diagnostic and a command.
    fn book() -> Book {
        Book::from_html(
            "<section id=\"demo\" class=\"chapter\"><p>Prose.</p>\n\
             <pre data-lang=\"msl\">module A() {}</pre>\n\
             <pre data-lang=\"text\" data-cli-output=\"piperine run a.msl\">out</pre>\n\
             <pre data-lang=\"msl\">module B() {}</pre>\n\
             <!-- cli-file: m.msl -->\n<pre data-lang=\"msl\">module C() {}</pre>\n\
             <!-- cli-file: m.msl -->\n<pre data-lang=\"msl\">module D() {}</pre>\n\
             <pre data-lang=\"diag\">E0001</pre>\n\
             <pre data-lang=\"sh\">piperine new x</pre></section>"
                .to_string(),
        )
    }

    /// The tests the planted rows name, as a tree on disk: a root target declared in the manifest, and
    /// the CLI target that holds listings.
    fn tests() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a temporary tree");
        let write = |path: &str, text: &str| {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            std::fs::write(path, text).expect("a planted file");
        };
        write("Cargo.toml", "[package]\nname = \"demo\"\n\n[[test]]\nname = \"book_demo\"\npath = \"tests/book/demo.rs\"\n");
        write(
            "tests/book/demo.rs",
            "#[test]\nfn runs_demo() {\n    // demo#1 demo#4 demo#5\n    model.run(overrides);\n}\n\n#[test]\nfn refuses_demo() {\n    // demo#3\n    Refusal::expect(result, \"demo#3\", code, message);\n}\n\n#[test]\nfn says_nothing() {\n    let x = 1;\n}\n",
        );
        write("crates/piperine-cli/tests/book.rs", "#[test]\nfn every_cli_listing_of_the_book_is_what_the_binary_prints() {\n}\n");
        root
    }

    /// The rows of `book` as a settled chapter would write them: the first of each, then the
    /// verdicts the planted tests hold.
    fn rows(book: &Book) -> Vec<Row> {
        let mut rows: Vec<Row> = book.blocks().iter().map(Row::first_of).collect();
        let settle = |row: &mut Row, verdict: Verdict, covered_by: &str| {
            row.verdict = verdict;
            row.covered_by = covered_by.to_string();
        };
        settle(&mut rows[0], Verdict::Run, "piperine/book_demo::runs_demo");
        settle(&mut rows[2], Verdict::Refused, "piperine/book_demo::refuses_demo");
        rows[2].gap = "L7".to_string();
        settle(&mut rows[3], Verdict::Run, "piperine/book_demo::runs_demo");
        settle(&mut rows[4], Verdict::Run, "piperine/book_demo::runs_demo");
        settle(&mut rows[5], Verdict::Holds, "piperine/book_demo::runs_demo");
        rows
    }

    /// A `GAPS.md` whose section 11 lists the planted refusal `demo#3` under the gap `L7` and holds `L7` as an entry.
    fn gaps() -> Gaps {
        Gaps::from_text("| L7 | a generic module | §8 | `generic module` | L |\n\n## 11. The Book against the binary\n\n| Book | Gap | Owner | Today |\n| ---- | --- | ----- | ----- |\n| demo#3 | L7 | `generics` | `P0001`: a generic module |\n")
    }

    /// The inventory that holds `rows`.
    fn inventory(rows: &[Row]) -> Inventory {
        Inventory::parse(&rows.iter().map(Row::line).collect::<Vec<_>>().join("\n"))
    }
}

#[test]
fn the_guard_names_a_block_without_a_row_and_a_row_naming_no_test() {
    // RE-01 AC 3 and RE-02 AC 1, 3: each rule on a planted tree names the block it breaks
    let book = Planted::book();
    let tests = Planted::tests();
    let coverage = Coverage::at(tests.path());
    let clean = Planted::rows(&book);
    let inventory = Planted::inventory(&clean);
    let gaps = Planted::gaps();
    let guard = Guard { book: &book, inventory: &inventory, coverage: &coverage, gaps: &gaps, settled: &["demo"] };
    for offences in [guard.rows_against_blocks(), guard.tests_that_do_not_exist(), guard.verdicts_that_do_not_fit(), guard.run_tests_that_do_not_run(), guard.refusals_that_are_not_expected(), guard.models_split_across_tests(), guard.pending_in_settled_chapters(), guard.section_eleven_disagreements()] {
        assert!(offences.is_empty(), "a clean planted tree breaks no rule: {offences:?}");
    }

    let broken = |change: &dyn Fn(&mut Vec<Row>)| -> Vec<Row> {
        let mut rows = clean.clone();
        change(&mut rows);
        rows
    };
    let judged = |rows: &[Row], rule: &dyn Fn(&Guard) -> Vec<String>| {
        let inventory = Planted::inventory(rows);
        rule(&Guard { book: &book, inventory: &inventory, coverage: &coverage, gaps: &gaps, settled: &["demo"] })
    };

    let without = broken(&|rows| {
        rows.remove(0);
    });
    let found = judged(&without, &|guard| guard.rows_against_blocks());
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("demo#1: the block has no row; add\nblock\tdemo#1\t"), "the offence names the block and prints its row: {}", found[0]);

    let moved = broken(&|rows| rows[1].fingerprint = "0000000000000000".to_string());
    let found = judged(&moved, &|guard| guard.rows_against_blocks());
    assert!(found.len() == 1 && found[0].starts_with("demo#2: the block changed or moved"), "{found:?}");

    let lost = broken(&|rows| rows.push(Row { id: "demo#9".to_string(), ..rows[0].clone() }));
    let found = judged(&lost, &|guard| guard.rows_against_blocks());
    assert_eq!(found, vec!["demo#9: a row names a block the Book does not have; remove it"]);

    let nameless = broken(&|rows| {
        rows[0].covered_by = "piperine/book_demo::no_such_test".to_string();
        rows[1].covered_by = "piperine-cli/no_such_target::x".to_string();
    });
    let found = judged(&nameless, &|guard| guard.tests_that_do_not_exist());
    assert!(found.len() == 2 && found[0].starts_with("demo#1:") && found[0].contains("no top-level `fn no_such_test(`") && found[1].starts_with("demo#2:"), "{found:?}");

    let fragment = broken(&|rows| {
        rows[3].verdict = Verdict::Fragment;
        rows[3].covered_by = "demo#7".to_string();
    });
    let found = judged(&fragment, &|guard| guard.tests_that_do_not_exist());
    assert_eq!(found, vec!["demo#4: a fragment names `demo#7`, which is not a `run` or `refused` block row"]);

    let misfit = broken(&|rows| {
        rows[6].verdict = Verdict::Run;
        rows[5].verdict = Verdict::Shown;
    });
    let found = judged(&misfit, &|guard| guard.verdicts_that_do_not_fit());
    assert_eq!(found, vec!["demo#6: the verdict `shown` does not fit a `diag` block", "demo#7: the verdict `run` does not fit a `sh` block"]);

    let idle = broken(&|rows| {
        rows[0].covered_by = "piperine/book_demo::says_nothing".to_string();
        rows[2].covered_by = "piperine/book_demo::says_nothing".to_string();
    });
    let found = judged(&idle, &|guard| guard.run_tests_that_do_not_run());
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[0].starts_with("demo#1: its test") && found[1].starts_with("demo#1:") && found[1].contains("`.run(`"));
    let found = judged(&idle, &|guard| guard.refusals_that_are_not_expected());
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found[1].starts_with("demo#3:") && found[1].contains("`Refusal::expect(`"));

    let split = broken(&|rows| rows[4].covered_by = "piperine/book_demo::refuses_demo".to_string());
    let found = judged(&split, &|guard| guard.models_split_across_tests());
    assert_eq!(found, vec!["m.msl: the blocks demo#4, demo#5 join one model but name different tests"]);

    let pending = broken(&|rows| rows[0].verdict = Verdict::Pending);
    let found = judged(&pending, &|guard| guard.pending_in_settled_chapters());
    assert_eq!(found, vec!["demo#1: the chapter is settled and the row is still pending"]);
}

#[test]
fn the_gaps_guard_names_a_missing_and_an_extra_row() {
    // RE-02 AC 2: section 11 of GAPS.md against the inventory, on planted text: a row with no line, a
    // line with no row, a wrong gap, a gap that is no entry, a missing owner, a row with no gap
    let book = Planted::book();
    let tests = Planted::tests();
    let coverage = Coverage::at(tests.path());
    let clean = Planted::rows(&book);
    let inventory = Planted::inventory(&clean);
    let judged = |inventory: &Inventory, gaps: &Gaps| Guard { book: &book, inventory, coverage: &coverage, gaps, settled: &[] }.section_eleven_disagreements();
    let section = |lines: &str| Gaps::from_text(&format!("| L7 | a generic module | §8 | x | L |\n\n## 11. The Book against the binary\n\n| Book | Gap | Owner | Today |\n| ---- | --- | ----- | ----- |\n{lines}"));

    assert!(judged(&inventory, &Planted::gaps()).is_empty(), "the planted section agrees with the planted inventory");

    let found = judged(&inventory, &section(""));
    assert_eq!(found, vec!["demo#3: section 11 has no row; add\n| demo#3 | L7 | <owner> | <what the binary does today> |"]);

    let found = judged(&inventory, &section("| demo#3 | L7 | `generics` | refused |\n| demo#9 | L7 | `generics` | refused |\n"));
    assert_eq!(found, vec!["demo#9: section 11 lists a row the inventory does not hold as refused, diverging, unraised or disputed; remove it"]);

    let found = judged(&inventory, &section("| demo#3 | L8 | `generics` | refused |\n"));
    assert_eq!(found, vec!["demo#3: the row says gap L7, section 11 says L8"]);

    let mut other = clean.clone();
    other[2].gap = "L99".to_string();
    let found = judged(&Planted::inventory(&other), &section("| demo#3 | L99 | `generics` | refused |\n"));
    assert_eq!(found, vec!["demo#3: the gap L99 is no entry of GAPS.md"]);

    let found = judged(&inventory, &section("| demo#3 | L7 | | refused |\n"));
    assert_eq!(found, vec!["demo#3: section 11 gives no owner or no `Today`"]);

    let mut nameless = clean.clone();
    nameless[5].verdict = Verdict::Diverges;
    let found = judged(&Planted::inventory(&nameless), &section("| demo#3 | L7 | `generics` | refused |\n"));
    assert_eq!(found, vec!["demo#6: a `diverges` row names its gap; the row has none"]);

    let mut taught = clean.clone();
    taught[2].gap = "-".to_string();
    let found = judged(&Planted::inventory(&taught), &section(""));
    assert!(found.is_empty(), "a refusal the Book teaches has no gap and no line: {found:?}");

    let found = judged(&inventory, &section("| demo#3 | L7 | `generics` | refused |\n| broken |\n"));
    assert_eq!(found, vec!["section 11 has a line that is not a four-cell row: | broken |"]);
}
