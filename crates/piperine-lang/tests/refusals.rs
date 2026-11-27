//! Scope: the refusal owner of the front end (feature `code-clarity`, CC-07). `P0001` is worded in one
//! place, `Refusal::diagnostic`; the helpers are structs with methods.

use std::path::{Path, PathBuf};

use piperine_lang::msl::Span;
use piperine_lang::refusal::forms::{AnalogForm, ConstantForm, DigitalForm, EventFunctionForm, GenericsForm, ItemForm, PatternForm, StructureForm, ValueForm};
use piperine_lang::refusal::{Refusal, RefusalOwner, RefusalRow, RefusedForm};

/// The sources of this crate, read as text.
struct Sources;

impl Sources {
    /// Every `.rs` file under `src/`, in name order.
    fn all() -> Vec<PathBuf> {
        let mut found = Vec::new();
        Self::collect(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        found.sort();
        found
    }

    fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
}

#[test]
fn p0001_is_worded_in_one_place() {
    let spelling = ["\"P000", "1\""].concat();
    let spelled: Vec<PathBuf> = Sources::all().into_iter().filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&spelling))).collect();
    assert_eq!(spelled.len(), 1, "the literal {spelling} is written in {spelled:?}; `Refusal::diagnostic` is its one place");
    assert!(spelled[0].ends_with("refusal.rs"), "the literal {spelling} is written in {spelled:?}, not in refusal.rs");
}

#[test]
fn p0001_does_not_name_a_milestone() {
    let diagnostic = Refusal::of(ValueForm::Assignment).diagnostic(Span::default());
    assert_eq!(diagnostic.message, "assignment is not supported yet");
    assert_eq!(diagnostic.primary.1, "not supported yet");
    let milestone = ["M1", " subset"].concat();
    let naming: Vec<PathBuf> = Sources::all().into_iter().filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&milestone))).collect();
    assert!(naming.is_empty(), "these sources still name the milestone `{milestone}`: {naming:?}");
}

#[test]
fn the_catch_all_refusal_files_are_gone() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(!source.join("types/unsupported.rs").exists(), "types/unsupported.rs is gone: its names live in refusal/syntax.rs");
    assert!(!source.join("elab/unlowered.rs").exists(), "elab/unlowered.rs is gone: its forms are rows of the refusal table");
}

/// The refusal table, read as one list of rows.
struct RefusalTable;

impl RefusalTable {
    /// Every form of every family, each paired with the row its family gives it, in declaration order.
    fn entries() -> Vec<(RefusedForm, &'static RefusalRow)> {
        let mut entries = Vec::new();
        entries.extend(ValueForm::ALL.iter().map(|form| (RefusedForm::Value(*form), form.row())));
        entries.extend(PatternForm::ALL.iter().map(|form| (RefusedForm::Pattern(*form), form.row())));
        entries.extend(AnalogForm::ALL.iter().map(|form| (RefusedForm::Analog(*form), form.row())));
        entries.extend(DigitalForm::ALL.iter().map(|form| (RefusedForm::Digital(*form), form.row())));
        entries.extend(StructureForm::ALL.iter().map(|form| (RefusedForm::Structure(*form), form.row())));
        entries.extend(GenericsForm::ALL.iter().map(|form| (RefusedForm::Generics(*form), form.row())));
        entries.extend(ConstantForm::ALL.iter().map(|form| (RefusedForm::Constant(*form), form.row())));
        entries.extend(ItemForm::ALL.iter().map(|form| (RefusedForm::Item(*form), form.row())));
        entries.extend(EventFunctionForm::ALL.iter().map(|form| (RefusedForm::EventFunction(*form), form.row())));
        entries
    }

    /// The number of variants the table's enums declare, read from `src/refusal/forms.rs`.
    fn declared_variants() -> usize {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/refusal/forms.rs")).expect("the table source is readable");
        let mut inside = false;
        let mut count = 0;
        for line in text.lines() {
            if line.starts_with("pub enum ") {
                inside = true;
            } else if line == "}" {
                inside = false;
            } else if inside && line.starts_with("    ") && line.ends_with(',') && !line.trim_start().starts_with("//") {
                count += 1;
            }
        }
        count
    }

    /// The repository's `GAPS.md`.
    fn gaps() -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../GAPS.md")).expect("GAPS.md is readable")
    }
}

#[test]
fn every_refused_form_has_a_row() {
    let entries = RefusalTable::entries();
    assert_eq!(entries.len(), RefusalTable::declared_variants(), "a form of `refusal/forms.rs` is missing from its family's `ALL`");
    for (form, row) in &entries {
        assert_eq!(&row.form, form, "the row of {form:?} words another form: `ROWS` follows the declaration order of the enum");
        assert!(!row.text.is_empty(), "the row of {form:?} has no words");
    }
}

/// The `P0001` messages one `GAPS.md` row quotes: the strings of its column that starts with `` `P0001`: ``, never its
/// prose (a row's prose may mention a form it does not own).
struct QuotedMessages {
    messages: Vec<String>,
}

