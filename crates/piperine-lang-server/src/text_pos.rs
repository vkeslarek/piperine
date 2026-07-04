//! Single source of truth for LSP position handling.
//!
//! LSP clients (VS Code default) speak **UTF-16 code units** in
//! `Position.character` (LSP 3.17 lets them negotiate `utf-8`; we declare
//! `utf-16` in `server_capabilities` and stick with that). Source text in
//! `Value::text` is a UTF-8 byte string (Rust `&str`). All four handlers
//! that previously did ad-hoc `chars().count()` round-trips — wrong for any
//! non-ASCII content (and off-by-one on surrogate pairs for non-BMP code
//! points) — route through this module.

use lsp_types::{Position, Range};

/// Convert an LSP `Position` (UTF-16 line/column) to a UTF-8 byte offset
/// into `source`. Past-end columns clamp to the end of the requested line
/// (or to `source.len()`); past-end lines clamp to the start of the next
/// line. Both match VS Code's editor behaviour.
pub fn position_to_byte(source: &str, position: Position) -> usize {
    // 1. Locate the byte where the requested line starts.
    let mut line_start = 0usize;
    let mut line = 0u32;
    if position.line == 0 {
        line_start = 0;
    } else {
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line += 1;
                if line == position.line {
                    line_start = i + 1;
                    break;
                }
            }
        }
        if line < position.line {
            // Past EOF — clamp to end-of-source.
            return source.len();
        }
    }

    // 2. Walk the line, accumulating UTF-16 columns, until we reach or
    // exceed `position.character`. The newline that ends the line is
    // NOT part of this line (column counter resets).
    let mut col16 = 0u32;
    let mut byte = line_start;
    while byte < source.len() {
        // Stop before advancing past a newline — the request line ends.
        let next_newline = source[byte..].bytes().position(|b| b == b'\n').unwrap_or(source.len() - byte);
        let line_end = byte + next_newline;
        while byte < line_end && col16 < position.character {
            let c = source[byte..].chars().next().unwrap();
            let step = utf16_len(c) as u32;
            // Don't overshoot — if this char would exceed the target col,
            // snap to the byte of the previous char.
            if col16 + step > position.character {
                // Snap back to the start of this char.
                // No col16 update; break out via `col16 == target` next.
                return byte;
            }
            col16 += step;
            byte += c.len_utf8();
        }
        if col16 >= position.character {
            return byte;
        }
        // Reached end of line without hitting the target col — clamp to
        // end-of-line (byte before the newline, or the line-end itself).
        return line_end.min(source.len());
    }
    source.len()
}

/// Inverse of `position_to_byte`: UTF-8 byte offset → UTF-16 `(line,
/// character)`. Clamps to the end-of-source.
pub fn byte_to_position(source: &str, byte_offset: usize) -> Position {
    let offset = byte_offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col16 = source[line_start..offset]
        .chars()
        .map(|c| utf16_len(c) as u32)
        .sum();
    Position::new(line, col16)
}

/// Build a `Range` for `[start, end)` (UTF-8 byte offsets).
pub fn byte_range(source: &str, start: usize, end: usize) -> Range {
    Range::new(
        byte_to_position(source, start),
        byte_to_position(source, end),
    )
}

/// Identifier at `position`: ASCII letter/`_` followed by ASCII
/// letters/digits/`_`, extended backward and forward. If the cursor sits
/// on whitespace just after an identifier, returns the trailing word.
pub fn word_at_position(source: &str, position: Position) -> Option<String> {
    let offset = position_to_byte(source, position);
    let start = if offset < source.len() {
        let c = source[offset..].chars().next().unwrap();
        if is_word_char(c) {
            offset
        } else if offset > 0 {
            let back = prev_char_offset(source, offset);
            if back < source.len() && is_word_char(source[back..].chars().next().unwrap()) {
                back
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else if offset > 0 {
        let back = prev_char_offset(source, offset);
        if back < source.len() && is_word_char(source[back..].chars().next().unwrap()) {
            back
        } else {
            return None;
        }
    } else {
        return None;
    };
    let end = expand_word_right(source, start);
    let start = expand_word_left(source, start);
    if start < end {
        Some(source[start..end].to_string())
    } else {
        None
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn expand_word_left(source: &str, mut i: usize) -> usize {
    while i > 0 {
        let back = prev_char_offset(source, i);
        let c = source[back..].chars().next().unwrap();
        if is_word_char(c) {
            i = back;
        } else {
            break;
        }
    }
    i
}

fn expand_word_right(source: &str, mut i: usize) -> usize {
    while i < source.len() {
        let c = source[i..].chars().next().unwrap();
        if is_word_char(c) {
            i += c.len_utf8();
        } else {
            break;
        }
    }
    i
}

/// Byte offset of the start of the previous char (0 if at start).
fn prev_char_offset(source: &str, i: usize) -> usize {
    if i == 0 {
        return 0;
    }
    let mut j = i - 1;
    while j > 0 && !source.is_char_boundary(j) {
        j -= 1;
    }
    j
}

/// UTF-16 code-unit length of a char (1 or 2).
fn utf16_len(c: char) -> usize {
    c.len_utf16()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(line: u32, ch: u32) -> Position {
        Position::new(line, ch)
    }

    #[test]
    fn ascii_round_trip() {
        let s = "fn main() {\n    let x = 42;\n}\n";
        for off in 0..s.len() {
            let pos = byte_to_position(s, off);
            assert_eq!(position_to_byte(s, pos), off, "byte {off} → {pos:?}");
        }
    }

    #[test]
    fn non_ascii_columns_are_utf16() {
        let s = "var µ = 1;\n";
        assert_eq!(position_to_byte(s, p(0, 4)), 4);
        let s2 = "var é = 1;\n";
        assert_eq!(position_to_byte(s2, p(0, 5)), 6);
        let s3 = "var 𝒻 = 1;\n";
        assert_eq!(position_to_byte(s3, p(0, 5)), 4);
        assert_eq!(position_to_byte(s3, p(0, 6)), 8);
        assert_eq!(byte_to_position(s3, 4), p(0, 4));
        assert_eq!(byte_to_position(s3, 8), p(0, 6));
    }

    #[test]
    fn line_past_eof_clamps() {
        let s = "one\ntwo\n";
        assert_eq!(position_to_byte(s, p(10, 0)), s.len());
    }

    #[test]
    fn column_past_end_of_line_clamps() {
        let s = "abc\ndef\n";
        // Line 0, col 100 → clamp to end of line 0 (byte 3, before '\n').
        assert_eq!(position_to_byte(s, p(0, 100)), 3);
    }

    #[test]
    fn word_at_handles_cursor_positions() {
        let s = "let foo_bar = 1;\n";
        assert_eq!(word_at_position(s, p(0, 4)).as_deref(), Some("foo_bar"));
        assert_eq!(word_at_position(s, p(0, 11)).as_deref(), Some("foo_bar"));
        assert_eq!(word_at_position(s, p(0, 15)).as_deref(), Some("1"));
    }

    #[test]
    fn byte_range_builds_lsp_range() {
        let s = "hello\nworld\n";
        let r = byte_range(s, 6, 11);
        assert_eq!(r.start, p(1, 0));
        assert_eq!(r.end, p(1, 5));
    }
}
