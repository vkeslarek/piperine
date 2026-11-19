//! `Gaps`: section 11 of `GAPS.md`, "The Book against the binary": one row per Book block or claim
//! the binary refuses, does not meet, never raises or the spec disputes, each under the id of the gap
//! that explains it. The inventory guard holds the section equal to the inventory's rows.

use std::path::Path;

/// One row of section 11: `| Book | Gap | Owner | Today |`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GapRow {
    /// The inventory id of the block or claim.
    pub book: String,
    /// The id of the `GAPS.md` entry that explains the row.
    pub gap: String,
    /// The feature that closes the row, or `user` for a disputed one.
    pub owner: String,
    /// What the binary does today, as the row's test asserts it.
    pub today: String,
}

/// `GAPS.md` as text, read for its section 11 and its entry ids.
pub struct Gaps {
    text: String,
}

impl Gaps {
    /// The heading of section 11.
    const SECTION: &'static str = "## 11. The Book against the binary";

    /// `GAPS.md` of this repository.
    pub fn read() -> Gaps {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("GAPS.md");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: GAPS.md is unreadable: {error}", path.display()));
        Gaps::from_text(&text)
    }

    /// The `GAPS.md` that holds `text` (a guard test plants one).
    pub fn from_text(text: &str) -> Gaps {
        Gaps { text: text.to_string() }
    }

    /// The rows of section 11, in order: the table lines under its heading, header and rule
    /// skipped. A table line that has not four cells is listed by `malformed_rows`.
    pub fn rows(&self) -> Vec<GapRow> {
        self.section_lines().filter_map(|line| Self::cells(line)).filter(|cells| cells[0] != "Book" && !cells[0].starts_with('-')).map(|cells| GapRow { book: Self::unquote(cells[0]), gap: Self::unquote(cells[1]), owner: Self::unquote(cells[2]), today: cells[3].to_string() }).collect()
    }

    /// The table lines of section 11 that do not have four cells, as text.
    pub fn malformed_rows(&self) -> Vec<String> {
        self.section_lines().filter(|line| line.starts_with('|')).filter(|line| Self::cells(line).is_none()).map(str::to_string).collect()
    }

    /// Whether the file has an entry with the id `id` outside section 11: a table row `| <id> |` or
    /// a heading `### <id>.`.
    pub fn has_entry(&self, id: &str) -> bool {
        let before = self.text.split(Self::SECTION).next().unwrap_or("");
        before.lines().any(|line| line.starts_with(&format!("| {id} |")) || line.starts_with(&format!("### {id}.")))
    }

    /// The lines of section 11 (from its heading to the next `## ` heading or the end).
    fn section_lines(&self) -> impl Iterator<Item = &str> {
        let section = self.text.split_once(Self::SECTION).map_or("", |(_, after)| after.split("\n## ").next().unwrap_or(""));
        section.lines()
    }

    /// The four cells of a table line, trimmed, or `None` when the line is no four-cell table row.
    fn cells(line: &str) -> Option<[&str; 4]> {
        let inner = line.strip_prefix('|')?.strip_suffix('|')?;
        let cells: Vec<&str> = inner.splitn(4, '|').map(str::trim).collect();
        <[&str; 4]>::try_from(cells).ok()
    }

    /// `text` without the backticks that quote it.
    fn unquote(text: &str) -> String {
        text.trim_matches('`').to_string()
    }
}
