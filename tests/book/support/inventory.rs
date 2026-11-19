//! The Book's inventory, `tests/book/inventory.tsv`: one row per code block of the Book and one
//! per sentence that states a value or an outcome, each with the verdict the Book's claim has and
//! the test that holds it. Rows are tab-separated, one per line; a line starting with `#` is a
//! comment. The columns are `kind id fingerprint verdict covered_by gap text`.

use std::path::{Path, PathBuf};

use super::book::{Block, BlockId, Book, Sentence};

/// What a row is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A code block of the Book, named `<chapter>#<n>`.
    Block,
    /// A sentence of the Book's prose, named `<chapter>~<first 8 hex of its fingerprint>`.
    Claim,
}

impl Kind {
    /// The word of the first column.
    pub fn word(&self) -> &'static str {
        match self {
            Kind::Block => "block",
            Kind::Claim => "claim",
        }
    }
}

/// What a test makes of a row (design "Data Models").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// A model that runs through the host library.
    Run,
    /// A model the binary refuses, asserted refused with its code and message.
    Refused,
    /// A piece of a model, held by the test of the block it names.
    Fragment,
    /// A listing held to the real binary by a `piperine-cli` test.
    Held,
    /// A block that shows a command, a manifest or a source the guards do not run.
    Shown,
    /// A diagnostic or a sentence the code meets.
    Holds,
    /// A diagnostic or a sentence the code does not meet yet, to be fixed.
    Diverges,
    /// An error code the spec names that no source raises.
    Unraised,
    /// A sentence the spec itself looks wrong about.
    Disputed,
    /// A claim only the binary can make, held by a `piperine-cli` test.
    Binary,
    /// A sentence that states no value and no outcome.
    Prose,
    /// A row no chapter task has settled.
    Pending,
}

impl Verdict {
    /// The verdict spelled `word`, or `None` when it is no verdict.
    pub fn parse(word: &str) -> Option<Verdict> {
        Some(match word {
            "run" => Verdict::Run,
            "refused" => Verdict::Refused,
            "fragment" => Verdict::Fragment,
            "held" => Verdict::Held,
            "shown" => Verdict::Shown,
            "holds" => Verdict::Holds,
            "diverges" => Verdict::Diverges,
            "unraised" => Verdict::Unraised,
            "disputed" => Verdict::Disputed,
            "binary" => Verdict::Binary,
            "prose" => Verdict::Prose,
            "pending" => Verdict::Pending,
            _ => return None,
        })
    }

    /// The word of the verdict column.
    pub fn word(&self) -> &'static str {
        match self {
            Verdict::Run => "run",
            Verdict::Refused => "refused",
            Verdict::Fragment => "fragment",
            Verdict::Held => "held",
            Verdict::Shown => "shown",
            Verdict::Holds => "holds",
            Verdict::Diverges => "diverges",
            Verdict::Unraised => "unraised",
            Verdict::Disputed => "disputed",
            Verdict::Binary => "binary",
            Verdict::Prose => "prose",
            Verdict::Pending => "pending",
        }
    }

    /// Whether a block row may carry this verdict: `msl` blocks run, are refused or are a
    /// fragment; a listing (`data-cli-output`) is held; a `diag` block holds, diverges, is
    /// unraised or disputed; `sh`, `toml`, other `text` and `rust` blocks are shown; any block may
    /// be pending.
    pub fn fits(&self, block: &Block) -> bool {
        let listing = block.attribute("data-cli-output").is_some();
        match self {
            Verdict::Pending => true,
            Verdict::Held => listing,
            Verdict::Run | Verdict::Refused | Verdict::Fragment => block.language() == "msl" && !listing,
            Verdict::Holds | Verdict::Diverges | Verdict::Unraised | Verdict::Disputed => block.language() == "diag",
            Verdict::Shown => !listing && matches!(block.language(), "sh" | "toml" | "text" | "rust"),
            Verdict::Binary | Verdict::Prose => false,
        }
    }
}

