//! Source formatter (tasks.md T15): whitespace normalization that never
//! touches token content, never reflows lines, and never reindents.
//!
//! **Scope, deliberately bounded (SPEC_DEVIATION-adjacent design call, not
//! a shortcut on a VAMS-NN requirement — this task backs `piperine fmt`
//! only, no requirement ID depends on its output shape):** a full
//! grammar-aware pretty printer (reflowing statements, computing
//! indentation from brace/block nesting) is a large, open-ended problem in
//! its own right and no acceptance criterion needs it. What `format`
//! guarantees instead:
//!
//! - **Compiler-directive lines are copied verbatim**, including any
//!   backslash line-continuation (`` `define X 1 + \<newline> 2 ``) —
//!   detected and copied *before* lexing, so the lexer's real
//!   [`Tok::EscapedIdent`] handling of a bare `\` (a different, legitimate
//!   LRM §2.8.1 construct) never has to disambiguate the two. This is what
//!   "macro continuation preserved" means and how it's guaranteed, not
//!   merely tested.
//! - **Comments are copied verbatim** (the lexer already captures a
//!   comment's exact text as one token).
//! - **Leading indentation on every line is preserved exactly as written**
//!   — this formatter does not compute or opine on indentation.
//! - **Inter-token whitespace on the same line collapses to at most one
//!   space**; adjacent tokens (no gap) stay adjacent. Trailing whitespace
//!   before a newline is dropped (no space is ever emitted right before a
//!   `\n`).
//!
//! Because every rule above is a pure function of "was there a gap" (not
//! of *how much* gap, beyond directive/comment content which is untouched
//! either way), formatting an already-formatted file is a no-op —
//! `format(format(x)) == format(x)` holds by construction, proven for the
//! Phase 1-2 corpus in `tests/fmt.rs`.

use crate::error::ParseError;
use crate::lexer::{Lexer, Tok};

pub struct Formatter;

impl Formatter {
    /// Formats `.va`/`.vams` source text per this module's contract.
    pub fn format(source: &str) -> Result<String, ParseError> {
        let mut out = String::with_capacity(source.len());
        for region in split_regions(source) {
            match region {
                Region::Directive(text) => out.push_str(text),
                Region::Code(text) => out.push_str(&format_code_chunk(text)?),
            }
        }
        Ok(out)
    }
}

enum Region<'a> {
    /// A compiler-directive line (and any backslash-continued follow-on
    /// lines), copied byte-for-byte.
    Directive(&'a str),
    /// Everything else, formatted by [`format_code_chunk`].
    Code(&'a str),
}

/// Splits `source` into directive and code regions. A line is a directive
/// line when its first non-horizontal-whitespace character is `` ` `` —
/// the same convention `preprocessor.rs` relies on (directives start a
/// logical line, LRM chapter 10).
fn split_regions(source: &str) -> Vec<Region<'_>> {
    let mut regions = Vec::new();
    let mut code_start = 0usize;
    let mut pos = 0usize;

    while pos < source.len() {
        let line_start = pos;
        let line_end = match source[pos..].find('\n') {
            Some(rel) => pos + rel + 1, // include the '\n'
            None => source.len(),
        };
        let line = &source[line_start..line_end];

        if is_directive_line(line) {
            if code_start < line_start {
                regions.push(Region::Code(&source[code_start..line_start]));
            }
            let mut region_end = line_end;
            let mut cur = line;
            while ends_with_continuation(cur) && region_end < source.len() {
                let next_start = region_end;
                let next_end = match source[next_start..].find('\n') {
                    Some(rel) => next_start + rel + 1,
                    None => source.len(),
                };
                cur = &source[next_start..next_end];
                region_end = next_end;
            }
            regions.push(Region::Directive(&source[line_start..region_end]));
            code_start = region_end;
            pos = region_end;
        } else {
            pos = line_end;
        }
    }

    if code_start < source.len() {
        regions.push(Region::Code(&source[code_start..]));
    }
    regions
}

fn is_directive_line(line: &str) -> bool {
    line.trim_start_matches([' ', '\t']).starts_with('`')
}

/// A line ends with a line-continuation when its last character before the
/// trailing `\n` (if any) is a lone `\`.
fn ends_with_continuation(line: &str) -> bool {
    let without_nl = line.strip_suffix('\n').unwrap_or(line);
    without_nl.ends_with('\\')
}

/// Re-emits `chunk` with normalized inter-token spacing (module doc's
/// rules). Lexes with the real [`Lexer`] so string/comment content is
/// never mistaken for whitespace to collapse.
fn format_code_chunk(chunk: &str) -> Result<String, ParseError> {
    let toks = Lexer::new(chunk).tokenize()?;
    let mut out = String::with_capacity(chunk.len());
    let mut prev_end: Option<usize> = None;
    let mut at_line_start = true;

    for lexed in &toks {
        match lexed.tok {
            Tok::Eof => {
                if let Some(pe) = prev_end {
                    out.push_str(&chunk[pe..lexed.span.start]);
                }
                break;
            }
            Tok::Newline => {
                out.push('\n');
                prev_end = Some(lexed.span.end);
                at_line_start = true;
                continue;
            }
            _ => {}
        }

        if at_line_start {
            let indent_start = prev_end.unwrap_or(0);
            out.push_str(&chunk[indent_start..lexed.span.start]);
        } else if let Some(pe) = prev_end {
            if pe < lexed.span.start {
                out.push(' ');
            }
        }

        out.push_str(&chunk[lexed.span.start..lexed.span.end]);
        prev_end = Some(lexed.span.end);
        at_line_start = false;
    }

    Ok(out)
}