impl QuotedMessages {
    /// The openers of a form's context clause: a punctuated clause (`: it has no finite symbolic reading`, `, in a
    /// function that returns an `Event``, the bare `,` after a detail) or a place (` in a `fn` called inside a `ddx``,
    /// ` before the event of ..`).
    const CONTEXT_OPENERS: [&'static str; 4] = [":", ",", " in ", " before "];

    /// The messages quoted by the row `id` of `gaps`, or `None` when it has no such row or the row has no `P0001` column.
    fn of_row(gaps: &str, id: &str) -> Option<QuotedMessages> {
        let line = gaps.lines().find(|line| line.starts_with(&format!("| {id} |")))?;
        let unescaped = line.replace("\\|", "\u{1}");
        let column = unescaped.split('|').map(str::trim).find(|cell| cell.starts_with("`P0001`:"))?;
        let messages = column.split('"').skip(1).step_by(2).map(|quote| quote.replace('\u{1}', "|")).collect();
        Some(QuotedMessages { messages })
    }

    /// Whether one of the messages is the message of the form worded `text`.
    fn names_form(&self, text: &str) -> bool {
        self.messages.iter().any(|quote| QuotedMessages::message_names(quote, text))
    }

    /// Where the last context clause of `text` starts: the last opener in it, taken with the comma right before it
    /// (`, in ..` is one clause), or `None` when the words have no clause.
    fn context_clause(text: &str) -> Option<usize> {
        let last = (0..text.len()).rev().find(|at| text.is_char_boundary(*at) && QuotedMessages::CONTEXT_OPENERS.iter().any(|opener| text[*at..].starts_with(opener)))?;
        let joined = last > 0 && text[..last].ends_with(',') && !text[last..].starts_with(',');
        Some(if joined { last - 1 } else { last })
    }

    /// Whether the last `{}` of `text` is its context clause, the body the form is raised in (`a `let` of a pattern that
    /// is no name in {}`): such a form is shared by the bodies that fill it, so no one gap owns it.
    fn detail_is_context(text: &str) -> bool {
        QuotedMessages::context_clause(text).is_some_and(|start| text[start..].contains("{}"))
    }

    /// Whether `quote` is `text`, or `text` without its last context clause, with each `{}` of the words read as the
    /// detail it stands for (any non-empty text).
    fn message_names(quote: &str, text: &str) -> bool {
        let cut = QuotedMessages::context_clause(text).map(|start| &text[..start]);
        QuotedMessages::fills(quote, text) || cut.is_some_and(|words| QuotedMessages::fills(quote, words))
    }

    /// Whether `quote` is `words` with each `{}` filled by a non-empty detail.
    fn fills(quote: &str, words: &str) -> bool {
        let pieces: Vec<&str> = words.split("{}").collect();
        let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
        if pieces.len() == 1 {
            return quote == words;
        }
        let Some(mut rest) = quote.strip_prefix(first) else { return false };
        for piece in &pieces[1..pieces.len() - 1] {
            let detail = rest.chars().next().map_or(0, char::len_utf8);
            let Some(found) = rest.get(detail..).and_then(|after| after.find(piece)) else { return false };
            rest = &rest[detail + found + piece.len()..];
        }
        rest.len() > last.len() && rest.ends_with(last)
    }
}

/// A form names `Gap(id)` only when that `GAPS.md` row quotes the form's message in its `P0001` column. GAPS quotes a
/// message whole, or abbreviated by leaving off its trailing context clause (`a `while` or `loop` in a `fn` called inside
/// a `ddx`` for the words `..: it has no finite symbolic reading`; `a `match` arm whose pattern binds a name` for
/// `.., in a function that returns an `Event``), and fills a `{}` with an example detail (`value input port of type
/// `module ..``). Only the last context clause is left off, so a quote cannot own a form by its first words alone. The
/// row's prose is never read: L74's prose mentions "an assignment to a field or an element", a form it does not own. A form
/// whose last `{}` is its context clause (the body it is raised in, shared by ddx and event-function bodies) has no
/// `Gap` owner: each body is its own gap.
#[test]
fn every_gap_owner_names_a_gaps_row() {
    let gaps = RefusalTable::gaps();
    for (_, row) in RefusalTable::entries() {
        if let RefusalOwner::Gap(id) = row.owner {
            assert!(!QuotedMessages::detail_is_context(row.text), "{:?} names `{id}` as its owner but its last `{{}}` is the body it is raised in (`{}`): a shared form stays `Unswept`", row.form, row.text);
            let quoted = QuotedMessages::of_row(&gaps, id).unwrap_or_else(|| panic!("the form {:?} names `{id}` as its owner and GAPS.md has no such row with a `P0001` column", row.form));
            assert!(quoted.names_form(row.text), "{:?} names `{id}` as its owner but that row's `P0001` column quotes none of {:?} as its words `{}`", row.form, quoted.messages, row.text);
        }
    }
}

#[test]
fn the_unswept_count_only_shrinks() {
    let swept = RefusalTable::entries().iter().filter(|(_, row)| row.owner == RefusalOwner::Unswept).count();
    let recorded: usize = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/unswept_count.txt")).expect("the count is recorded").trim().parse().expect("the count is a number");
    assert!(swept <= recorded, "{swept} forms are `Unswept`, the record holds {recorded}: a new form needs an owner (`Gap(id)`), the count never grows");
    assert_eq!(swept, recorded, "{swept} forms are `Unswept`: write {swept} in tests/support/unswept_count.txt (the count only shrinks)");
}

#[test]
fn each_form_words_live_in_one_row() {
    let mut seen: std::collections::BTreeMap<&str, RefusedForm> = std::collections::BTreeMap::new();
    for (form, row) in RefusalTable::entries() {
        if let Some(first) = seen.insert(row.text, form.clone()) {
            panic!("the words `{}` are the row of {first:?} and of {form:?}: a form's words are defined in exactly one row", row.text);
        }
    }
}