/// One line of the inventory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    pub id: String,
    pub fingerprint: String,
    pub verdict: Verdict,
    /// The test that holds the row: `<crate>/<target>::<fn>`, a block id for a fragment, `-` for none.
    pub covered_by: String,
    /// The `GAPS.md` id of a row the binary does not meet, `-` for none.
    pub gap: String,
    /// The listing's command, or the sentence; `-` for none.
    pub text: String,
}

impl Row {
    /// The row as one line of the file.
    pub fn line(&self) -> String {
        [self.kind.word(), &self.id, &self.fingerprint, self.verdict.word(), &self.covered_by, &self.gap, &self.text].join("\t")
    }

    /// The row a block gets before any chapter task settles it: a listing is held by the CLI test
    /// that holds every listing, a `sh`, `toml`, `text` or `rust` block is shown, any other is pending.
    pub fn first_of(block: &Block) -> Row {
        let (verdict, covered_by) = match (block.attribute("data-cli-output"), block.language()) {
            (Some(_), _) => (Verdict::Held, "piperine-cli/book::every_cli_listing_of_the_book_is_what_the_binary_prints"),
            (None, "sh" | "toml" | "text" | "rust") => (Verdict::Shown, "-"),
            (None, _) => (Verdict::Pending, "-"),
        };
        let text = block.attribute("data-cli-output").unwrap_or("-").to_string();
        Row { kind: Kind::Block, id: block.id().to_string(), fingerprint: block.fingerprint().to_string(), verdict, covered_by: covered_by.to_string(), gap: "-".to_string(), text }
    }
}

impl Row {
    /// The id of the claim row of `sentence` of `chapter`: the chapter, `~`, and the first eight hex
    /// digits of the sentence's fingerprint.
    pub fn claim_id(chapter: &str, sentence: &Sentence) -> String {
        format!("{chapter}~{}", &sentence.fingerprint().to_string()[..8])
    }

    /// The row a flagged sentence gets before any chapter task settles it: pending.
    pub fn first_of_sentence(chapter: &str, sentence: &Sentence) -> Row {
        Row { kind: Kind::Claim, id: Row::claim_id(chapter, sentence), fingerprint: sentence.fingerprint().to_string(), verdict: Verdict::Pending, covered_by: "-".to_string(), gap: "-".to_string(), text: sentence.text().to_string() }
    }
}

/// The rows of the inventory, in file order.
#[derive(Debug)]
pub struct Inventory {
    rows: Vec<Row>,
}

impl Inventory {
    /// `tests/book/inventory.tsv` of this repository.
    pub fn read() -> Inventory {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/book/inventory.tsv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: the inventory is unreadable: {error}", path.display()));
        Inventory::parse(&text)
    }

    /// The inventory `text` holds. Panics naming the line when a line has not seven columns, an
    /// unknown kind or an unknown verdict.
    pub fn parse(text: &str) -> Inventory {
        let mut rows = Vec::new();
        for (number, line) in text.lines().enumerate().filter(|(_, line)| !line.is_empty() && !line.starts_with('#')) {
            let columns: Vec<&str> = line.split('\t').collect();
            let [kind, id, fingerprint, verdict, covered_by, gap, text] = columns.as_slice() else {
                panic!("inventory line {}: {} columns, expected 7", number + 1, columns.len());
            };
            let kind = match *kind {
                "block" => Kind::Block,
                "claim" => Kind::Claim,
                other => panic!("inventory line {}: unknown kind `{other}`", number + 1),
            };
            let verdict = Verdict::parse(verdict).unwrap_or_else(|| panic!("inventory line {}: unknown verdict `{verdict}`", number + 1));
            rows.push(Row { kind, id: id.to_string(), fingerprint: fingerprint.to_string(), verdict, covered_by: covered_by.to_string(), gap: gap.to_string(), text: text.to_string() });
        }
        Inventory { rows }
    }

    /// Every row.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The block rows.
    pub fn blocks(&self) -> Vec<&Row> {
        self.rows.iter().filter(|row| row.kind == Kind::Block).collect()
    }

    /// The block `id` of `book`, after its text is checked against its row's fingerprint: a chapter
    /// test reads a block through this, so a block that changed or moved fails by name instead of
    /// running another text. Panics naming `id` when the inventory has no row for it, or the row's
    /// fingerprint is another one.
    pub fn checked_block<'a>(&self, book: &'a Book, id: &str) -> &'a Block {
        let row = self.row(id).unwrap_or_else(|| panic!("{id}: the inventory has no row"));
        let block = book.block(&BlockId::parse(id).unwrap_or_else(|| panic!("{id}: not a block id")));
        assert_eq!(row.fingerprint, block.fingerprint().to_string(), "{id}: the block changed or moved since its row was written");
        block
    }

    /// The claim rows.
    pub fn claims(&self) -> Vec<&Row> {
        self.rows.iter().filter(|row| row.kind == Kind::Claim).collect()
    }

    /// The row named `id`, if any.
    pub fn row(&self, id: &str) -> Option<&Row> {
        self.rows.iter().find(|row| row.id == id)
    }
}

/// Where the tests a row names live: a repository root with a `Cargo.toml` and the `tests/` of its
/// crates.
pub struct Coverage {
    root: PathBuf,
}

impl Coverage {
    /// The repository this guard belongs to.
    pub fn of_repository() -> Coverage {
        Coverage::at(Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    /// The coverage of the tree at `root` (a guard test plants one).
    pub fn at(root: &Path) -> Coverage {
        Coverage { root: root.to_path_buf() }
    }

    /// The test `covered_by` names: `piperine/<target>::<fn>` is a root target (the path its
    /// `[[test]]` table gives, else `tests/<target>.rs`), `<crate>/<target>::<fn>` is
    /// `crates/<crate>/tests/<target>.rs`. An error says what is missing.
    pub fn resolve(&self, covered_by: &str) -> Result<CoveringTest, String> {
        let (location, name) = covered_by.split_once("::").ok_or_else(|| format!("`{covered_by}` is not `<crate>/<target>::<fn>`"))?;
        let (package, target) = location.split_once('/').ok_or_else(|| format!("`{covered_by}` is not `<crate>/<target>::<fn>`"))?;
        let path = if package == "piperine" { self.root.join(self.root_target_path(target)) } else { self.root.join("crates").join(package).join("tests").join(format!("{target}.rs")) };
        let text = std::fs::read_to_string(&path).map_err(|error| format!("`{covered_by}`: {} is unreadable: {error}", path.display()))?;
        let marker = format!("fn {name}(");
        let at = text.match_indices(&marker).map(|(at, _)| at).find(|at| *at == 0 || text[..*at].ends_with('\n')).ok_or_else(|| format!("`{covered_by}`: {} has no top-level `{marker}`", path.display()))?;
        let body = text[at..].split_once("\n}\n").map_or(&text[at..], |(body, _)| body).to_string();
        Ok(CoveringTest { body })
    }

    /// The path of the root target `target`: its `[[test]]` table's `path` in the root manifest,
    /// else `tests/<target>.rs`.
    fn root_target_path(&self, target: &str) -> String {
        let manifest = std::fs::read_to_string(self.root.join("Cargo.toml")).unwrap_or_default();
        let mut named = false;
        let mut in_test_table = false;
        for line in manifest.lines().map(str::trim) {
            if line.starts_with('[') {
                in_test_table = line == "[[test]]";
                named = false;
            } else if in_test_table {
                if let Some(value) = line.strip_prefix("name").map(str::trim_start).and_then(|rest| rest.strip_prefix('=')) {
                    named = value.trim().trim_matches('"') == target;
                } else if named && let Some(value) = line.strip_prefix("path").map(str::trim_start).and_then(|rest| rest.strip_prefix('=')) {
                    return value.trim().trim_matches('"').to_string();
                }
            }
        }
        format!("tests/{target}.rs")
    }
}

/// A test function a row names, as its text.
pub struct CoveringTest {
    body: String,
}

impl CoveringTest {
    /// The function from `fn <name>(` to its closing brace at the start of a line (a covering test
    /// is a top-level function).
    pub fn body(&self) -> &str {
        &self.body
    }
}
